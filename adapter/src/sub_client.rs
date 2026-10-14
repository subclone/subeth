//! Smoldot light client process
//!
//! Wrapped structure for Substrate light client that uses smoldot internally

use crate::adapter::{hash_key, AddressMapping, PalletContractMapping, StorageKey};
use crate::cache::BlockCache;
use crate::server::BlockNotification;
use crate::types::*;
use alloy_consensus::{Signed, TxEip1559};
use alloy_primitives::{Address, ChainId, PrimitiveSignature, TxKind, U256};
use alloy_rpc_types_eth::pubsub::SubscriptionKind;
use alloy_rpc_types_eth::{
    Block as EthBlock, BlockId, BlockNumberOrTag, Header as EthHeader, Index, SyncStatus,
    TransactionRequest,
};
use frame_support::StorageHasher;
use futures::{Stream, StreamExt};
use jsonrpsee::{SubscriptionMessage, SubscriptionSink};
use parity_scale_codec::Decode;
use subxt::backend::rpc::RpcClient;
use subxt::blocks::ExtrinsicDetails;
use subxt::metadata::types::StorageEntryType;
use subxt::rpc_params;
use subxt::storage::Storage;
use subxt::utils::{AccountId32, MultiAddress, H256};
use subxt::{lightclient::LightClient, OnlineClient};

#[derive(Debug, Clone)]
pub struct Properties {
    /// Decimals of the token
    decimals: u32,
    /// Symbol of the token
    symbol: String,
}

impl Properties {
    /// Fallback used when the chain does not report token properties
    const DEFAULT_DECIMALS: u32 = 10;
    const DEFAULT_SYMBOL: &'static str = "DOT";

    /// Parse the result of `system_properties`.
    ///
    /// `tokenDecimals`/`tokenSymbol` may be a single value or a list (multi-token chains),
    /// in which case the first entry is the native token.
    fn from_system_properties(props: &serde_json::Value) -> Self {
        let first = |key: &str| match props.get(key) {
            Some(serde_json::Value::Array(values)) => values.first().cloned(),
            other => other.cloned(),
        };
        Self {
            decimals: first("tokenDecimals")
                .and_then(|v| v.as_u64())
                .map(|d| d as u32)
                .unwrap_or(Self::DEFAULT_DECIMALS),
            symbol: first("tokenSymbol")
                .and_then(|v| v.as_str().map(str::to_string))
                .unwrap_or_else(|| Self::DEFAULT_SYMBOL.to_string()),
        }
    }
}

/// Represents the Substrate light client
#[derive(Clone)]
pub struct SubLightClient {
    /// Represents the client instance of the Substrate chain
    inner: Option<LightClient>,
    /// Represents a chains API
    api: OnlineClient<ChainConfig>,
    /// Rpc client itself
    rpc_client: RpcClient,
    /// Chain ID of the Substrate chain
    chain_id: ChainId,
    /// Properties of the chain
    properties: Properties,
    /// Cache for the chain
    cache: BlockCache,
}

impl SubLightClient {
    async fn new(
        api: OnlineClient<ChainConfig>,
        rpc: RpcClient,
        chain_id: ChainId,
        cache_capacity: Option<usize>,
    ) -> anyhow::Result<Self> {
        let properties = match rpc
            .request::<serde_json::Value>("system_properties", rpc_params![])
            .await
        {
            Ok(props) => Properties::from_system_properties(&props),
            Err(e) => {
                log::warn!("Failed to fetch chain properties, using defaults: {:?}", e);
                Properties::from_system_properties(&serde_json::Value::Null)
            }
        };
        log::info!(
            "Native token: {} ({} decimals)",
            properties.symbol,
            properties.decimals
        );

        Ok(Self {
            inner: None,
            api,
            chain_id,
            properties,
            rpc_client: rpc,
            cache: BlockCache::new(cache_capacity),
        })
    }

    pub async fn from_light_client(
        chain_spec: &str,
        chain_id: ChainId,
        cache_blocks: Option<usize>,
    ) -> anyhow::Result<Self> {
        let (inner, rpc) = LightClient::relay_chain(chain_spec)?;
        let rpc_client: RpcClient = rpc.into();
        let api = OnlineClient::<ChainConfig>::from_rpc_client(rpc_client.clone()).await?;

        let mut client = Self::new(api, rpc_client, chain_id, cache_blocks).await?;

        client.inner = Some(inner);

        Ok(client)
    }

    pub async fn from_url(
        url: &str,
        chain_id: ChainId,
        cache_blocks: Option<usize>,
    ) -> anyhow::Result<Self> {
        let rpc = RpcClient::from_url(url).await?;
        let api = OnlineClient::<ChainConfig>::from_url(url).await?;

        Self::new(api, rpc, chain_id, cache_blocks).await
    }
}

impl SubLightClient {
    /// Chain id
    pub fn chain_id(&self) -> ChainId {
        self.chain_id
    }

    pub fn syncing(&self) -> Result<SyncStatus, SubEthError> {
        Ok(SyncStatus::None)
    }

    /// Current block number
    pub async fn block_number(&self) -> Result<u64, SubEthError> {
        let latest_block = self.api.blocks().at_latest().await?;
        Ok(latest_block.number().into())
    }

    /// Resolve a block number or tag to a block hash.
    ///
    /// Substrate has no pending block and the client follows finalized blocks, so `latest`,
    /// `pending`, `safe` and `finalized` all resolve to the latest finalized block.
    pub async fn resolve_block_hash(
        &self,
        block: BlockNumberOrTag,
    ) -> Result<Option<H256>, SubEthError> {
        let number = match block {
            BlockNumberOrTag::Number(n) => n,
            BlockNumberOrTag::Earliest => 0,
            BlockNumberOrTag::Latest
            | BlockNumberOrTag::Pending
            | BlockNumberOrTag::Safe
            | BlockNumberOrTag::Finalized => {
                return Ok(Some(self.api.blocks().at_latest().await?.hash()));
            }
        };

        if let Some(hash) = self.cache.get_hash_by_number(number) {
            return Ok(Some(hash));
        }

        let hash = self
            .rpc_client
            .request::<Option<H256>>("chain_getBlockHash", rpc_params![number])
            .await?;
        if let Some(hash) = hash {
            self.cache.insert_number_to_hash(number, hash);
        }
        Ok(hash)
    }

    /// Get block by number or tag
    pub async fn get_block_by_number(
        &self,
        block_number: BlockNumberOrTag,
    ) -> Result<Option<EthBlock>, SubEthError> {
        match self.resolve_block_hash(block_number).await? {
            Some(hash) => self.get_block_by_hash(hash).await.map(Some),
            None => Ok(None),
        }
    }

    /// Get block by hash
    pub async fn get_block_by_hash(&self, block_hash: H256) -> Result<EthBlock, SubEthError> {
        if let Some(block) = self.cache.get_by_hash(&block_hash) {
            return Ok(block);
        }
        let block = self.api.blocks().at(block_hash).await?;
        let eth_block = convert_block(block, self.properties.decimals).await?;
        self.cache.insert_block(eth_block.clone());
        Ok(eth_block)
    }

    /// Storage view at the given block, defaulting to the latest block
    async fn storage_at_block(
        &self,
        block: Option<BlockId>,
    ) -> Result<Storage<ChainConfig, OnlineClient<ChainConfig>>, SubEthError> {
        let hash = match block {
            None => return Ok(self.api.storage().at_latest().await?),
            Some(BlockId::Hash(hash)) => H256(hash.block_hash.0),
            Some(BlockId::Number(number)) => self
                .resolve_block_hash(number)
                .await?
                .ok_or(SubEthError::RequestFailed("Unknown block"))?,
        };
        Ok(self.api.storage().at(hash))
    }

    /// Get balance of an address
    pub async fn get_balance(
        &self,
        address: Address,
        block: Option<BlockId>,
    ) -> Result<U256, SubEthError> {
        let account_id = AddressMapping::to_ss58(address);
        let query = storage().system().account(&account_id);
        let account = self.storage_at_block(block).await?.fetch(&query).await?;

        if let Some(account_info) = account {
            Ok(U256::from(account_info.data.free))
        } else {
            Ok(U256::ZERO)
        }
    }

    /// Get transaction count
    pub async fn get_transaction_count(
        &self,
        address: Address,
        block: Option<BlockId>,
    ) -> Result<U256, SubEthError> {
        let account_id = AddressMapping::to_ss58(address);
        let query = storage().system().account(&account_id);
        let account = self.storage_at_block(block).await?.fetch(&query).await?;

        if let Some(account_info) = account {
            Ok(U256::from(account_info.nonce))
        } else {
            Ok(U256::ZERO)
        }
    }

    /// Get code of a contract
    ///
    /// In our case, (for now) it returns `revert` bytecode if the given address is a pallet's contract address
    pub fn get_code(&self, address: Address) -> Result<Vec<u8>, SubEthError> {
        // Only addresses that name a pallet in the runtime are "contracts"; the zero address
        // (empty name) and arbitrary EOAs must report empty code.
        if let Some(name) = PalletContractMapping::pallet_name(address) {
            if self.api.metadata().pallet_by_name(&name).is_some() {
                let code = format!("revert: {}", name);
                return Ok(code.into_bytes());
            }
        }

        Ok(vec![])
    }

    /// Get raw storage value for the given key
    ///
    /// The address is ignored: `key` is treated as a full Substrate storage key. `eth_call` is
    /// more flexible for reading pallet storage, so that's preferred.
    pub async fn get_storage_at(
        &self,
        _address: Address,
        key: H256,
        block: Option<BlockId>,
    ) -> Result<Vec<u8>, SubEthError> {
        let storage_value = self
            .storage_at_block(block)
            .await?
            .fetch_raw(key.as_bytes())
            .await?
            .unwrap_or_default();

        Ok(storage_value)
    }

    /// Get a transaction by block number (or tag) and its index in the block
    pub async fn get_transaction_by_block_and_index(
        &self,
        block: BlockNumberOrTag,
        tx_index: Index,
    ) -> Result<Option<EthTransaction>, SubEthError> {
        match self.resolve_block_hash(block).await? {
            Some(hash) => {
                self.get_transaction_by_block_hash_and_index(hash, tx_index)
                    .await
            }
            None => Ok(None),
        }
    }

    /// Get a transaction by block hash and its index in the block
    pub async fn get_transaction_by_block_hash_and_index(
        &self,
        block_hash: H256,
        tx_index: Index,
    ) -> Result<Option<EthTransaction>, SubEthError> {
        let block = self.get_block_by_hash(block_hash).await?;
        let tx = block
            .transactions
            .txns()
            .find(|tx| tx.transaction_index == Some(tx_index.0 as u64))
            .cloned();
        Ok(tx)
    }

    /// Read the storage of a pallet
    pub async fn call(
        &self,
        request: TransactionRequest,
        block: Option<BlockId>,
    ) -> Result<Option<Vec<u8>>, SubEthError> {
        let dest = match request.to {
            Some(TxKind::Call(dest)) => dest,
            _ => return Ok(None),
        };

        let pallet_name = match PalletContractMapping::pallet_name(dest) {
            Some(name) => name,
            None => return Ok(None),
        };

        // Parse storage key from input
        let storage_key: StorageKey = match request
            .input
            .input
            .and_then(|input| serde_json::from_slice(&input).ok())
        {
            Some(key) => key,
            None => return Ok(None),
        };

        let metadata = self.api.metadata();
        // Get metadata and find storage entry using method chaining
        let entry = match metadata
            .pallet_by_name(&pallet_name)
            .and_then(|p| p.storage())
            .and_then(|s| s.entry_by_name(&storage_key.name))
        {
            Some(entry) => entry,
            None => return Ok(None),
        };

        let mut final_key = vec![];

        let pallet_prefix_hashed = frame_support::Twox128::hash(pallet_name.as_bytes());
        let storage_prefix_hashed = frame_support::Twox128::hash(storage_key.name.as_bytes());

        final_key.extend_from_slice(&pallet_prefix_hashed);
        final_key.extend_from_slice(&storage_prefix_hashed);

        let hashers: &[_] = match entry.entry_type() {
            StorageEntryType::Map { hashers, .. } => hashers,
            StorageEntryType::Plain(_) => &[],
        };
        // A partial key would silently read nothing (or a different item), so require one
        // key per hasher.
        if storage_key.keys.len() != hashers.len() {
            return Err(SubEthError::RequestFailed(
                "Number of storage keys does not match the storage item",
            ));
        }
        for (key_raw, hasher) in storage_key.keys.iter().zip(hashers) {
            final_key.extend(hash_key(key_raw, hasher));
        }

        let value = self
            .storage_at_block(block)
            .await?
            .fetch_raw(final_key)
            .await?;

        Ok(value)
    }

    /// Submit an EVM transaction to the chain via the evm-adapter pallet
    ///
    /// This method creates and submits a transaction that calls pallet_evm_adapter::transact
    /// For MVP, it uses Alice's dev account to sign the outer extrinsic.
    /// The inner EVM transaction signature is what authorizes the actual operation.
    pub async fn submit_evm_transaction(
        &self,
        transaction: subeth_primitives::EthereumTransaction,
    ) -> Result<alloy_primitives::B256, SubEthError> {
        use subxt::dynamic::Value;

        // For MVP, use Alice's dev account to sign the extrinsic wrapper
        // The EVM signature inside the transaction is what matters for authorization
        let alice = subxt_signer::sr25519::dev::alice();

        // Helper to convert H256 to Value (as 32-byte array)
        fn h256_to_value(h: &sp_core::H256) -> Value {
            Value::from_bytes(h.as_bytes())
        }

        // Helper to convert H160 to Value (as 20-byte array)
        fn h160_to_value(h: &sp_core::H160) -> Value {
            Value::from_bytes(h.as_bytes())
        }

        // Build access_list as Value
        let access_list_value: Vec<Value> = transaction
            .access_list
            .iter()
            .map(|(addr, keys)| {
                let keys_values: Vec<Value> = keys.iter().map(h256_to_value).collect();
                Value::unnamed_composite(vec![
                    h160_to_value(addr),
                    Value::unnamed_composite(keys_values),
                ])
            })
            .collect();

        // Build the EthereumTransaction as a composite Value
        let u256_values_priority: Vec<Value> = transaction
            .max_priority_fee_per_gas
            .0
            .iter()
            .map(|&w| Value::u128(w as u128))
            .collect();
        let u256_values_max_fee: Vec<Value> = transaction
            .max_fee_per_gas
            .0
            .iter()
            .map(|&w| Value::u128(w as u128))
            .collect();
        let u256_values_value: Vec<Value> = transaction
            .value
            .0
            .iter()
            .map(|&w| Value::u128(w as u128))
            .collect();

        let tx_value = Value::unnamed_composite(vec![
            Value::u128(transaction.chain_id as u128), // chain_id: u64
            Value::u128(transaction.nonce as u128),    // nonce: u64
            Value::unnamed_composite(u256_values_priority), // max_priority_fee_per_gas: U256
            Value::unnamed_composite(u256_values_max_fee), // max_fee_per_gas: U256
            Value::u128(transaction.gas_limit as u128), // gas_limit: u64
            h160_to_value(&transaction.to),            // to: H160
            Value::unnamed_composite(u256_values_value), // value: U256
            Value::from_bytes(&transaction.data),      // data: Vec<u8>
            Value::unnamed_composite(access_list_value), // access_list: Vec<(H160, Vec<H256>)>
            Value::u128(transaction.v as u128),        // v: u64
            h256_to_value(&transaction.r),             // r: H256
            h256_to_value(&transaction.s),             // s: H256
        ]);

        // Build the call dynamically: evm_adapter::transact(transaction)
        let call = subxt::dynamic::tx("EvmAdapter", "transact", vec![tx_value]);

        // Sign and submit
        let tx_hash = self.api.tx().sign_and_submit_default(&call, &alice).await?;

        Ok(alloy_primitives::B256::from_slice(tx_hash.0.as_slice()))
    }

    /// Subscribe new blocks
    ///
    /// the extracted block and ethereum transactions
    async fn subscribe_new_blocks(
        &self,
        subscription_kind: SubscriptionKind,
    ) -> Result<impl Stream<Item = Result<BlockNotification, SubEthError>>, SubEthError> {
        let block_stream = match subscription_kind {
            SubscriptionKind::NewHeads => self.api.blocks().subscribe_finalized().await,
            _ => return Err(SubEthError::Unsupported),
        }?;

        Ok(block_stream.filter_map(|block| async {
            match block {
                Ok(block) => Some(Ok(BlockNotification {
                    hash: block.hash().0.into(),
                    is_new_best: false,
                })),
                Err(_) => Some(Err(SubEthError::AdapterError {
                    message: "Error in block stream".to_string(),
                })),
            }
        }))
    }
}

/// Handle accepted subscription
///
/// Pipes the block stream to the subscription sink
pub async fn handle_accepted_subscription(
    client: SubLightClient,
    kind: SubscriptionKind,
    sink: SubscriptionSink,
) -> Result<(), SubEthError> {
    let mut stream = Box::pin(client.subscribe_new_blocks(kind).await?);

    loop {
        tokio::select! {
            _ = sink.closed() => {
                break;
            },
            maybe_notification = stream.next() => {
                let notif = match maybe_notification {
                    Some(Ok(notif)) => notif,
                    _ => break,
                };

                // `newHeads` subscribers expect Ethereum block header objects
                let block = client.get_block_by_hash(notif.hash.0.into()).await?;
                if sink.send(SubscriptionMessage::from_json(&block.header)?).await.is_err() {
                    break;
                }
            }
        }
    }

    Ok(())
}

/// Given a substrate block, convert it to an Ethereum block
async fn convert_block(block: SubstrateBlock, decimals: u32) -> Result<EthBlock, SubEthError> {
    let block_hash = block.hash().0;

    let header = block.header().clone();

    let parent_hash = header.parent_hash.0;
    let state_root = header.state_root.0;
    let txs_root = header.extrinsics_root.0;

    // because eth timestamp is in seconds. Genesis (and chains without pallet-timestamp)
    // have no `timestamp::set` inherent, so default to zero rather than panicking.
    let timestamp = block
        .extrinsics()
        .await?
        .find_first::<timestamp::calls::types::Set>()?
        .map(|set| set.value.now / 1000)
        .unwrap_or_default();

    let block_transactions = extract_transactions(block_hash, block, decimals).await?;

    let eth_header = EthHeader {
        hash: block_hash.into(),
        inner: alloy_consensus::Header {
            parent_hash: parent_hash.into(),
            state_root: state_root.into(),
            timestamp: timestamp.into(),
            number: header.number.into(),
            transactions_root: txs_root.into(),
            ..Default::default()
        },
        ..Default::default()
    };

    Ok(EthBlock {
        header: eth_header,
        transactions: alloy_rpc_types_eth::BlockTransactions::Full(block_transactions),
        ..Default::default()
    })
}

/// Given a substrate block, extract extrinsics, and convert them to an Ethereum transaction
async fn extract_transactions(
    block_hash: [u8; 32],
    block: SubstrateBlock,
    decimals: u32,
) -> Result<Vec<EthTransaction>, SubEthError> {
    let mut transactions = vec![];

    for ext in block.extrinsics().await?.iter() {
        let eth_tx = convert_extrinsic((block.number().into(), block_hash), ext, decimals).await?;

        transactions.push(eth_tx);
    }

    Ok(transactions)
}

/// Converts an extrinsic to eth-like transaction
///
/// ### Notes
///
/// Some values use default values as they are not available in the substrate block
///
/// ### Pallets
///
/// Balances transfer extrinsics are treated as native token transfers. Other extrinsics are treated as contract calls,
/// where:
///
/// - the contract address is derived from the pallet name
/// - call data is the encoded call bytes
/// - value is the transferred value
/// - gas limit is hard coded to 21000000
async fn convert_extrinsic(
    (block_number, block_hash): (u64, [u8; 32]),
    ext: ExtrinsicDetails<ChainConfig, OnlineClient<ChainConfig>>,
    decimals: u32,
) -> Result<EthTransaction, SubEthError> {
    let tx_hash = ext.hash();
    let tx_index = ext.index();
    // Inherents and unsigned extrinsics have no signer, so we use null address
    let from = ext
        .address_bytes()
        .and_then(signer_address)
        .unwrap_or(Address::ZERO);

    let (dest, value) = {
        // TODO: handle `TransferAll` as well
        let vals = if let Ok(Some(transfer_allow_death)) =
            ext.as_extrinsic::<balances::calls::types::TransferAllowDeath>()
        {
            Some((transfer_allow_death.dest, transfer_allow_death.value))
        } else if let Ok(Some(transfer_keep_alive)) =
            ext.as_extrinsic::<balances::calls::types::TransferKeepAlive>()
        {
            Some((transfer_keep_alive.dest, transfer_keep_alive.value))
        } else {
            None
        };

        // Transfers to account-like destinations become native value transfers. Anything
        // else (other calls, or index/raw destinations) is a call to the pallet's "contract".
        match vals.and_then(|(dest, value)| Some((multi_address_to_eth(dest)?, value))) {
            Some((dest, value)) => (dest, U256::from(value)),
            None => {
                let pallet_name = ext.pallet_name().map_err(|_| SubEthError::AdapterError {
                    message: "Could not fetch pallet name from extrinsic".to_string(),
                })?;
                (
                    PalletContractMapping::contract_address(pallet_name),
                    U256::ZERO,
                )
            }
        }
    };

    let nonce = match ext.signed_extensions() {
        Some(exts) => exts.nonce().unwrap_or_default(),
        None => 0,
    };

    let input = ext.call_bytes().to_vec();

    let inner = alloy_consensus::TxEnvelope::Eip1559(Signed::new_unchecked(
        TxEip1559 {
            nonce,
            gas_limit: 21000000,
            max_fee_per_gas: u128::MAX,
            max_priority_fee_per_gas: 0,
            to: dest.into(),
            value: to_wei(value, decimals),
            input: input.into(),
            ..Default::default()
        },
        PrimitiveSignature::try_from(&[0u8; 65][..]).expect("valid sig; qed"), // TODO: better way to represent this (?)
        tx_hash.0.into(),
    ));

    // convert gas limit from `Weight` to `u64`

    let eth_tx = EthTransaction {
        block_hash: Some(block_hash.into()),
        block_number: Some(block_number).into(),
        from,
        transaction_index: Some(tx_index.into()),
        inner,
        effective_gas_price: None,
    };
    Ok(eth_tx)
}

/// Map a Substrate `MultiAddress` to an Ethereum address, if it identifies an account
fn multi_address_to_eth<I>(address: MultiAddress<AccountId32, I>) -> Option<Address> {
    match address {
        MultiAddress::Id(id) => Some(AddressMapping::to_address(id)),
        MultiAddress::Address32(raw) => Some(AddressMapping::to_address(raw.into())),
        MultiAddress::Address20(raw) => Some(Address::from(raw)),
        MultiAddress::Index(_) | MultiAddress::Raw(_) => None,
    }
}

/// Decode the signer of an extrinsic from its SCALE-encoded `MultiAddress` bytes.
///
/// The bytes include the enum variant prefix, so they can't be reinterpreted as a raw
/// 32-byte account id directly.
fn signer_address(mut bytes: &[u8]) -> Option<Address> {
    let address = MultiAddress::<AccountId32, u32>::decode(&mut bytes).ok()?;
    multi_address_to_eth(address)
}

/// Decimals of the ETH native unit (wei per ether)
const ETH_DECIMALS: u32 = 18;

/// Convert value from chain's native token (with `decimals` decimals) to wei (18 decimals)
fn to_wei(value: U256, decimals: u32) -> U256 {
    let ten = U256::from(10);
    if decimals <= ETH_DECIMALS {
        value.saturating_mul(ten.pow(U256::from(ETH_DECIMALS - decimals)))
    } else {
        value / ten.pow(U256::from(decimals - ETH_DECIMALS))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use parity_scale_codec::Encode;

    #[test]
    fn signer_is_decoded_from_multi_address_bytes() {
        let account = AccountId32::from([7u8; 32]);
        let encoded = MultiAddress::<AccountId32, u32>::Id(account.clone()).encode();
        // 1 byte variant prefix + 32 byte account id: not a plain [u8; 32]
        assert_eq!(encoded.len(), 33);
        assert_eq!(
            signer_address(&encoded),
            Some(AddressMapping::to_address(account))
        );

        let raw20 = [9u8; 20];
        let encoded = MultiAddress::<AccountId32, u32>::Address20(raw20).encode();
        assert_eq!(signer_address(&encoded), Some(Address::from(raw20)));

        assert_eq!(signer_address(&[0xff]), None);
    }

    #[test]
    fn properties_are_read_from_chain() {
        let props = serde_json::json!({ "tokenDecimals": 12, "tokenSymbol": "UNIT" });
        let parsed = Properties::from_system_properties(&props);
        assert_eq!((parsed.decimals, parsed.symbol.as_str()), (12, "UNIT"));

        // Multi-token chains report lists; the first entry is the native token
        let props = serde_json::json!({ "tokenDecimals": [18, 6], "tokenSymbol": ["GLMR", "X"] });
        let parsed = Properties::from_system_properties(&props);
        assert_eq!((parsed.decimals, parsed.symbol.as_str()), (18, "GLMR"));

        // Missing properties fall back to the defaults
        let parsed = Properties::from_system_properties(&serde_json::Value::Null);
        assert_eq!((parsed.decimals, parsed.symbol.as_str()), (10, "DOT"));
    }

    #[test]
    fn to_wei_rescales_to_18_decimals() {
        // 1 DOT (10 decimals) must be worth exactly 1 ether, not 10^10 ether
        let one_dot = U256::from(10_000_000_000u64);
        assert_eq!(to_wei(one_dot, 10), U256::from(10).pow(U256::from(18)));
        // A chain that already uses 18 decimals is passed through unchanged
        assert_eq!(to_wei(U256::from(42), 18), U256::from(42));
        // More than 18 decimals loses precision instead of inflating the value
        assert_eq!(to_wei(U256::from(1_000), 20), U256::from(10));
        // Huge values saturate instead of panicking on overflow
        assert_eq!(to_wei(U256::MAX, 0), U256::MAX);
    }
}
