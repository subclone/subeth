//! Smoldot light client process
//!
//! Wrapped structure for Substrate light client that uses smoldot internally

use jsonrpsee::tracing::instrument::WithSubscriber;
use jsonrpsee::Subscribers;
use smoldot_light::platform::DefaultPlatform;
use smoldot_light::{AddChainConfig, AddChainSuccess, ChainId};
use std::iter;
use std::{num::NonZeroU32, sync::Arc};

use crate::types::SubEthError;

/// Represents the Substrate light client
pub(crate) struct SubLightClient {
    /// Represents the client instance of the Substrate chain
    ///
    /// Requests are forwarded to this client
    pub(crate) client: smoldot_light::Client<Arc<DefaultPlatform>>,
    /// Chain ID of the Substrate chain
    pub(crate) chain_id: ChainId,
    /// This is a stream of JSON-RPC responses, once the request is sent
    /// to the client, the response is received here
    pub(crate) json_rpc_responses: smoldot_light::JsonRpcResponses<Arc<DefaultPlatform>>,
}

impl SubLightClient {
    pub(crate) fn new(chain_spec: &'static str) -> Self {
        let platform = smoldot_light::platform::default::DefaultPlatform::new(
            env!("CARGO_PKG_NAME").into(),
            env!("CARGO_PKG_VERSION").into(),
        );

        let mut client = smoldot_light::Client::new(platform);

        // add chain
        let AddChainSuccess {
            chain_id,
            json_rpc_responses,
        } = client
            .add_chain(AddChainConfig {
                specification: chain_spec.into(),
                json_rpc: smoldot_light::AddChainConfigJsonRpc::Enabled {
                    max_pending_requests: NonZeroU32::new(128).expect("128 is not zero"),
                    max_subscriptions: 1024,
                },
                potential_relay_chains: iter::empty(),
                database_content: "",
                user_data: (),
            })
            .unwrap();

        let json_rpc_responses = json_rpc_responses.ok_or("no json rpc responses").unwrap();

        Self {
            client,
            chain_id,
            json_rpc_responses,
        }
    }
}

impl SubLightClient {
    pub async fn request_blocking(
        &mut self,
        method: &'static str,
        _params: Vec<serde_json::Value>,
    ) -> Result<String, SubEthError> {
        // add the method to the request
        let request_str = format!(
            r#"{{"jsonrpc":"2.0","method":"{}","params":[],"id":1}}"#,
            method,
        );

        self.client.json_rpc_request(request_str, self.chain_id)?;

        // wait for the response
        let response = self.json_rpc_responses.next().await.ok_or("no response")?;

        println!("Received response: {:?}", response);

        Ok(response)
    }

    /// Subscription is a little different from the request, as it is a stream of responses
    /// that are received from the client. So we should spawn a thread to listen to the responses
    /// and return the stream to the caller.
    fn subscribe(&self, method: &'static str, params: Vec<serde_json::Value>) {
        // add the method to the request
        let request_str = format!(
            r#"{{"jsonrpc":"2.0","method":"{}","params":[],"id":1}}"#,
            method,
        );

        self.client
            .json_rpc_request(request_str, self.chain_id)
            .unwrap();

        self.client.with_subscriber(subscriber)
        // wait for the response
    }
}
