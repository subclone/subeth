use std::collections::BTreeMap;

use super::*;
use ethereum_types::{H160, H256, H64, U256, U64};
use fc_rpc_core::types::*;
use jsonrpsee::core::{async_trait, RpcResult};

/// ETH RPC adapter
pub struct EthAdapter {
    client: SubLightClient,
}

#[async_trait]
impl EthApiServer for EthAdapter {
    // ########################################################################
    // Client
    // ########################################################################

    fn protocol_version(&self) -> RpcResult<u64> {
        unimplemented!()
    }

    async fn syncing(&self) -> RpcResult<SyncStatus> {
        unimplemented!()
    }

    fn author(&self) -> RpcResult<H160> {
        unimplemented!()
    }

    fn accounts(&self) -> RpcResult<Vec<H160>> {
        unimplemented!()
    }

    fn block_number(&self) -> RpcResult<U256> {
        unimplemented!()
    }

    fn chain_id(&self) -> RpcResult<Option<U64>> {
        unimplemented!()
    }

    // ########################################################################
    // Block
    // ########################################################################

    async fn block_by_hash(&self, hash: H256, full: bool) -> RpcResult<Option<RichBlock>> {
        unimplemented!()
    }

    async fn block_by_number(
        &self,
        number_or_hash: BlockNumberOrHash,
        full: bool,
    ) -> RpcResult<Option<RichBlock>> {
        let block = self
            .client
            .request_blocking("eth_getBlockByNumber", vec![])
            .await?;

        Ok(Some(block))
    }

    async fn block_transaction_count_by_hash(&self, hash: H256) -> RpcResult<Option<U256>> {
        unimplemented!()
    }

    async fn block_transaction_count_by_number(
        &self,
        number_or_hash: BlockNumberOrHash,
    ) -> RpcResult<Option<U256>> {
        unimplemented!()
    }

    async fn block_transaction_receipts(
        &self,
        number_or_hash: BlockNumberOrHash,
    ) -> RpcResult<Option<Vec<Receipt>>> {
        unimplemented!()
    }

    fn block_uncles_count_by_hash(&self, hash: H256) -> RpcResult<U256> {
        unimplemented!()
    }

    fn block_uncles_count_by_number(&self, number_or_hash: BlockNumberOrHash) -> RpcResult<U256> {
        unimplemented!()
    }

    fn uncle_by_block_hash_and_index(
        &self,
        hash: H256,
        index: Index,
    ) -> RpcResult<Option<RichBlock>> {
        unimplemented!()
    }

    fn uncle_by_block_number_and_index(
        &self,
        number_or_hash: BlockNumberOrHash,
        index: Index,
    ) -> RpcResult<Option<RichBlock>> {
        unimplemented!()
    }

    // ########################################################################
    // Transaction
    // ########################################################################

    async fn transaction_by_hash(&self, hash: H256) -> RpcResult<Option<Transaction>> {
        unimplemented!()
    }

    async fn transaction_by_block_hash_and_index(
        &self,
        hash: H256,
        index: Index,
    ) -> RpcResult<Option<Transaction>> {
        unimplemented!()
    }

    async fn transaction_by_block_number_and_index(
        &self,
        number_or_hash: BlockNumberOrHash,
        index: Index,
    ) -> RpcResult<Option<Transaction>> {
        unimplemented!()
    }

    async fn transaction_receipt(&self, hash: H256) -> RpcResult<Option<Receipt>> {
        unimplemented!()
    }

    // ########################################################################
    // State
    // ########################################################################

    async fn balance(
        &self,
        address: H160,
        number_or_hash: Option<BlockNumberOrHash>,
    ) -> RpcResult<U256> {
        unimplemented!()
    }

    async fn storage_at(
        &self,
        address: H160,
        index: U256,
        number_or_hash: Option<BlockNumberOrHash>,
    ) -> RpcResult<H256> {
        unimplemented!()
    }

    async fn transaction_count(
        &self,
        address: H160,
        number_or_hash: Option<BlockNumberOrHash>,
    ) -> RpcResult<U256> {
        unimplemented!()
    }

    async fn code_at(
        &self,
        address: H160,
        number_or_hash: Option<BlockNumberOrHash>,
    ) -> RpcResult<Bytes> {
        unimplemented!()
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
        unimplemented!()
    }

    async fn estimate_gas(
        &self,
        request: TransactionRequest,
        number_or_hash: Option<BlockNumberOrHash>,
    ) -> RpcResult<U256> {
        unimplemented!()
    }

    // ########################################################################
    // Fee
    // ########################################################################

    fn gas_price(&self) -> RpcResult<U256> {
        unimplemented!()
    }

    async fn fee_history(
        &self,
        block_count: U256,
        newest_block: BlockNumberOrHash,
        reward_percentiles: Option<Vec<f64>>,
    ) -> RpcResult<FeeHistory> {
        unimplemented!()
    }

    fn max_priority_fee_per_gas(&self) -> RpcResult<U256> {
        unimplemented!()
    }

    // ########################################################################
    // Mining
    // ########################################################################

    fn is_mining(&self) -> RpcResult<bool> {
        unimplemented!()
    }

    fn hashrate(&self) -> RpcResult<U256> {
        unimplemented!()
    }

    fn work(&self) -> RpcResult<Work> {
        unimplemented!()
    }

    fn submit_hashrate(&self, hashrate: U256, id: H256) -> RpcResult<bool> {
        unimplemented!()
    }

    fn submit_work(&self, nonce: H64, pow_hash: H256, mix_digest: H256) -> RpcResult<bool> {
        unimplemented!()
    }

    // ########################################################################
    // Submit
    // ########################################################################

    async fn send_transaction(&self, request: TransactionRequest) -> RpcResult<H256> {
        unimplemented!()
    }

    async fn send_raw_transaction(&self, bytes: Bytes) -> RpcResult<H256> {
        unimplemented!()
    }
}
