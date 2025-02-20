//! Command line interface for the Subeth RPC adapter.

use crate::sub_client::SubLightClient;
use clap::Parser;

#[derive(clap::Parser)]
#[clap(version = "0.1", author = "Subeth")]
struct Opts {
    /// Chain specification file
    #[clap(short, long)]
    chain_spec: String,
}
