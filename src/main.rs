//! # Subeth RPC adapter
//!
//! this is an adapter for any Substrate based chain to convert it to an ETH-RPC interface.
//!
//! ## smoldot
//!
//! It exposes an RPC interface with most of the methods that are available in the Ethereum JSON-RPC interface.
//! Internally, it instantiates a `smoldot` light client instance to the respective Substrate chain and
//! forwards the requests to the light client.

mod adapter;
mod command;
mod server;
mod sub_client;
mod traits;
mod types;

use crate::sub_client::SubLightClient;
use clap::Parser;
use env_logger::{Builder, Env};
use log::{debug, error, info, warn};

fn init_logger() {
    let env = Env::default()
        .filter_or("RUST_LOG", "info")
        .write_style_or("RUST_LOG_STYLE", "always");

    Builder::from_env(env)
        .format_timestamp(None)
        .format_target(false)
        .init();

    info!("Logger initialized");
}

#[tokio::main]
async fn main() {
    println!("Subeth RPC adapter!");
    init_logger();

    let opts = command::Opts::parse();

    if let Err(e) = command::run(opts).await {
        eprintln!("Error: {:?}", e);
    }
}
