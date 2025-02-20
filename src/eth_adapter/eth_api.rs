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
