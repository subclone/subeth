//! Command line interface for the Subeth RPC adapter.

use crate::smoldot::SubLightClient;
use clap::Parser;

#[derive(Parser)]
#[clap(version = "0.1", author = "Subeth")]
struct Opts {
    /// Chain specification file
    #[clap(short, long)]
    chain_spec: String,
}
