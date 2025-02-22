//! Smoldot light client process
//!
//! Wrapped structure for Substrate light client that uses smoldot internally

use alloy_rpc_types_eth::BlockNumberOrTag;
use jsonrpsee::tracing::instrument::WithSubscriber;
use jsonrpsee::Subscribers;
use smoldot_light::platform::DefaultPlatform;
use smoldot_light::{AddChainConfig, AddChainSuccess, ChainId};
use std::iter;
use std::{num::NonZeroU32, sync::Arc};
use subxt::PolkadotConfig;
use subxt_lightclient::JsonRpcError;

use crate::types::SubEthError;
type SubstrateBlock = subxt::blocks::Block<PolkadotConfig>;

/// Represents the Substrate light client
pub(crate) struct SubLightClient {
    /// Represents the client instance of the Substrate chain
    ///
    /// Requests are forwarded to this client
    pub(crate) inner: subxt_lightclient::LightClient,
    /// Represents RPC connection to the light client
    pub(crate) rpc: subxt_lightclient::LightClientRpc,
    /// Chain ID of the Substrate chain
    pub(crate) chain_id: ChainId,
}

impl SubLightClient {
    pub(crate) fn new(chain_spec: &'static str) -> Self {
        let (inner, rpc) = subxt_lightclient::LightClient::relay_chain(chain_spec).unwrap();

        Self {
            inner,
            rpc,
            chain_id,
        }
    }
}

impl SubLightClient {
    /// Chain id
    fn chain_id(&self) -> ChainId {
        self.chain_id
    }

    /// Current block number
    async fn block_number(&self) -> Result<BlockNumberOrTag, SubEthError> {
        let substrate_block = self.rpc.request(&"system_blockNumber", None).await?;
    }
}
