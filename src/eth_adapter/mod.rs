//! ETH Rpc adapter
use super::SubLightClient;
use eth_api::EthAdapter;
use jsonrpsee::server::Server;

mod eth_api;

/// Runs the ETH RPC adapter server
async fn run_server(client: SubLightClient, port: u16) -> Result<(), Box<dyn std::error::Error>> {
    let eth_adapter = EthAdapter::new(client);

    let server = Server::builder().build(format!("127.0.0.1:{port}")).await?;
    let addr = server.local_addr()?;

    let server_handle = server.start(eth_adapter.into_rpc());

    tokio::spawn(server_handle.stopped());

    println!("Server started at: {}", addr);

    Ok(())
}
