use super::*;
use alloy_consensus::Receipt;
use alloy_primitives::{Address, Bytes, B256, U256, U64};
use alloy_rpc_types_eth::{
    pubsub::{Params, SubscriptionKind},
    state::StateOverride,
    Block as EthBlock, BlockId, BlockNumberOrTag, BlockOverrides, FeeHistory, Index, SyncStatus,
    Transaction, TransactionRequest, Work,
};
use futures::FutureExt;
use jsonrpsee::{
    core::{async_trait, RpcResult},
    PendingSubscriptionSink,
};
use sub_client::handle_accepted_subscription;
use traits::{EthApiServer, EthPubSubApiServer};

pub type SubscriptionTaskExecutor = std::sync::Arc<dyn sp_core::traits::SpawnNamed>;

/// A notification when new block is received.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct BlockNotification {
    pub hash: B256,
    pub is_new_best: bool,
}

/// The main ETH adapter struct responsible for handling all the ETH RPC methods and converting them to Substrate calls.
pub struct EthAdapter {
    /// The Substrate light client
    client: SubLightClient,
    /// Accounts managed by this ETH adapter
    accounts: Vec<Address>,
    /// Subscription task executor
    executor: SubscriptionTaskExecutor,
}

impl EthAdapter {
    /// Create a new instance of the ETH adapter
    pub fn new(
        client: SubLightClient,
        accounts: Vec<Address>,
        executor: SubscriptionTaskExecutor,
    ) -> Self {
        Self {
            client,
            accounts,
            executor,
        }
    }
}

/// Implement the ETH API server
#[async_trait]
impl EthApiServer for EthAdapter {
    // ########################################################################
    // Client
    // ########################################################################

    /// Returns protocol version encoded as a string (quotes are necessary).
    fn protocol_version(&self) -> RpcResult<u64> {
        Ok(1)
    }

    fn syncing(&self) -> RpcResult<SyncStatus> {
        let status = self.client.syncing()?;

        Ok(status)
    }

    /// Returns block author.
    fn author(&self) -> RpcResult<Address> {
        unimplemented!()
    }

    /// Returns accounts list.
    fn accounts(&self) -> RpcResult<Vec<Address>> {
        Ok(self.accounts.clone())
    }

    /// Returns highest block number.
    async fn block_number(&self) -> RpcResult<U256> {
        let block_number = self.client.block_number().await?;
        Ok(U256::from(block_number))
    }

    /// Returns the chain ID used for transaction signing at the
    /// current best block. None is returned if not
    /// available.
    fn chain_id(&self) -> RpcResult<Option<U64>> {
        Ok(Some(U64::from(self.client.chain_id())))
    }

    // ########################################################################
    // Block
    // ########################################################################

    /// Returns block with given hash.
    async fn block_by_hash(&self, hash: B256, _full: bool) -> RpcResult<Option<EthBlock>> {
        let block = self.client.get_block_by_hash(hash.0.into()).await?;

        Ok(Some(block))
    }

    /// Returns block with given number.
    async fn block_by_number(
        &self,
        number: BlockNumberOrTag,
        _full: bool,
    ) -> RpcResult<Option<EthBlock>> {
        let block = self.client.get_block_by_number(number).await?;

        Ok(block)
    }

    /// Returns the number of transactions in a block with given hash.
    async fn block_transaction_count_by_hash(&self, _hash: B256) -> RpcResult<Option<U256>> {
        unimplemented!()
    }

    /// Returns the number of transactions in a block with given block number.
    async fn block_transaction_count_by_number(
        &self,
        _number: BlockNumberOrTag,
    ) -> RpcResult<Option<U256>> {
        unimplemented!()
    }

    /// Returns the number of uncles in a block with given hash.
    fn block_uncles_count_by_hash(&self, _hash: B256) -> RpcResult<U256> {
        unimplemented!()
    }

    /// Returns the number of uncles in a block with given block number.
    fn block_uncles_count_by_number(&self, _number: u64) -> RpcResult<U256> {
        unimplemented!()
    }

    /// Returns an uncles at given block and index.
    fn uncle_by_block_hash_and_index(
        &self,
        _hash: B256,
        _index: Index,
    ) -> RpcResult<Option<EthBlock>> {
        unimplemented!()
    }

    /// Returns an uncles at given block and index.
    fn uncle_by_block_number_and_index(
        &self,
        _number: u64,
        _index: Index,
    ) -> RpcResult<Option<EthBlock>> {
        unimplemented!()
    }

    // ########################################################################
    // Transaction
    // ########################################################################

    /// Get transaction by its hash.
    async fn transaction_by_hash(&self, _hash: B256) -> RpcResult<Option<Transaction>> {
        unimplemented!()
    }

    /// Returns transaction by given block number and index.
    async fn transaction_by_block_hash_and_index(
        &self,
        _hash: B256,
        _index: Index,
    ) -> RpcResult<Option<Transaction>> {
        unimplemented!()
    }

    /// Returns transaction by given block number and index.
    async fn transaction_by_block_number_and_index(
        &self,
        number: BlockNumberOrTag,
        index: Index,
    ) -> RpcResult<Option<Transaction>> {
        let tx = self
            .client
            .get_transaction_by_block_and_index(number, index)
            .await?;

        Ok(tx)
    }

    /// Returns transaction receipt by transaction hash.
    async fn transaction_receipt(&self, _hash: B256) -> RpcResult<Option<Receipt>> {
        unimplemented!()
    }

    // ########################################################################
    // State
    // ########################################################################

    /// Returns balance of the given account.
    async fn balance(&self, address: Address, _number_or_tag: Option<BlockId>) -> RpcResult<U256> {
        let balance = self.client.get_balance(address).await?;

        Ok(balance)
    }

    /// Returns content of the storage at given address.
    async fn storage_at(
        &self,
        address: Address,
        key: B256,
        _number_or_tag: Option<BlockId>,
    ) -> RpcResult<Vec<u8>> {
        let storage = self.client.get_storage_at(address, key.0.into()).await?;

        Ok(storage)
    }

    /// Returns the number of transactions sent from given address at given time (block number).
    async fn transaction_count(
        &self,
        address: Address,
        _number_or_tag: Option<BlockNumberOrTag>,
    ) -> RpcResult<U256> {
        let count = self.client.get_transaction_count(address).await?;

        Ok(count)
    }

    /// Returns the code at given address at given time (block number).
    async fn code_at(
        &self,
        address: Address,
        _number_or_tag: Option<BlockNumberOrTag>,
    ) -> RpcResult<Bytes> {
        let code = self.client.get_code(address)?;

        Ok(code.into())
    }

    // ########################################################################
    // Execute
    // ########################################################################

    /// Call contract, returning the output data.
    async fn call(
        &self,
        request: TransactionRequest,
        _block_number: Option<BlockId>,
        _state_overrides: Option<StateOverride>,
        _block_overrides: Option<Box<BlockOverrides>>,
    ) -> RpcResult<Bytes> {
        let res = self.client.call(request).await?;

        if let Some(output) = res {
            Ok(output.into())
        } else {
            Ok(Bytes::new())
        }
    }

    /// Estimate gas needed for execution of given contract.
    async fn estimate_gas(
        &self,
        _request: TransactionRequest,
        _block_number: Option<BlockId>,
        _state_override: Option<StateOverride>,
    ) -> RpcResult<U256> {
        unimplemented!()
    }

    // ########################################################################
    // Fee
    // ########################################################################

    /// Returns current gas_price.
    fn gas_price(&self) -> RpcResult<U256> {
        // TODO: fix this
        Ok(U256::from(1_000_000))
    }

    /// Introduced in EIP-1159 for getting information on the appropriate priority fee to use.
    async fn fee_history(
        &self,
        _block_count: U256,
        _newest_block: U256,
        _reward_percentiles: Option<Vec<f64>>,
    ) -> RpcResult<FeeHistory> {
        unimplemented!()
    }

    /// Introduced in EIP-1159, a Geth-specific and simplified priority fee oracle.
    /// Leverages the already existing fee history cache.
    fn max_priority_fee_per_gas(&self) -> RpcResult<U256> {
        unimplemented!()
    }

    // ########################################################################
    // Mining
    // ########################################################################

    /// Returns true if client is actively mining new blocks.
    fn is_mining(&self) -> RpcResult<bool> {
        Ok(false)
    }

    /// Returns the number of hashes per second that the node is mining with.
    fn hashrate(&self) -> RpcResult<U256> {
        Ok(U256::ZERO)
    }

    /// Returns the hash of the current block, the seedHash, and the boundary condition to be met.
    fn work(&self) -> RpcResult<Work> {
        Ok(Work::default())
    }

    /// Used for submitting mining hashrate.
    fn submit_hashrate(&self, _hashrate: U256, _id: B256) -> RpcResult<bool> {
        Ok(false)
    }

    /// Used for submitting a proof-of-work solution.
    fn submit_work(&self, _nonce: u64, _pow_hash: B256, _mix_digest: B256) -> RpcResult<bool> {
        Ok(false)
    }

    // ########################################################################
    // Submit
    // ########################################################################

    /// Sends transaction; will block waiting for signer to return the
    /// transaction hash.
    async fn send_transaction(&self, _request: TransactionRequest) -> RpcResult<B256> {
        unimplemented!("Transaction submission support is not implemented yet.")
    }

    /// Sends signed transaction, returning its hash.
    async fn send_raw_transaction(&self, _bytes: Bytes) -> RpcResult<B256> {
        unimplemented!("Transaction submission support is not implemented yet.")
    }
}

#[async_trait]
impl EthPubSubApiServer for EthAdapter {
    async fn subscribe(
        &self,
        pending: PendingSubscriptionSink,
        kind: SubscriptionKind,
        params: Option<Params>,
    ) -> jsonrpsee::core::SubscriptionResult {
        // Handle the subscription logic here
        println!(
            "Subscribed with kind: {:?}, params: {:?}, id: {:?}",
            kind, params, pending
        );

        let sink = pending.accept().await?;
        let client = self.client.clone();

        let fut = async move {
            match kind {
                SubscriptionKind::NewHeads => {
                    let _ = handle_accepted_subscription(client, kind, sink).await;
                }
                _ => {}
            }
        }
        .boxed();

        self.executor
            .spawn("sub-eth-subscription", Some("subeth"), fut);

        Ok(())
    }
}
