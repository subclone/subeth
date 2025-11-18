//! Smoldot light client process
//!
//! Wrapped structure for Substrate light client that uses smoldot internally

use crate::adapter::{hash_key, AddressMapping, PalletContractMapping, StorageKey};
use crate::cache::BlockCache;
use crate::server::BlockNotification;
use crate::types::*;
use alloy_consensus::{Signed, TxEip1559};
use alloy_primitives::{Address, ChainId, PrimitiveSignature, TxKind, B256, U256};
use alloy_rpc_types_eth::pubsub::SubscriptionKind;
use alloy_rpc_types_eth::{
    Block as EthBlock, BlockNumberOrTag, Header as EthHeader, Index, SyncStatus,
    TransactionReceipt, TransactionRequest,
};
use frame_support::StorageHasher;
use futures::{Stream, StreamExt};
use jsonrpsee::{SubscriptionMessage, SubscriptionSink};
use subxt::backend::rpc::RpcClient;
use subxt::blocks::ExtrinsicDetails;
use subxt::metadata::types::StorageEntryType;
use subxt::rpc_params;
use subxt::utils::{AccountId32, MultiAddress, H256};
use subxt::{lightclient::LightClient, OnlineClient};

#[derive(Debug, Clone)]
pub struct Properties {
    /// Decimals of the token
    decimals: u32,
    /// Symbol of the token
    symbol: String,
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
        rpc: impl Into<RpcClient>,
        chain_id: ChainId,
        cache_capacity: Option<usize>,
    ) -> anyhow::Result<Self> {
        let rpc = rpc.into();
        let api = OnlineClient::<ChainConfig>::from_rpc_client(rpc.clone()).await?;

        let properties = Properties {
            decimals: 10,
            symbol: "Dot".to_string(),
        };

        Ok(Self {
            inner: None,
            api,
            chain_id,
            properties,
            rpc_client: rpc.into(),
            cache: BlockCache::new(cache_capacity),
        })
    }

    pub async fn from_light_client(
        chain_spec: &str,
        chain_id: ChainId,
        cache_blocks: Option<usize>,
    ) -> anyhow::Result<Self> {
        let (inner, rpc) = LightClient::relay_chain(chain_spec)?;

        let mut client = Self::new(rpc, chain_id, cache_blocks).await?;

        client.inner = Some(inner);

        Ok(client)
    }

    pub async fn from_url(
        url: &str,
        chain_id: ChainId,
        cache_blocks: Option<usize>,
    ) -> anyhow::Result<Self> {
        let rpc = RpcClient::from_url(url).await?;

        Self::new(rpc, chain_id, cache_blocks).await
    }
}

impl SubLightClient {
    /// Chain id
    pub fn chain_id(&self) -> ChainId {
        self.chain_id
    }

    pub fn syncing(&self) -> Result<SyncStatus, SubEthError> {
        // let status = self.rpc.system_health().await?;
        Ok(SyncStatus::None)
    }

    /// Current block number
    pub async fn block_number(&self) -> Result<u64, SubEthError> {
        let latest_block = self.api.blocks().at_latest().await?;
        Ok(latest_block.number().into())
    }

    /// Get current block
    pub async fn get_block_by_number(
        &self,
        block_number: BlockNumberOrTag,
    ) -> Result<Option<EthBlock>, SubEthError> {
        let substrate_block = match block_number {
            BlockNumberOrTag::Latest => {
                let current_block_number = self.block_number().await?;
                if let Some(block) = self.cache.get_by_number(current_block_number) {
                    return Ok(Some(block));
                }

                Some(self.api.blocks().at_latest().await?)
            }
            BlockNumberOrTag::Number(n) => {
                let block_hash = if let Some(hash) = self.cache.get_hash_by_number(n) {
                    Some(hash)
                } else {
                    self.rpc_client
                        .request::<Option<H256>>("chain_getBlockHash", rpc_params![n])
                        .await?
                };

                if let Some(hash) = block_hash {
                    self.cache.insert_number_to_hash(n, hash);

                    if let Some(block) = self.cache.get_by_hash(&hash) {
                        return Ok(Some(block));
                    }

                    Some(self.api.blocks().at(hash).await?)
                } else {
                    None
                }
            }
            _ => None,
        };

        if let Some(block) = substrate_block {
            let eth_block = convert_block(block, self.properties.decimals).await?;
            self.cache.insert_block(eth_block.clone());
            Ok(Some(eth_block))
        } else {
            Ok(None)
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

    /// Get balance of an address
    pub async fn get_balance(&self, address: Address) -> Result<U256, SubEthError> {
        let account_id = AddressMapping::to_ss58(address);
        let query = storage().system().account(&account_id);
        let current_block_hash = self.cache.get_hash_by_number(self.block_number().await?);

        let account = if let Some(hash) = current_block_hash {
            self.api.storage().at(hash).fetch(&query).await?
        } else {
            self.api.storage().at_latest().await?.fetch(&query).await?
        };

        if let Some(account_info) = account {
            Ok(U256::from(account_info.data.free))
        } else {
            Ok(U256::ZERO)
        }
    }

    /// Get transaction count
    pub async fn get_transaction_count(&self, address: Address) -> Result<U256, SubEthError> {
        let account_id = AddressMapping::to_ss58(address);
        let query = storage().system().account(&account_id);
        let account = self.api.storage().at_latest().await?.fetch(&query).await?;

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
        if let Some(name) = PalletContractMapping::pallet_name(address) {
            let code = format!("revert: {}", name);
            return Ok(code.into_bytes());
        }

        Ok(vec![])
    }

    /// Get storage at a given address
    ///
    /// This could be used to read the pallet's storage, however, `eth_call` is more flexible,
    /// so that's preferred for now
    ///
    /// Due to the nature of Substrate's storage, this function is not implemented
    pub async fn get_storage_at(
        &self,
        _address: Address,
        key: H256,
    ) -> Result<Vec<u8>, SubEthError> {
        let storage_value = self
            .api
            .storage()
            .at_latest()
            .await?
            .fetch_raw(key.as_bytes())
            .await?
            .unwrap_or_default();

        Ok(storage_value)
    }

    // pub async fn get_storage_client(
    //     &self,
    //     at: BlockNumberOrTag,
    // ) -> Result<StorageClient<ChainConfig, OnlineClient<ChainConfig>>, SubEthError> {
    //     if let Some(at) = at.as_number() {
    //         if let Some(hash) = self.cache.get_hash_by_number(at) {
    //             Ok(self.api.storage().at(hash))
    //         } else {
    //             self.api.storage().at_latest().await.map_err(Into::into)
    //         }
    //     }
    // }

    /// Get transaction by hash
    ///
    /// Please, use `get_transaction_by_block_and_index` instead
    pub async fn _get_transaction_by_hash(
        &self,
        _tx_hash: B256,
    ) -> Result<Option<EthTransaction>, SubEthError> {
        unimplemented!("Use `get_transaction_by_block_and_index` instead")
    }

    pub async fn get_transaction_by_block_and_index(
        &self,
        block: BlockNumberOrTag,
        tx_index: Index,
    ) -> Result<Option<EthTransaction>, SubEthError> {
        let number = match block {
            BlockNumberOrTag::Latest => self.block_number().await?,
            BlockNumberOrTag::Number(n) => n,
            _ => {
                return Ok(None);
            }
        };

        if let Some(eth_block) = self.cache.get_by_number(number) {
            if let Some(tx) = eth_block
                .transactions
                .txns()
                .find(|tx| tx.transaction_index == Some(tx_index.0 as u64))
            {
                return Ok(Some(tx.clone()));
            }
        }

        let block_hash = if let Some(hash) = self.cache.get_hash_by_number(number) {
            Some(hash)
        } else {
            self.rpc_client
                .request::<Option<H256>>("chain_getBlockHash", rpc_params![number])
                .await?
        };

        let block_hash = match block_hash {
            Some(hash) => hash,
            None => return Ok(None),
        };

        let block = self.api.blocks().at(block_hash).await?;

        let ext = block
            .extrinsics()
            .await?
            .iter()
            .find(|ext| ext.index() as usize == tx_index.0);

        if let Some(ext) = ext {
            let transaction = convert_extrinsic(
                (block.number().into(), block_hash.0),
                ext,
                self.properties.decimals,
            )
            .await?;
            Ok(Some(transaction))
        } else {
            Ok(None)
        }
    }

    pub async fn _get_transaction_receipt(
        &self,
        _tx_hash: B256,
    ) -> Result<Option<TransactionReceipt>, SubEthError> {
        unimplemented!("Use `get_transaction_by_block_and_index` instead")
    }

    /// Read the storage of a pallet
    pub async fn call(&self, request: TransactionRequest) -> Result<Option<Vec<u8>>, SubEthError> {
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

        if let StorageEntryType::Map { hashers, .. } = entry.entry_type() {
            for (i, hasher) in hashers.iter().enumerate() {
                if let Some(key_raw) = storage_key.keys.get(i) {
                    final_key.extend(hash_key(key_raw, hasher));
                }
            }
        }

        let value = self
            .api
            .storage()
            .at_latest()
            .await?
            .fetch_raw(final_key)
            .await?;

        Ok(value)
    }

    /// Submit an EVM transaction to the chain via the evm-adapter pallet
    ///
    /// This method creates and submits a transaction that calls pallet_evm_adapter::transact
    pub async fn submit_evm_transaction(
        &self,
        transaction: subeth_primitives::EthereumTransaction,
    ) -> Result<alloy_primitives::B256, SubEthError> {
        use parity_scale_codec::Encode;

        // Encode the transaction
        let tx_encoded = transaction.encode();

        // Create the extrinsic call dynamically
        // Note: This is a simplified version. In production, you would:
        // 1. Use a proper keypair for signing
        // 2. Handle nonce management
        // 3. Calculate proper fees

        // For now, we'll use author_submitExtrinsic which submits unsigned
        // In production, you'd use system.submit_transaction or similar
        let call_data = {
            // Pallet index 6, call index 0 (transact)
            let mut data = vec![6u8, 0u8];
            data.extend_from_slice(&tx_encoded);
            data
        };

        // Submit the extrinsic
        let hex_data = format!("0x{}", hex::encode(&call_data));
        let tx_hash: subxt::utils::H256 = self
            .rpc_client
            .request("author_submitExtrinsic", rpc_params![hex_data])
            .await?;

        Ok(alloy_primitives::B256::from_slice(tx_hash.as_bytes()))
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
                let notif = if let Some(notification) = maybe_notification {
                    if let Ok(notif) = notification {
                        notif
                    } else {
                        break ();
                    }
                } else {
                    break ();
                };


                if sink.send(SubscriptionMessage::from_json(&notif)?).await.is_err() {
                    break ();
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

    // because eth timestamp is in seconds
    let timestamp = block
        .extrinsics()
        .await?
        .find_first::<timestamp::calls::types::Set>()?
        .expect("should have timestamp::set")
        .value
        .now
        / 1000;

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
    let from: [u8; 32] = match ext.address_bytes() {
        Some(addr) => addr.try_into().unwrap_or_default(), // TODO: investigate why this fails sometimes
        // Inherents and unsigned extrinsics have no signer, so we use null address
        None => [0u8; 32],
    };
    // let is_unsigned = !ext.is_signed();

    let from = AddressMapping::to_address(AccountId32::from(from));

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

        if let Some((account_id, value)) = vals {
            match account_id {
                MultiAddress::Id(id) => {
                    (AddressMapping::to_address(id.0.into()), U256::from(value))
                }
                MultiAddress::Address32(id) => {
                    (AddressMapping::to_address(id.into()), U256::from(value))
                }
                _ => unreachable!("Unsupported account type; qed"),
            }
        } else {
            let pallet_name = ext.pallet_name().map_err(|_| SubEthError::AdapterError {
                message: "Could not fetch pallet name from extrinsic".to_string(),
            })?;
            (
                PalletContractMapping::contract_address(pallet_name),
                U256::ZERO,
            )
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

/// Convert value from chain's native token to wei
fn to_wei(value: U256, decimals: u32) -> U256 {
    value * U256::from(10).pow(U256::from(decimals))
}
