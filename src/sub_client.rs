//! Smoldot light client process
//!
//! Wrapped structure for Substrate light client that uses smoldot internally

use crate::adapter::{hash_key, AddressMapping, PalletContractMapping, StorageKey};
use crate::server::BlockNotification;
use crate::types::*;
use alloy_consensus::{Signed, TxEip1559};
use alloy_primitives::{Address, ChainId, PrimitiveSignature, TxKind, B256, U256};
use alloy_rpc_types_eth::pubsub::SubscriptionKind;
use alloy_rpc_types_eth::{
    Block as EthBlock, BlockHashOrNumber, BlockNumberOrTag, Header as EthHeader, Index,
    Transaction, TransactionReceipt, TransactionRequest,
};
use frame_support::StorageHasher;
use futures::{Stream, StreamExt};
use jsonrpsee::{SubscriptionMessage, SubscriptionSink};
use subxt::backend::legacy::LegacyRpcMethods;
use subxt::backend::rpc::RpcClient;
use subxt::blocks::{Block, ExtrinsicDetails};
use subxt::metadata::types::StorageEntryType;
use subxt::utils::{AccountId32, MultiAddress, H256};
use subxt::{lightclient::LightClient, OnlineClient};

type SubstrateBlock = Block<ChainConfig, OnlineClient<ChainConfig>>;
type EthTransaction = Transaction;

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
    /// Represents a connection to RPC
    rpc: LegacyRpcMethods<ChainConfig>,
    /// Chain ID of the Substrate chain
    chain_id: ChainId,
    /// Properties of the chain
    properties: Properties,
}

impl SubLightClient {
    async fn new(rpc: impl Into<RpcClient>, chain_id: ChainId) -> anyhow::Result<Self> {
        let rpc = rpc.into();
        let api = OnlineClient::<ChainConfig>::from_rpc_client(rpc.clone()).await?;
        let rpc = LegacyRpcMethods::new(rpc);
        let system_props = rpc.system_properties().await?;

        let properties = Properties {
            decimals: system_props
                .get("tokenDecimals")
                .and_then(|v| v.as_u64())
                .expect("tokenDecimals should be set") as u32,
            symbol: system_props
                .get("tokenSymbol")
                .and_then(|v| v.as_str())
                .expect("tokenSymbol should be set")
                .to_string(),
        };

        Ok(Self {
            inner: None,
            api,
            rpc,
            chain_id,
            properties,
        })
    }

    pub async fn from_light_client(chain_spec: &str, chain_id: ChainId) -> anyhow::Result<Self> {
        let (inner, rpc) = LightClient::relay_chain(chain_spec)?;

        let mut client = Self::new(rpc, chain_id).await?;

        client.inner = Some(inner);

        Ok(client)
    }

    pub async fn from_url(url: &str, chain_id: ChainId) -> anyhow::Result<Self> {
        let rpc = RpcClient::from_url(url).await?;

        Self::new(rpc, chain_id).await
    }
}

impl SubLightClient {
    /// Chain id
    pub fn chain_id(&self) -> ChainId {
        self.chain_id
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
    ) -> Result<EthBlock, SubEthError> {
        let substrate_block = match block_number {
            BlockNumberOrTag::Latest => Some(self.api.blocks().at_latest().await?),
            BlockNumberOrTag::Number(n) => {
                let block_hash = self
                    .rpc
                    .chain_get_block_hash(Some(
                        subxt::backend::legacy::rpc_methods::NumberOrHex::Number(n),
                    ))
                    .await?;
                if let Some(hash) = block_hash {
                    Some(self.api.blocks().at(hash).await?)
                } else {
                    None
                }
            }
            _ => None,
        };

        if let Some(block) = substrate_block {
            convert_block(block, self.properties.decimals).await
        } else {
            Err(SubEthError::AdapterError {
                message: "Block not found".to_string(),
            })
        }
    }

    /// Get block by hash
    pub async fn get_block_by_hash(&self, block_hash: H256) -> Result<EthBlock, SubEthError> {
        let block = self.api.blocks().at(block_hash).await?;
        convert_block(block, self.properties.decimals).await
    }

    /// Get balance of an address
    pub async fn get_balance(&self, address: Address) -> Result<U256, SubEthError> {
        let account_id = AddressMapping::to_ss58(address);
        let query = storage().system().account(&account_id);
        let balance = self
            .api
            .storage()
            .at_latest()
            .await?
            .fetch(&query)
            .await?
            .ok_or(SubEthError::AdapterError {
                message: "Balance not found".to_string(),
            })?;

        Ok(U256::from(balance.data.free))
    }

    /// Get transaction count
    pub async fn get_transaction_count(&self, address: Address) -> Result<U256, SubEthError> {
        let account_id = AddressMapping::to_ss58(address);
        let query = storage().system().account(&account_id);
        let nonce = self
            .api
            .storage()
            .at_latest()
            .await?
            .fetch(&query)
            .await?
            .ok_or(SubEthError::AdapterError {
                message: "Couldn't fetch account from the storage".to_string(),
            })?
            .nonce;

        Ok(U256::from(nonce))
    }

    /// Get code of a contract
    ///
    /// In our case, (for now) it returns `revert` bytecode if the given address is a pallet's contract address
    pub fn get_code(&self, address: Address) -> Result<Vec<u8>, SubEthError> {
        let pallet_name =
            PalletContractMapping::pallet_name(address).ok_or(SubEthError::AdapterError {
                message: "Address is not a contract".to_string(),
            })?;
        let code = format!("revert: {}", pallet_name);
        Ok(code.into_bytes())
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
            .ok_or(SubEthError::AdapterError {
                message: "Storage value not found".to_string(),
            })?;

        Ok(storage_value)
    }

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
        block: BlockHashOrNumber,
        tx_index: Index,
    ) -> Result<Option<EthTransaction>, SubEthError> {
        let block_hash = if let Some(hash) = block.as_hash() {
            hash.0.into()
        } else {
            let block_hash = self
                .rpc
                .chain_get_block_hash(Some(
                    subxt::backend::legacy::rpc_methods::NumberOrHex::Number(
                        block.as_number().expect("should be a number"),
                    ),
                ))
                .await?;
            if let Some(hash) = block_hash {
                hash
            } else {
                return Ok(None);
            }
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
        let dest = request.to.ok_or(SubEthError::AdapterError {
            message: "Destination not found".to_string(),
        })?;
        let pallet_name = match dest {
            TxKind::Call(dest) => {
                PalletContractMapping::pallet_name(dest).ok_or(SubEthError::AdapterError {
                    message: "Destination is not a contract".to_string(),
                })?
            }
            _ => {
                return Err(SubEthError::AdapterError {
                    message: "Unsupported transaction type".to_string(),
                })
            }
        };
        let input = request.input.input.ok_or(SubEthError::AdapterError {
            message: "Call input not found".to_string(),
        })?;

        let storage_key: StorageKey =
            serde_json::from_slice(&input).map_err(|_| SubEthError::AdapterError {
                message: "Invalid call input".to_string(),
            })?;

        let metadata = self.api.metadata();
        let pallet = metadata
            .pallet_by_name(&pallet_name)
            .ok_or(SubEthError::AdapterError {
                message: "Pallet not found".to_string(),
            })?;
        let storage = pallet.storage().ok_or(SubEthError::AdapterError {
            message: "Pallet storage not found".to_string(),
        })?;

        let entry = storage
            .entry_by_name(&storage_key.name)
            .ok_or(SubEthError::AdapterError {
                message: "Storage entry not found".to_string(),
            })?;

        let mut final_key = vec![];

        let pallet_prefix_hashed = frame_support::Twox128::hash(pallet_name.as_bytes());
        let storage_prefix_hashed = frame_support::Twox128::hash(storage_key.name.as_bytes());

        final_key.extend_from_slice(&pallet_prefix_hashed);
        final_key.extend_from_slice(&storage_prefix_hashed);

        let storage_final_key = match entry.entry_type() {
            StorageEntryType::Plain(_) => vec![],
            StorageEntryType::Map {
                hashers,
                key_ty: _,
                value_ty: _,
            } => {
                let mut final_key = vec![];
                for (i, hasher) in hashers.iter().enumerate() {
                    let key_raw = storage_key.keys.get(i).ok_or(SubEthError::AdapterError {
                        message: "Number of storage keys does not match with hashers".to_string(),
                    })?;
                    let key = hash_key(key_raw, hasher);

                    final_key.extend_from_slice(&key);
                }

                final_key
            }
        };

        final_key.extend_from_slice(&storage_final_key);

        let value = self
            .api
            .storage()
            .at_latest()
            .await?
            .fetch_raw(final_key)
            .await?;

        Ok(value)
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
    let from: [u8; 32] = ext
        .address_bytes()
        .ok_or(SubEthError::AdapterError {
            message: "Address not found".to_string(),
        })?
        .try_into()
        .expect("should be safe to convert");
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

    let nonce = ext
        .signed_extensions()
        .expect("should have signed extensions")
        .nonce()
        .ok_or(SubEthError::AdapterError {
            message: "Nonce not found".to_string(),
        })?;
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
