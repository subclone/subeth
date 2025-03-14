//! # Subeth RPC adapter
//!
//! Exposes an ETH RPC interface from an underlying Substrate light client or RPC url. This allows
//! allows using different Ethereum tools and libraries with Substrate chains, for reading the chain state,
//! indexing, and making generic RPC calls.
//!
//! The adapter currently supports most of the read methods from the JSON-RPC spec, and can be extended
//! to support more methods as needed.

mod adapter;
mod cache;
mod command;
mod server;
mod sub_client;
#[cfg(test)]
mod tests;
mod traits;
mod types;

use crate::sub_client::SubLightClient;
use clap::Parser;
use env_logger::{Builder, Env};
use log::info;

fn init_logger() {
    let env = Env::default()
        // .filter_or("RUST_LOG", "debug,smoldot=trace,subxt=debug")
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
