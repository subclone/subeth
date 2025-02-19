//! # Subeth RPC adapter
//!
//! this is an adapter for any Substrate based chain to convert it to an ETH-RPC interface.
//!
//! ## smoldot
//!
//! It exposes an RPC interface with most of the methods that are available in the Ethereum JSON-RPC interface.
//! Internally, it instantiates a `smoldot` light client instance to the respective Substrate chain and
//! forwards the requests to the light client.

use crate::smoldot::SubLightClient;
mod command;
mod eth_adapter;
mod smoldot;
mod traits;
mod types;

#[tokio::main]
async fn main() {
    println!("Subeth RPC adapter!");

    let chain_spec = include_str!("../specs/polkadot.json");
    let mut client = SubLightClient::new(chain_spec);

    let latest_block = client
        .request_blocking("rpc_methods", vec![])
        .await
        .unwrap();

    println!("Latest block: {:?}", latest_block);
}
