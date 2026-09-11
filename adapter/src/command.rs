//! Command line interface for the Subeth RPC adapter.

use std::sync::Arc;

use jsonrpsee::RpcModule;
use sc_service::config::RpcConfiguration;

use crate::{
    sub_client::SubLightClient,
    traits::{EthApiServer, EthPubSubApiServer},
};

const DEFAULT_URL: &str = "wss://rpc.ibp.network/polkadot";

#[derive(clap::Parser, Debug)]
#[clap(version = "0.1", author = "Subeth")]
pub struct Opts {
    /// Chain specification file
    #[clap(short, long)]
    chain_spec: Option<String>,
    /// Remote node URL
    #[clap(short, long)]
    url: Option<String>,
    /// Chain ID
    #[clap(long, default_value = "42")]
    chain_id: u64,
    /// Maximum retries for client initialization
    #[clap(long, default_value = "3")]
    max_retries: u32,
    /// Rpc params
    #[allow(missing_docs)]
    #[clap(flatten)]
    rpc_params: sc_cli::RpcParams,
}

/// Waits until ctrl-c is received
async fn block_until_sigint() -> anyhow::Result<()> {
    tokio::signal::ctrl_c().await?;
    log::info!("Got interrupt, shutting down...");
    Ok(())
}

pub async fn run(opts: Opts) -> anyhow::Result<()> {
    let chain_id = opts.chain_id;

    let chain_spec = match &opts.chain_spec {
        Some(path) => {
            log::info!("Loading chain spec from: {}", path);
            Some(std::fs::read_to_string(path)?)
        }
        None => None,
    };
    let url = opts.url.as_deref().unwrap_or(DEFAULT_URL);

    // figure out if we are relying on a smoldot node or RPC node, retrying on failure
    let mut attempt = 0;
    let client = loop {
        attempt += 1;
        let result = match &chain_spec {
            Some(spec) => SubLightClient::from_light_client(spec, chain_id, None).await,
            None => SubLightClient::from_url(url, chain_id, None).await,
        };
        match result {
            Ok(client) => break client,
            Err(e) if attempt <= opts.max_retries => {
                log::warn!(
                    "Failed to connect (attempt {}/{}): {:?}",
                    attempt,
                    opts.max_retries + 1,
                    e
                );
                tokio::time::sleep(std::time::Duration::from_secs(2u64.pow(attempt))).await;
            }
            Err(e) => return Err(e),
        }
    };

    log::info!("Connected to chain: {}", client.chain_id());

    // Reuse the runtime started by `#[tokio::main]`; creating (and later dropping) a nested
    // runtime from within an async context panics on shutdown.
    let tokio_handle = tokio::runtime::Handle::current();

    let mut task_manager = sc_service::TaskManager::new(tokio_handle.clone(), None)?;

    let mut gen_rpc_module = RpcModule::new(());
    let adapter =
        crate::server::EthAdapter::new(client, vec![], Arc::new(task_manager.spawn_handle()));

    gen_rpc_module
        .merge(EthPubSubApiServer::into_rpc(adapter.clone()))
        .map_err(|e| anyhow::anyhow!(e))?;
    gen_rpc_module
        .merge(EthApiServer::into_rpc(adapter))
        .map_err(|e| anyhow::anyhow!(e))?;

    let rpc_port = opts.rpc_params.rpc_port.unwrap_or(8545);
    log::info!("RPC port: {}", rpc_port);

    let addrs: Option<Vec<sc_service::config::RpcEndpoint>> = opts
        .rpc_params
        .rpc_addr(false, false, rpc_port)?
        .map(|addrs| addrs.into_iter().map(Into::into).collect());

    log::info!("Launching RPC server at: {:?}", addrs);

    let rpc_params = opts.rpc_params;

    let rpc_config = RpcConfiguration {
        addr: addrs,
        methods: rpc_params.rpc_methods.into(),
        max_connections: rpc_params.rpc_max_connections,
        cors: rpc_params.rpc_cors(true)?,
        max_request_size: rpc_params.rpc_max_request_size,
        max_response_size: rpc_params.rpc_max_response_size,
        id_provider: None,
        max_subs_per_conn: rpc_params.rpc_max_subscriptions_per_connection,
        port: rpc_params.rpc_port.unwrap_or(8545),
        message_buffer_capacity: rpc_params.rpc_message_buffer_capacity_per_connection,
        batch_config: rpc_params.rpc_batch_config()?,
        rate_limit: rpc_params.rpc_rate_limit,
        rate_limit_whitelisted_ips: rpc_params.rpc_rate_limit_whitelisted_ips,
        rate_limit_trust_proxy_headers: rpc_params.rpc_rate_limit_trust_proxy_headers,
    };
    let eth_rpc_handle = sc_service::start_rpc_servers(
        &rpc_config,
        None,
        &tokio_handle,
        || Ok(gen_rpc_module.clone()),
        None,
    )?;

    task_manager.keep_alive(eth_rpc_handle);

    block_until_sigint().await?;

    Ok(())
}
