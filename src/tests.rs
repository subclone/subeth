use alloy_primitives::{hex, Address, Bytes, TxKind, U256, U64};
use alloy_rpc_types_eth::{
    Block, BlockId, Index, Transaction, TransactionInput, TransactionRequest,
};
use anyhow::Result;
use futures::future::join_all;
use jsonrpsee::{
    core::{client::ClientT, params::ArrayParams},
    rpc_params,
    ws_client::WsClientBuilder,
};
use sp_core::H256;
use std::{path::PathBuf, time::Duration};
use tokio::process::Command;

use crate::adapter::StorageKey;

const CHAIN_SPEC: &str = "./specs/polkadot.json";
const POLKADOT_RPC: &str = "wss://polkadot.dotters.network";

fn get_subeth_debug_path() -> PathBuf {
    // Determine the root target directory (respecting CARGO_TARGET_DIR)
    let target_root = std::env::var("CARGO_TARGET_DIR").unwrap_or_else(|_| "target".to_string());

    // Construct the path to the debug directory
    let mut debug_dir_path = PathBuf::from(target_root);
    debug_dir_path.push("debug"); // Append 'debug'

    // Append the binary name
    debug_dir_path.push("subeth"); // Append the binary name 'subeth'

    debug_dir_path // Return the full path
}

async fn spawn_client(use_light_client: bool) -> Result<tokio::process::Child> {
    let binary_path = get_subeth_debug_path();

    if !std::path::Path::new(&binary_path).exists() {
        return Err(anyhow::anyhow!("Binary not found at {:?}", binary_path));
    }

    let mut command = Command::new(&binary_path);
    if use_light_client {
        command
            .arg("--chain-spec")
            .arg(CHAIN_SPEC)
            .arg("--max-retries")
            .arg("3")
            .arg("--rpc-port")
            .arg("8546");
    } else {
        command.arg("--url").arg(POLKADOT_RPC);
    }
    command.kill_on_drop(true);

    let mut child = command
        .spawn()
        .map_err(|e| anyhow::anyhow!("Failed to start client: {}", e))?;

    tokio::time::sleep(Duration::from_secs(10)).await; // Wait for client to stabilize

    if use_light_client {
        tokio::time::sleep(Duration::from_secs(40)).await; // Wait for client to stabilize
    }

    match child.try_wait() {
        Ok(Some(status)) => Err(anyhow::anyhow!("Client exited with status: {}", status)),
        Ok(None) => Ok(child),
        Err(e) => Err(anyhow::anyhow!("Failed to check client status: {}", e)),
    }
}

async fn test_base_rpc_calls(ws_client: &jsonrpsee::ws_client::WsClient) -> Result<()> {
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
    assert!(block_number > U256::ZERO);

    // eth_chainId
    let chain_id: Option<U64> = ws_client.request("eth_chainId", rpc_params![]).await?;
    assert_eq!(chain_id, Some(U64::from(42)));

    // eth_getBlockByHash (fetch latest block first)
    let latest_block = ws_client
        .request::<Option<Block>, ArrayParams>("eth_getBlockByNumber", rpc_params!["latest", false])
        .await?;
    let block_by_hash = ws_client
        .request::<Option<Block>, ArrayParams>(
            "eth_getBlockByHash",
            rpc_params![latest_block.clone().unwrap().header.hash, false],
        )
        .await?;
    assert!(block_by_hash.is_some());

    // eth_getBlockByNumber
    let block_number =
        hex::encode_prefixed((latest_block.clone().unwrap().header.number).to_be_bytes());

    // if it's light client, wait a bit for the block to be finalized
    let block_by_number = ws_client
        .request::<Option<Block>, ArrayParams>(
            "eth_getBlockByNumber",
            rpc_params![block_number.clone(), false],
        )
        .await?;
    assert!(block_by_number.is_some());

    // eth_getTransactionByBlockNumberAndIndex
    let tx = ws_client
        .request::<Option<Transaction>, ArrayParams>(
            "eth_getTransactionByBlockNumberAndIndex",
            rpc_params![block_number, Index::from(0)],
        )
        .await?;
    if let Some(ref block) = block_by_number {
        if let Some(ref tx) = tx {
            assert!(block
                .transactions
                .clone()
                .into_transactions_vec()
                .contains(tx));
        }
    }

    println!("Transaction: {:?}", tx);
    // eth_getBalance
    let balance = ws_client
        .request::<U256, ArrayParams>(
            "eth_getBalance",
            rpc_params![
                "0x0000000000000000000000000000000000000000",
                hex::encode_prefixed(block_by_hash.unwrap().header.number.to_be_bytes())
            ],
        )
        .await?;
    assert_eq!(balance, U256::ZERO);

    println!("Balance: {:?}", balance);

    // println!("Balance: {:?}", balance);
    // eth_getStorageAt
    let storage: Vec<u8> = ws_client
        .request::<Vec<u8>, ArrayParams>(
            "eth_getStorageAt",
            rpc_params![
                "0x62616c616e636573000000000000000000000000", // "balances" pallet
                "0x0000000000000000000000000000000000000000000000000000000000000000",
                None::<BlockId>
            ],
        )
        .await?;
    assert!(!storage.is_empty() || storage.is_empty()); // Just check it returns

    println!("Storage: {:?}", storage);
    // eth_getTransactionCount
    let tx_count = ws_client
        .request::<U256, ArrayParams>(
            "eth_getTransactionCount",
            rpc_params!["0x0000000000000000000000000000000000000000", "latest"],
        )
        .await?;
    assert_eq!(tx_count, U256::ZERO);
    println!("Transaction count: {:?}", tx_count);

    // eth_getCode
    let code = ws_client
        .request::<Bytes, ArrayParams>(
            "eth_getCode",
            rpc_params!["0x42616c616e636573000000000000000000000000", "latest"],
        )
        .await?;
    assert!(code.starts_with(b"revert: Balances"));

    // eth_call
    let eth_call_input = serde_json::to_string(&StorageKey {
        name: "TotalIssuance".to_string(),
        keys: vec![],
    })
    .unwrap();
    let input_bytes = Bytes::from(eth_call_input.into_bytes());

    println!("Input bytes: {:?}", input_bytes);
    let eth_call_request = TransactionRequest {
        from: None,
        to: Some(TxKind::Call(
            "0x42616c616e636573000000000000000000000000"
                .parse()
                .unwrap(),
        )),
        input: TransactionInput {
            input: Some(input_bytes),
            data: None,
        },
        ..Default::default()
    };

    let call_result = ws_client
        .request::<Bytes, ArrayParams>("eth_call", rpc_params![eth_call_request, "latest"])
        .await?;
    // Substrate stores in little endian
    let total_issuance = u128::from_le_bytes(call_result.to_vec()[..].try_into().unwrap());
    assert!(total_issuance > 0); // Check it returns

    // Get staking storage entries
    let bonded_accounts = [
        "0x28ee403d79d6fb7a1d3eb608ba1655ae12913e478176167307ee5bf81310e485",
        "0x585a40fe9cba07338d4f3bb714a6107f29a872f02c4b041904c8a265095f6581",
        "0x31b0277dc6d1dd663f8e36fbe057483cf0648b69f296f61994c5bf10994e6732",
    ]
    .map(|acc| {
        let acc_bytes = hex::decode(acc).unwrap();
        let storage_key = StorageKey {
            name: "Bonded".to_string(),
            keys: vec![acc_bytes],
        };
        let storage_key_str = serde_json::to_string(&storage_key).unwrap();
        Bytes::from(storage_key_str.into_bytes())
    });

    let bonded_results = bonded_accounts
        .iter()
        .map(|input| {
            let eth_call_request = TransactionRequest {
                from: None,
                to: Some(TxKind::Call(
                    "0x5374616b696e6700000000000000000000000000"
                        .parse()
                        .unwrap(),
                )),
                input: TransactionInput {
                    input: Some(input.clone()),
                    data: None,
                },
                ..Default::default()
            };

            ws_client
                .request::<Bytes, ArrayParams>("eth_call", rpc_params![eth_call_request, "latest"])
        })
        .collect::<Vec<_>>();

    for res in join_all(bonded_results).await {
        let res = res?;
        println!("Bonded result: {:?}", res);
        let controller = H256::from_slice(&res.to_vec()[..32]);
        // assert that this is a valid address
        assert!(controller != H256::zero());
    }

    // eth_gasPrice
    let gas_price: U256 = ws_client.request("eth_gasPrice", rpc_params![]).await?;
    assert_eq!(gas_price, U256::from(1_000_000));

    Ok(())
}

#[tokio::test]
async fn test_eth_rpc_light_client() -> Result<()> {
    let _client = spawn_client(true).await?;
    let url: &str = "ws://127.0.0.1:8546";
    let ws_client = WsClientBuilder::default().build(url).await?;
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
    assert!(block_number > U256::ZERO);

    // eth_chainId
    let chain_id: Option<U64> = ws_client.request("eth_chainId", rpc_params![]).await?;
    assert_eq!(chain_id, Some(U64::from(42)));

    // eth_getBlockByHash (fetch latest block first)
    let latest_block = ws_client
        .request::<Option<Block>, ArrayParams>("eth_getBlockByNumber", rpc_params!["latest", false])
        .await?;
    let block_by_hash = ws_client
        .request::<Option<Block>, ArrayParams>(
            "eth_getBlockByHash",
            rpc_params![latest_block.clone().unwrap().header.hash, false],
        )
        .await?;
    assert!(block_by_hash.is_some());

    // eth_getBlockByNumber
    let block_number =
        hex::encode_prefixed((latest_block.clone().unwrap().header.number).to_be_bytes());

    // if it's light client, wait a bit for the block to be finalized
    let block_by_number = ws_client
        .request::<Option<Block>, ArrayParams>(
            "eth_getBlockByNumber",
            rpc_params![block_number.clone(), false],
        )
        .await?;
    assert!(block_by_number.is_some());

    Ok(())
}

#[tokio::test]
async fn test_eth_rpc_url() -> Result<()> {
    let _client = spawn_client(false).await?;
    let url: &str = "ws://127.0.0.1:8545";
    let ws_client = WsClientBuilder::default().build(url).await?;
    test_base_rpc_calls(&ws_client).await?;
    Ok(())
}
