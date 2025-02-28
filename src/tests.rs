use alloy_primitives::{Address, Bytes, B256, U256, U64};
use alloy_rpc_types_eth::{Block, BlockNumberOrTag, Index, Transaction, TransactionRequest};
use anyhow::Result;
use jsonrpsee::{core::client::ClientT, rpc_params, ws_client::WsClientBuilder};
use std::time::Duration;
use tokio::process::Command;

const CHAIN_SPEC: &str = "./specs/polkadot.json";
const WS_URL: &str = "ws://127.0.0.1:8545";
const POLKADOT_RPC: &str = "wss://polkadot.dotters.network";

async fn spawn_client(use_light_client: bool) -> Result<tokio::process::Child> {
    let binary_path = "/Users/dastansamat/.cargo/target/debug/subeth";
    if !std::path::Path::new(&binary_path).exists() {
        return Err(anyhow::anyhow!("Binary not found at {}", binary_path));
    }

    let mut command = Command::new(&binary_path);
    if use_light_client {
        command
            .arg("--chain-spec")
            .arg(CHAIN_SPEC)
            .arg("--max-retries")
            .arg("3");
    } else {
        command.arg("--url").arg(POLKADOT_RPC);
    }
    command.kill_on_drop(true);

    let mut child = command
        .spawn()
        .map_err(|e| anyhow::anyhow!("Failed to start client: {}", e))?;
    tokio::time::sleep(Duration::from_secs(60)).await; // Wait for client to stabilize

    match child.try_wait() {
        Ok(Some(status)) => Err(anyhow::anyhow!("Client exited with status: {}", status)),
        Ok(None) => Ok(child),
        Err(e) => Err(anyhow::anyhow!("Failed to check client status: {}", e)),
    }
}

async fn test_rpc_calls(ws_client: &jsonrpsee::ws_client::WsClient) -> Result<()> {
    // eth_protocolVersion
    let protocol_version: u64 = ws_client
        .request("eth_protocolVersion", rpc_params![])
        .await?;
    assert_eq!(protocol_version, 1);

    // eth_syncing
    let syncing: bool = ws_client.request("eth_syncing", rpc_params![]).await?;
    assert!(!syncing);

    // eth_accounts
    let accounts: Vec<Address> = ws_client.request("eth_accounts", rpc_params![]).await?;
    assert!(accounts.is_empty());

    // eth_blockNumber
    let block_number: U256 = ws_client.request("eth_blockNumber", rpc_params![]).await?;
    assert!(block_number >= U256::ZERO);

    // eth_chainId
    let chain_id: Option<U64> = ws_client.request("eth_chainId", rpc_params![]).await?;
    assert_eq!(chain_id, Some(U64::from(42)));

    // eth_getBlockByHash (fetch latest block first)
    let latest_block: Block = ws_client
        .request("eth_getBlockByNumber", rpc_params!["latest", false])
        .await?
        .expect("Latest block should exist");
    let block_by_hash: Option<Block> = ws_client
        .request(
            "eth_getBlockByHash",
            rpc_params![latest_block.header.hash, false],
        )
        .await?;
    assert!(block_by_hash.is_some());

    // eth_getBlockByNumber
    let block_by_number: Option<Block> = ws_client
        .request("eth_getBlockByNumber", rpc_params!["latest", false])
        .await?;
    assert!(block_by_number.is_some());

    // eth_getTransactionByBlockNumberAndIndex
    let tx: Option<Transaction> = ws_client
        .request(
            "eth_getTransactionByBlockNumberAndIndex",
            rpc_params!["latest", Index::from(0)],
        )
        .await?;
    if let Some(ref block) = block_by_number {
        if let Some(ref tx) = tx {
            assert!(block.transactions.as_full().unwrap().contains(tx));
        }
    }

    // eth_getBalance
    let balance: U256 = ws_client
        .request(
            "eth_getBalance",
            rpc_params!["0x0000000000000000000000000000000000000000", "latest"],
        )
        .await?;
    assert_eq!(balance, U256::ZERO);

    // eth_getStorageAt
    let storage: Vec<u8> = ws_client
        .request(
            "eth_getStorageAt",
            rpc_params![
                "0x62616c616e636573000000000000000000000000", // "balances" pallet
                "0x0000000000000000000000000000000000000000000000000000000000000000",
                "latest"
            ],
        )
        .await?;
    assert!(!storage.is_empty() || storage.is_empty()); // Just check it returns

    // eth_getTransactionCount
    let tx_count: U256 = ws_client
        .request(
            "eth_getTransactionCount",
            rpc_params!["0x0000000000000000000000000000000000000000", "latest"],
        )
        .await?;
    assert_eq!(tx_count, U256::ZERO);

    // eth_getCode
    let code: Bytes = ws_client
        .request(
            "eth_getCode",
            rpc_params!["0x62616c616e636573000000000000000000000000", "latest"],
        )
        .await?;
    assert!(code.starts_with(b"revert: balances") || code.is_empty());

    // eth_call
    let call_result: Bytes = ws_client
        .request(
            "eth_call",
            rpc_params![{
                "to": "0x62616c616e636573000000000000000000000000",
                "data": serde_json::to_vec(&StorageKey {
                    name: "TotalIssuance".to_string(),
                    keys: vec![]
                }).unwrap()
            }, "latest"],
        )
        .await?;
    assert!(call_result.len() > 0 || call_result.is_empty()); // Check it returns

    // eth_gasPrice
    let gas_price: U256 = ws_client.request("eth_gasPrice", rpc_params![]).await?;
    assert_eq!(gas_price, U256::from(1_000_000));

    // eth_isMining
    let is_mining: bool = ws_client.request("eth_mining", rpc_params![]).await?;
    assert!(!is_mining);

    // eth_hashrate
    let hashrate: U256 = ws_client.request("eth_hashrate", rpc_params![]).await?;
    assert_eq!(hashrate, U256::ZERO);

    // eth_getWork
    let work: [B256; 3] = ws_client.request("eth_getWork", rpc_params![]).await?;
    assert_eq!(work, [B256::ZERO, B256::ZERO, B256::ZERO]);

    // eth_submitHashrate
    let submit_hashrate: bool = ws_client
        .request(
            "eth_submitHashrate",
            rpc_params![U256::from(100), B256::random()],
        )
        .await?;
    assert!(!submit_hashrate);

    // eth_submitWork
    let submit_work: bool = ws_client
        .request(
            "eth_submitWork",
            rpc_params![0u64, B256::random(), B256::random()],
        )
        .await?;
    assert!(!submit_work);

    Ok(())
}

#[tokio::test]
async fn test_eth_rpc_light_client() -> Result<()> {
    let _client = spawn_client(true).await?;
    let ws_client = WsClientBuilder::default().build(WS_URL).await?;
    test_rpc_calls(&ws_client).await?;
    Ok(())
}

#[tokio::test]
async fn test_eth_rpc_url() -> Result<()> {
    let _client = spawn_client(false).await?;
    let ws_client = WsClientBuilder::default().build(WS_URL).await?;
    test_rpc_calls(&ws_client).await?;
    Ok(())
}
