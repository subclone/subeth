//! # Subeth RPC adapter
//!
//! this is an adapter for any Substrate based chain to convert it to an ETH-RPC interface.
//!
//! ## smoldot
//!
//! It exposes an RPC interface with most of the methods that are available in the Ethereum JSON-RPC interface.
//! Internally, it instantiates a `smoldot` light client instance to the respective Substrate chain and
//! forwards the requests to the light client.

use crate::sub_client::SubLightClient;
mod adapter;
mod command;
mod server;
mod sub_client;
mod traits;
mod types;

#[tokio::main]
async fn main() {
    println!("Subeth RPC adapter!");

    let chain_spec = include_str!("../specs/polkadot.json");
    let client = SubLightClient::new(chain_spec).await.unwrap();
    let adapter = server::EthAdapter::new(client, vec![]);

    println!("Latest block: {:?}", 1);
}
