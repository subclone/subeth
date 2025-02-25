//! Command line interface for the Subeth RPC adapter.

use std::sync::Arc;

use futures::FutureExt;
use jsonrpsee::RpcModule;
use sc_service::config::RpcConfiguration;

use crate::{sub_client::SubLightClient, traits::EthApiServer};
use std::{
    cell::RefCell,
    sync::atomic::{AtomicUsize, Ordering},
};

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
    /// Rpc params
    #[allow(missing_docs)]
    #[clap(flatten)]
    rpc_params: sc_cli::RpcParams,
}

/// Blocks current thread until ctrl-c is received
pub async fn block_until_sigint() {
    let (ctrlc_send, ctrlc_oneshot) = futures::channel::oneshot::channel();
    let ctrlc_send_c = RefCell::new(Some(ctrlc_send));

    let running = Arc::new(AtomicUsize::new(0));
    ctrlc::set_handler(move || {
        let prev = running.fetch_add(1, Ordering::SeqCst);
        if prev == 0 {
            println!("Got interrupt, shutting down...");
            // Send sig int in channel to blocking task
            if let Some(ctrlc_send) = ctrlc_send_c.try_borrow_mut().unwrap().take() {
                ctrlc_send.send(()).expect("Error sending ctrl-c message");
            }
        } else {
            std::process::exit(0);
        }
    })
    .expect("Error setting Ctrl-C handler");

    ctrlc_oneshot.await.unwrap();
}

fn tokio_runtime() -> Result<tokio::runtime::Runtime, tokio::io::Error> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
}

pub async fn run(opts: Opts) -> anyhow::Result<()> {
    let chain_id = opts.chain_id;

    // figure out if we are relying on a smoldot node or RPC node
    let client = if let Some(chain_spec_path) = opts.chain_spec {
        // load the chain spec file
        log::info!("Loading chain spec from: {}", chain_spec_path);
        let chain_spec = std::fs::read_to_string(&chain_spec_path)?;

        SubLightClient::from_light_client(&chain_spec, chain_id).await?
    } else if let Some(url) = opts.url {
        // create a new RPC client
        SubLightClient::from_url(&url, chain_id).await?
    } else {
        // default to a Polkadot node
        let polkadot_spec = include_str!("../specs/polkadot.json");
        SubLightClient::from_light_client(polkadot_spec, chain_id).await?
    };

    let tokio_runtime = tokio_runtime()?;
    let tokio_handle = tokio_runtime.handle();

    let mut task_manager = sc_service::TaskManager::new(tokio_handle.clone(), None)?;

    let mut gen_rpc_module = RpcModule::new(());
    let rpc_module =
        crate::server::EthAdapter::new(client, vec![], Arc::new(task_manager.spawn_handle()))
            .into_rpc();

    gen_rpc_module
        .merge(rpc_module)
        .map_err(|e| anyhow::anyhow!(e))?;

    let addrs: Option<Vec<sc_service::config::RpcEndpoint>> = opts
        .rpc_params
        .rpc_addr(false, false, 8545)?
        .map(|addrs| addrs.into_iter().map(Into::into).collect());

    println!("Launching RPC server at: {:?}", addrs);

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
        tokio_handle,
        || Ok(gen_rpc_module.clone()),
        None,
    )?;

    task_manager.keep_alive(eth_rpc_handle);

    // // block until ctrl-c is received
    // let signals = tokio_runtime.block_on(async { sc_cli::Signals::capture() })?;
    // tokio_runtime.block_on(signals.run_until_signal(task_manager.future().fuse()))?;
    block_until_sigint().await;

    tokio_runtime.shutdown_timeout(std::time::Duration::from_secs(10));
    Ok(())
}
