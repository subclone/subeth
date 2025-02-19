use super::*;
use jsonrpsee::{
    core::{async_trait, RpcResult},
    types::ErrorObject,
};
use serde::Deserialize;
use std::{collections::BTreeMap, sync::Mutex};
use subxt::PolkadotConfig;

/// ETH RPC adapter
pub struct EthAdapter {
    client: Mutex<SubLightClient>,
}

/// Substrate block
type SubstrateBlock = subxt::blocks::Block<PolkadotConfig>;

impl EthAdapter {
    /// Create a new instance of the ETH adapter
    pub fn new(client: SubLightClient) -> Self {
        Self {
            client: Mutex::new(client),
        }
    }

    /// Helper function to convert Substrate block to Ethereum block
    fn to_eth_block(substrate_block: SubstrateBlock) -> RichBlock {
        let header = RichHeader::from(&substrate_block.header());

        let block_number = U256::from_str(&substrate_block.header.number).unwrap_or(U256::zero());
        let block_hash = H256::from_str(&substrate_block.hash).unwrap_or(H256::zero());
        let parent_hash =
            H256::from_str(&substrate_block.header.parent_hash).unwrap_or(H256::zero());
        let author = H160::from_str(&substrate_block.header.author).unwrap_or(H160::zero());
        let timestamp = U256::from_str(&substrate_block.header.timestamp).unwrap_or(U256::zero());
        let transactions: Vec<Transaction> = substrate_block
            .extrinsics
            .into_iter()
            .map(Self::to_eth_transaction)
            .collect();

        RichBlock {
            inner: Block {
                header: Header {
                    number: Some(block_number),
                    hash: Some(block_hash),
                    parent_hash,
                    nonce: None,
                    logs_bloom: Bloom::default(),
                    transactions_root: H256::zero(),
                    state_root: H256::from_str(&substrate_block.header.state_root)
                        .unwrap_or(H256::zero()),
                    receipts_root: H256::zero(),
                    miner: Some(author),
                    difficulty: U256::zero(),
                    extra_data: Bytes::default(),
                    size: None,
                    gas_limit: U256::zero(),
                    gas_used: U256::zero(),
                    timestamp,
                    uncles_hash: H256::zero(),
                    author: author,
                },
                size: Some(U256::zero()),
                base_fee_per_gas: None,
                total_difficulty: Some(U256::zero()),
                transactions: BlockTransactions::Full(transactions),
                uncles: Vec::new(),
            },
            extra_info: BTreeMap::new(),
        }
    }

    /// Helper function to convert Substrate transaction to Ethereum transaction
    fn to_eth_transaction(substrate_tx: SubstrateTransaction) -> Transaction {
        let tx_hash = H256::from_str(&substrate_tx.hash).unwrap_or(H256::zero());
        let from = H160::from_str(&substrate_tx.signature.address).unwrap_or(H160::zero());
        let to = H160::from_str(&substrate_tx.call.to).unwrap_or(H160::zero());
        let value = U256::from_str(&substrate_tx.call.value).unwrap_or(U256::zero());
        let gas = U256::from_str(&substrate_tx.call.gas).unwrap_or(U256::zero());
        let gas_price = U256::from_str(&substrate_tx.call.gas_price).unwrap_or(U256::zero());
        let input = substrate_tx
            .call
            .input
            .map(|s| s.as_bytes().to_vec())
            .unwrap_or_default();

        Transaction {
            hash: tx_hash,
            nonce: U256::zero(),
            block_hash: None,
            block_number: None,
            transaction_index: None,
            from,
            to: Some(to),
            value,
            gas_price: Some(gas_price),
            gas,
            input: input.into(),
            v: Some(U256::zero()),
            r: U256::zero(),
            s: U256::zero(),
            ..Default::default()
        }
    }

    /// Helper function to convert Substrate receipt to Ethereum receipt
    fn to_eth_receipt(substrate_receipt: SubstrateReceipt) -> Receipt {
        let tx_hash = H256::from_str(&substrate_receipt.transaction_hash).unwrap_or(H256::zero());
        let block_hash = H256::from_str(&substrate_receipt.block_hash).unwrap_or(H256::zero());
        let block_number = U256::from_str(&substrate_receipt.block_number).unwrap_or(U256::zero());
        let gas_used = U256::from_str(&substrate_receipt.gas_used).unwrap_or(U256::zero());
        let status = U256::from_str(&substrate_receipt.status).unwrap_or(U256::zero());
        let logs = substrate_receipt
            .logs
            .into_iter()
            .map(Self::to_eth_log)
            .collect();

        Receipt {
            transaction_hash: Some(tx_hash),
            transaction_index: Some(U256::zero()),
            block_hash: Some(block_hash),
            block_number: Some(block_number),
            cumulative_gas_used: gas_used,
            gas_used: Some(gas_used),
            contract_address: None,
            logs,
            logs_bloom: Bloom::default(),
        }
    }

    /// Helper function to convert Substrate log to Ethereum log
    fn to_eth_log(substrate_log: SubstrateLog) -> Log {
        let address = H160::from_str(&substrate_log.address).unwrap_or(H160::zero());
        let topics = substrate_log
            .topics
            .into_iter()
            .map(|topic| H256::from_str(&topic).unwrap_or(H256::zero()))
            .collect();
        let data = substrate_log
            .data
            .map(|s| s.as_bytes().to_vec())
            .unwrap_or_default();

        Log {
            address,
            topics,
            data: data.into(),
            block_hash: None,
            block_number: None,
            transaction_hash: None,
            transaction_index: None,
            log_index: None,
            removed: false,
            transaction_log_index: None,
        }
    }

    /// Parse a JSON value into a SubstrateBlock
    fn parse_substrate_block(json_value: serde_json::Value) -> Result<SubstrateBlock, String> {
        serde_json::from_value(json_value).map_err(|e| e.to_string())
    }

    /// Parse a JSON value into a SubstrateTransaction
    fn parse_substrate_transaction(
        json_value: serde_json::Value,
    ) -> Result<SubstrateTransaction, String> {
        serde_json::from_value(json_value).map_err(|e| e.to_string())
    }

    /// Parse a JSON value into a SubstrateReceipt
    fn parse_substrate_receipt(json_value: serde_json::Value) -> Result<SubstrateReceipt, String> {
        serde_json::from_value(json_value).map_err(|e| e.to_string())
    }

    /// Parse a JSON value into a SubstrateLog
    fn parse_substrate_log(json_value: serde_json::Value) -> Result<SubstrateLog, String> {
        serde_json::from_value(json_value).map_err(|e| e.to_string())
    }
}

/// Implement the ETH API server
#[async_trait]
impl EthApiServer for EthAdapter {
    // ########################################################################
    // Client
    // ########################################################################

    fn protocol_version(&self) -> RpcResult<u64> {
        Ok(1)
    }

    async fn syncing(&self) -> RpcResult<SyncStatus> {
        // Query sync status from substrate
        let status = self
            .client
            .lock()
            .await
            .request_blocking("system_syncState", vec![])
            .await
            .map_err(|e| ErrorObject::from(e))?;

        // Parse response and convert to ETH sync status
        Ok(sync_status)
    }

    fn author(&self) -> RpcResult<H160> {
        unimplemented!()
    }

    fn accounts(&self) -> RpcResult<Vec<H160>> {
        unimplemented!()
    }

    async fn block_number(&self) -> RpcResult<U256> {
        let response = self
            .client
            .lock()
            .await
            .request_blocking("chain_getBlock", vec![])
            .await?;
        let block_number = serde_json::from_str(&response)
            .map_err(|e| ErrorObject::owned(500, e.to_string(), None::<()>))?;

        Ok(block_number)
    }

    fn chain_id(&self) -> RpcResult<Option<U64>> {
        let response = self
            .client
            .lock()
            .await
            .request_blocking("eth_chainId", vec![])
            .map_err(|e| ErrorObject::owned(500, e.to_string(), None::<()>))?;
        let chain_id: U64 = serde_json::from_str(&response)
            .map_err(|e| ErrorObject::owned(500, e.to_string(), None::<()>))?;
        Ok(Some(chain_id))
    }

    // ########################################################################
    // Block
    // ########################################################################

    async fn block_by_hash(&self, hash: H256, full: bool) -> RpcResult<Option<RichBlock>> {
        let response = self
            .client
            .lock()
            .await
            .request_blocking("chain_getBlock", vec![serde_json::json!(hash)])
            .await?;
        let substrate_block_json: serde_json::Value = serde_json::from_str(&response)
            .map_err(|e| ErrorObject::owned(500, e.to_string(), None::<()>))?;
        let substrate_block = Self::parse_substrate_block(substrate_block_json)
            .map_err(|e| ErrorObject::owned(500, "Failed to parse block", Some(e)))?;
        let eth_block = Self::to_eth_block(substrate_block);
        Ok(Some(eth_block))
    }

    async fn block_by_number(
        &self,
        number_or_hash: BlockNumberOrHash,
        full: bool,
    ) -> RpcResult<Option<RichBlock>> {
        let block_number = match number_or_hash {
            BlockNumberOrHash::Num(number) => number.to_string(),
            BlockNumberOrHash::Hash {
                hash,
                require_canonical: _,
            } => hash.to_string(),
        };

        let response = self
            .client
            .lock()
            .await
            .request_blocking("chain_getBlock", vec![serde_json::json!(block_number)])
            .await?;
        let substrate_block_json: serde_json::Value = serde_json::from_str(&response)
            .map_err(|e| ErrorObject::owned(500, e.to_string(), None::<()>))?;
        let substrate_block = Self::parse_substrate_block(substrate_block_json)
            .map_err(|e| ErrorObject::owned(500, "Failed to parse block", Some(e)))?;
        let eth_block = Self::to_eth_block(substrate_block);
        Ok(Some(eth_block))
    }

    async fn block_transaction_count_by_hash(&self, hash: H256) -> RpcResult<Option<U256>> {
        let response = self
            .client
            .lock()
            .await
            .request_blocking("chain_getBlock", vec![serde_json::json!(hash)])
            .await?;
        let substrate_block_json: serde_json::Value = serde_json::from_str(&response)
            .map_err(|e| ErrorObject::owned(500, e.to_string(), None::<()>))?;
        let substrate_block = Self::parse_substrate_block(substrate_block_json)
            .map_err(|e| ErrorObject::owned(500, "Failed to parse block", Some(e)))?;
        let tx_count = U256::from(substrate_block.extrinsics.len());
        Ok(Some(tx_count))
    }

    async fn block_transaction_count_by_number(
        &self,
        number_or_hash: BlockNumberOrHash,
    ) -> RpcResult<Option<U256>> {
        let block_number = match number_or_hash {
            BlockNumberOrHash::Number(number) => number.to_string(),
            BlockNumberOrHash::Hash(hash) => hash.to_string(),
        };

        let response = self
            .client
            .lock()
            .await
            .request_blocking("chain_getBlock", vec![serde_json::json!(block_number)])
            .await?;
        let substrate_block_json: serde_json::Value = serde_json::from_str(&response)
            .map_err(|e| ErrorObject::owned(500, e.to_string(), None::<()>))?;
        let substrate_block = Self::parse_substrate_block(substrate_block_json)
            .map_err(|e| ErrorObject::owned(500, "Failed to parse block", Some(e)))?;
        let tx_count = U256::from(substrate_block.extrinsics.len());
        Ok(Some(tx_count))
    }

    async fn block_transaction_receipts(
        &self,
        number_or_hash: BlockNumberOrHash,
    ) -> RpcResult<Option<Vec<Receipt>>> {
        let block_number = match number_or_hash {
            BlockNumberOrHash::Number(number) => number.to_string(),
            BlockNumberOrHash::Hash(hash) => hash.to_string(),
        };

        let response = self
            .client
            .lock()
            .await
            .request_blocking("chain_getBlock", vec![serde_json::json!(block_number)])
            .await?;
        let substrate_block_json: serde_json::Value = serde_json::from_str(&response)
            .map_err(|e| ErrorObject::owned(500, e.to_string(), None::<()>))?;
        let substrate_block = Self::parse_substrate_block(substrate_block_json)
            .map_err(|e| ErrorObject::owned(500, "Failed to parse block", Some(e)))?;
        let receipts = substrate_block
            .extrinsics
            .into_iter()
            .map(|tx| {
                let receipt_json = serde_json::to_value(tx).unwrap();
                Self::parse_substrate_receipt(receipt_json)
                    .map(Self::to_eth_receipt)
                    .unwrap_or_default()
            })
            .collect();
        Ok(Some(receipts))
    }

    fn block_uncles_count_by_hash(&self, hash: H256) -> RpcResult<U256> {
        Ok(U256::zero()) // Substrate doesn't have uncles
    }

    fn block_uncles_count_by_number(&self, number_or_hash: BlockNumberOrHash) -> RpcResult<U256> {
        Ok(U256::zero()) // Substrate doesn't have uncles
    }

    fn uncle_by_block_hash_and_index(
        &self,
        hash: H256,
        index: Index,
    ) -> RpcResult<Option<RichBlock>> {
        Ok(None) // Substrate doesn't have uncles
    }

    fn uncle_by_block_number_and_index(
        &self,
        number_or_hash: BlockNumberOrHash,
        index: Index,
    ) -> RpcResult<Option<RichBlock>> {
        Ok(None) // Substrate doesn't have uncles
    }

    // ########################################################################
    // Transaction
    // ########################################################################

    async fn transaction_by_hash(&self, hash: H256) -> RpcResult<Option<Transaction>> {
        let response = self
            .client
            .lock()
            .await
            .request_blocking("chain_getTransaction", vec![serde_json::json!(hash)])
            .await?;
        let substrate_tx_json: serde_json::Value = serde_json::from_str(&response)
            .map_err(|e| ErrorObject::owned(500, e.to_string(), None::<()>))?;
        let substrate_tx = Self::parse_substrate_transaction(substrate_tx_json)
            .map_err(|e| ErrorObject::owned(500, "Failed to parse transaction", Some(e)))?;
        let eth_tx = Self::to_eth_transaction(substrate_tx);
        Ok(Some(eth_tx))
    }

    async fn transaction_by_block_hash_and_index(
        &self,
        hash: H256,
        index: Index,
    ) -> RpcResult<Option<Transaction>> {
        let response = self
            .client
            .lock()
            .await
            .request_blocking("chain_getBlock", vec![serde_json::json!(hash)])
            .await?;
        let substrate_block_json: serde_json::Value = serde_json::from_str(&response)
            .map_err(|e| ErrorObject::owned(500, e.to_string(), None::<()>))?;
        let substrate_block = Self::parse_substrate_block(substrate_block_json)
            .map_err(|e| ErrorObject::owned(500, "Failed to parse block", Some(e)))?;
        let tx = substrate_block
            .extrinsics
            .get(index.as_usize())
            .map(|tx| Self::to_eth_transaction(tx.clone()));
        Ok(tx)
    }

    async fn transaction_by_block_number_and_index(
        &self,
        number_or_hash: BlockNumberOrHash,
        index: Index,
    ) -> RpcResult<Option<Transaction>> {
        let block_number = match number_or_hash {
            BlockNumberOrHash::Number(number) => number.to_string(),
            BlockNumberOrHash::Hash(hash) => hash.to_string(),
        };

        let response = self
            .client
            .lock()
            .await
            .request_blocking("chain_getBlock", vec![serde_json::json!(block_number)])
            .await?;
        let substrate_block_json: serde_json::Value = serde_json::from_str(&response)
            .map_err(|e| ErrorObject::owned(500, e.to_string(), None::<()>))?;
        let substrate_block = Self::parse_substrate_block(substrate_block_json)
            .map_err(|e| ErrorObject::owned(500, "Failed to parse block", Some(e)))?;
        let tx = substrate_block
            .extrinsics
            .get(index.as_usize())
            .map(|tx| Self::to_eth_transaction(tx.clone()));
        Ok(tx)
    }

    async fn transaction_receipt(&self, hash: H256) -> RpcResult<Option<Receipt>> {
        let response = self
            .client
            .lock()
            .await
            .request_blocking("chain_getTransactionReceipt", vec![serde_json::json!(hash)])
            .await?;
        let substrate_receipt_json: serde_json::Value = serde_json::from_str(&response)
            .map_err(|e| ErrorObject::owned(500, e.to_string(), None::<()>))?;
        let substrate_receipt = Self::parse_substrate_receipt(substrate_receipt_json)
            .map_err(|e| ErrorObject::owned(500, "Failed to parse receipt", Some(e)))?;
        let eth_receipt = Self::to_eth_receipt(substrate_receipt);
        Ok(Some(eth_receipt))
    }

    // ########################################################################
    // State
    // ########################################################################

    async fn balance(
        &self,
        address: H160,
        number_or_hash: Option<BlockNumberOrHash>,
    ) -> RpcResult<U256> {
        let block = number_or_hash.unwrap_or(BlockNumberOrHash::Number(BlockNumber::Latest));
        let response = self
            .client
            .lock()
            .await
            .request_blocking(
                "state_getBalance",
                vec![serde_json::json!(address), serde_json::json!(block)],
            )
            .await?;
        let balance: U256 = serde_json::from_str(&response)
            .map_err(|e| ErrorObject::owned(500, e.to_string(), None::<()>))?;
        Ok(balance)
    }

    async fn storage_at(
        &self,
        address: H160,
        index: U256,
        number_or_hash: Option<BlockNumberOrHash>,
    ) -> RpcResult<H256> {
        let block = number_or_hash.unwrap_or(BlockNumberOrHash::Number(BlockNumber::Latest));
        let response = self
            .client
            .lock()
            .await
            .request_blocking(
                "state_getStorageAt",
                vec![
                    serde_json::json!(address),
                    serde_json::json!(index),
                    serde_json::json!(block),
                ],
            )
            .await?;
        let storage: H256 = serde_json::from_str(&response)
            .map_err(|e| ErrorObject::owned(500, e.to_string(), None::<()>))?;
        Ok(storage)
    }

    async fn transaction_count(
        &self,
        address: H160,
        number_or_hash: Option<BlockNumberOrHash>,
    ) -> RpcResult<U256> {
        let block = number_or_hash.unwrap_or(BlockNumberOrHash::Number(BlockNumber::Latest));
        let response = self
            .client
            .lock()
            .await
            .request_blocking(
                "state_getTransactionCount",
                vec![serde_json::json!(address), serde_json::json!(block)],
            )
            .await?;
        let count: U256 = serde_json::from_str(&response)
            .map_err(|e| ErrorObject::owned(500, e.to_string(), None::<()>))?;
        Ok(count)
    }

    async fn code_at(
        &self,
        address: H160,
        number_or_hash: Option<BlockNumberOrHash>,
    ) -> RpcResult<Bytes> {
        let block = number_or_hash.unwrap_or(BlockNumberOrHash::Number(BlockNumber::Latest));
        let response = self
            .client
            .lock()
            .await
            .request_blocking(
                "state_getCode",
                vec![serde_json::json!(address), serde_json::json!(block)],
            )
            .await?;
        let code: Bytes = serde_json::from_str(&response)
            .map_err(|e| ErrorObject::owned(500, e.to_string(), None::<()>))?;
        Ok(code)
    }

    // ########################################################################
    // Execute
    // ########################################################################

    async fn call(
        &self,
        request: TransactionRequest,
        number_or_hash: Option<BlockNumberOrHash>,
        state_overrides: Option<BTreeMap<H160, CallStateOverride>>,
    ) -> RpcResult<Bytes> {
        let block = number_or_hash.unwrap_or(BlockNumberOrHash::Number(BlockNumber::Latest));
        let response = self
            .client
            .lock()
            .await
            .request_blocking(
                "state_call",
                vec![serde_json::json!(request), serde_json::json!(block)],
            )
            .await?;
        let result: Bytes = serde_json::from_str(&response)
            .map_err(|e| ErrorObject::owned(500, e.to_string(), None::<()>))?;
        Ok(result)
    }

    async fn estimate_gas(
        &self,
        request: TransactionRequest,
        number_or_hash: Option<BlockNumberOrHash>,
    ) -> RpcResult<U256> {
        let block = number_or_hash.unwrap_or(BlockNumberOrHash::Number(BlockNumber::Latest));
        let response = self
            .client
            .lock()
            .await
            .request_blocking(
                "state_estimateGas",
                vec![serde_json::json!(request), serde_json::json!(block)],
            )
            .await?;
        let gas: U256 = serde_json::from_str(&response)
            .map_err(|e| ErrorObject::owned(500, e.to_string(), None::<()>))?;
        Ok(gas)
    }

    // ########################################################################
    // Fee
    // ########################################################################

    fn gas_price(&self) -> RpcResult<U256> {
        let response = self
            .client
            .lock()
            .await
            .request_blocking("eth_gasPrice", vec![])
            .map_err(|e| ErrorObject::owned(500, e.to_string(), None::<()>))?;
        let gas_price: U256 = serde_json::from_str(&response)
            .map_err(|e| ErrorObject::owned(500, e.to_string(), None::<()>))?;
        Ok(gas_price)
    }

    async fn fee_history(
        &self,
        block_count: U256,
        newest_block: BlockNumberOrHash,
        reward_percentiles: Option<Vec<f64>>,
    ) -> RpcResult<FeeHistory> {
        unimplemented!() // Not required for Milestone 1
    }

    fn max_priority_fee_per_gas(&self) -> RpcResult<U256> {
        unimplemented!() // Not required for Milestone 1
    }

    // ########################################################################
    // Mining
    // ########################################################################

    fn is_mining(&self) -> RpcResult<bool> {
        Ok(false) // Substrate doesn't have mining
    }

    fn hashrate(&self) -> RpcResult<U256> {
        Ok(U256::zero()) // Substrate doesn't have mining
    }

    fn work(&self) -> RpcResult<Work> {
        unimplemented!() // Not required for Milestone 1
    }

    fn submit_hashrate(&self, hashrate: U256, id: H256) -> RpcResult<bool> {
        Ok(false) // Substrate doesn't have mining
    }

    fn submit_work(&self, nonce: H64, pow_hash: H256, mix_digest: H256) -> RpcResult<bool> {
        Ok(false) // Substrate doesn't have mining
    }

    // ########################################################################
    // Submit
    // ########################################################################

    async fn send_transaction(&self, request: TransactionRequest) -> RpcResult<H256> {
        unimplemented!() // Not required for Milestone 1
    }

    async fn send_raw_transaction(&self, bytes: Bytes) -> RpcResult<H256> {
        unimplemented!() // Not required for Milestone 1
    }
}
