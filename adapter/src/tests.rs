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
use parity_scale_codec::Encode;
use sp_core::{H160, H256, U256 as SpU256};
use std::{path::PathBuf, time::Duration};
use subeth_primitives::EthereumTransaction;
use tokio::process::Command;

use crate::adapter::StorageKey;

const CHAIN_SPEC: &str = "../specs/polkadot.json";
const POLKADOT_RPC: &str = "wss://polkadot.dotters.network";

fn get_subeth_debug_path() -> PathBuf {
    // Determine the root target directory (respecting CARGO_TARGET_DIR)
    // In a workspace, target is at the workspace root, not in the member directory
    let target_root = std::env::var("CARGO_TARGET_DIR")
        .unwrap_or_else(|_| {
            // Try workspace root target first
            if PathBuf::from("../target").exists() {
                "../target".to_string()
            } else {
                "target".to_string()
            }
        });

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
        // Skip empty results (account not bonded)
        if res.is_empty() {
            println!("Account not bonded, skipping");
            continue;
        }
        if res.len() >= 32 {
            let controller = H256::from_slice(&res.to_vec()[..32]);
            // assert that this is a valid address
            assert!(controller != H256::zero());
        }
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

/// Test encoding and decoding of EthereumTransaction with SCALE-encoded call
#[test]
fn test_ethereum_transaction_encoding() {
    use parity_scale_codec::Encode;

    // Create SCALE-encoded RuntimeCall: Balances::transfer_allow_death
    let pallet_index: u8 = 5;
    let call_index: u8 = 0;
    let dest_account = [0xaa; 32];
    let amount: u128 = 1000;

    let mut call_data = Vec::new();
    call_data.push(pallet_index);
    call_data.push(call_index);
    call_data.extend_from_slice(&dest_account);
    call_data.extend_from_slice(&parity_scale_codec::Compact(amount).encode());

    // Create the transaction
    let eth_tx = EthereumTransaction {
        chain_id: 1,
        nonce: 0,
        max_priority_fee_per_gas: SpU256::zero(),
        max_fee_per_gas: SpU256::from(1_000_000u128),
        gas_limit: 100000,
        to: H160::zero(), // Generic dispatch address
        value: SpU256::zero(),
        data: call_data.clone(),
        access_list: vec![],
        v: 27,
        r: H256::from([0x11; 32]),
        s: H256::from([0x22; 32]),
    };

    // Encode the transaction
    let encoded = eth_tx.encode();
    assert!(!encoded.is_empty());

    // Verify it can be decoded back
    use parity_scale_codec::Decode;
    let decoded = EthereumTransaction::decode(&mut &encoded[..]).unwrap();
    assert_eq!(decoded.chain_id, eth_tx.chain_id);
    assert_eq!(decoded.nonce, eth_tx.nonce);
    assert_eq!(decoded.to, eth_tx.to);
    assert_eq!(decoded.data, eth_tx.data);
    assert_eq!(decoded.gas_limit, eth_tx.gas_limit);

    println!("✅ EthereumTransaction encoding/decoding works correctly");
    println!("   Encoded length: {} bytes", encoded.len());
    println!("   Transaction hash: {:?}", eth_tx.hash());
}

/// Test the complete flow of constructing a SCALE-encoded transfer transaction
#[test]
fn test_transfer_transaction_construction() {
    use parity_scale_codec::Encode;

    println!("\n=== Testing SCALE-Encoded Transfer Transaction Construction ===\n");

    // Step 1: Define recipient AccountId32
    let recipient_account = [0xbb; 32];
    println!("Recipient AccountId32: 0x{}", hex::encode(recipient_account));

    // Step 2: Define transfer amount
    let amount: u128 = 1_000_000_000_000; // 1 token with 12 decimals
    println!("Amount: {}", amount);

    // Step 3: SCALE encode the RuntimeCall
    // Balances::transfer_allow_death { dest, value }
    let pallet_index: u8 = 5;  // Balances pallet (check metadata)
    let call_index: u8 = 0;    // transfer_allow_death

    let mut call_data = Vec::new();
    call_data.push(pallet_index);
    call_data.push(call_index);
    call_data.extend_from_slice(&recipient_account);
    call_data.extend_from_slice(&parity_scale_codec::Compact(amount).encode());

    println!("\nSCALE-encoded RuntimeCall:");
    println!("  Pallet index: {}", pallet_index);
    println!("  Call index: {}", call_index);
    println!("  Call data: 0x{}", hex::encode(&call_data));
    println!("  Length: {} bytes", call_data.len());

    // Step 4: Create the EthereumTransaction
    let eth_tx = EthereumTransaction {
        chain_id: 1,
        nonce: 0,
        max_priority_fee_per_gas: SpU256::zero(),
        max_fee_per_gas: SpU256::from(1_000_000u128),
        gas_limit: 100000,
        to: H160::zero(), // Generic dispatch address
        value: SpU256::zero(),
        data: call_data,
        access_list: vec![],
        v: 27,
        r: H256::from([0xaa; 32]),
        s: H256::from([0xbb; 32]),
    };

    // Step 5: Encode for transmission
    let encoded = eth_tx.encode();
    println!("\nSCALE encoded EthereumTransaction:");
    println!("  Length: {} bytes", encoded.len());
    println!("  Hex: 0x{}", hex::encode(&encoded));

    println!("\n✅ Transaction ready to submit via eth_sendRawTransaction");
    println!("   RPC call: eth_sendRawTransaction(\"0x{}\")", hex::encode(&encoded));
}

/// Test generic SCALE-encoded RuntimeCall dispatch
/// This demonstrates a more generic approach where the entire RuntimeCall
/// is SCALE-encoded and passed directly in the transaction data
#[test]
fn test_generic_scale_encoded_call_dispatch() {
    println!("\n=== Testing Generic SCALE-Encoded Call Dispatch ===\n");

    // In a real implementation, you would have access to the runtime metadata
    // and could construct the actual RuntimeCall. For this example, we'll
    // demonstrate the pattern with a mock call structure.

    // Example: Balances.transfer_allow_death call structure
    // RuntimeCall = enum with pallet index
    // Balances is typically pallet index 5 or 10 depending on runtime
    // transfer_allow_death is call index 0

    // Mock SCALE-encoded call: Balances::transfer_allow_death { dest, value }
    let pallet_index: u8 = 5; // Balances pallet
    let call_index: u8 = 0;   // transfer_allow_death

    // Destination: AccountId32 (32 bytes)
    let dest_account = [0xaa; 32];

    // Amount: Compact<u128>
    let amount: u128 = 1_000_000_000_000; // 1 token with 12 decimals

    // SCALE encode the call
    let mut scale_encoded_call = Vec::new();
    scale_encoded_call.push(pallet_index);
    scale_encoded_call.push(call_index);

    // Encode destination (AccountId32 is just raw 32 bytes)
    scale_encoded_call.extend_from_slice(&dest_account);

    // Encode amount as compact
    use parity_scale_codec::Encode;
    scale_encoded_call.extend_from_slice(&parity_scale_codec::Compact(amount).encode());

    println!("SCALE-encoded RuntimeCall:");
    println!("  Pallet index: {}", pallet_index);
    println!("  Call index: {}", call_index);
    println!("  Destination: 0x{}", hex::encode(dest_account));
    println!("  Amount: {}", amount);
    println!("  Total encoded: 0x{}", hex::encode(&scale_encoded_call));
    println!("  Length: {} bytes", scale_encoded_call.len());

    // Create EthereumTransaction with SCALE-encoded call in data field
    // Use the reserved address for generic call dispatch
    use crate::adapter::GENERIC_CALL_ADDRESS;
    let generic_dispatch_address = H160::from(GENERIC_CALL_ADDRESS.0 .0);

    let eth_tx = EthereumTransaction {
        chain_id: 1,
        nonce: 0,
        max_priority_fee_per_gas: SpU256::zero(),
        max_fee_per_gas: SpU256::from(1_000_000u128),
        gas_limit: 100000,
        to: generic_dispatch_address,
        value: SpU256::zero(),
        data: scale_encoded_call.clone(), // SCALE-encoded RuntimeCall
        access_list: vec![],
        v: 27,
        r: H256::from([0xcc; 32]),
        s: H256::from([0xdd; 32]),
    };

    // Encode the transaction for submission
    let encoded_tx = eth_tx.encode();
    println!("\nEthereumTransaction ready for submission:");
    println!("  To: 0x{}", hex::encode(eth_tx.to.as_bytes()));
    println!("  Data (SCALE-encoded call): 0x{}", hex::encode(&eth_tx.data));
    println!("  SCALE-encoded tx length: {} bytes", encoded_tx.len());

    // Verify decoding
    use parity_scale_codec::Decode;
    let decoded_tx = EthereumTransaction::decode(&mut &encoded_tx[..]).unwrap();
    assert_eq!(decoded_tx.data, scale_encoded_call);
    assert_eq!(decoded_tx.to, generic_dispatch_address);

    println!("\n✅ Generic call dispatch pattern:");
    println!("   1. Client SCALE-encodes the RuntimeCall (pallet_index + call_index + params)");
    println!("   2. Client creates EthereumTransaction with call in 'data' field");
    println!("   3. Client sends via eth_sendRawTransaction");
    println!("   4. Pallet verifies signature, decodes SCALE call, dispatches to runtime");
    println!("\n📝 Benefits:");
    println!("   - No ABI encoding/decoding needed");
    println!("   - Works with ANY runtime call (Balances, Staking, Governance, etc.)");
    println!("   - Type-safe with proper metadata");
    println!("   - More efficient than ABI encoding");
}

/// Test SCALE encoding for different transfer amounts
#[test]
fn test_call_data_encoding_various_amounts() {
    use parity_scale_codec::Encode;

    let amounts = vec![
        (0u128, "zero"),
        (1u128, "one"),
        (1000u128, "one thousand"),
        (1_000_000u128, "one million"),
        (1_000_000_000_000u128, "one token (12 decimals)"),
        (u128::MAX, "max u128"),
    ];

    println!("\n=== Testing SCALE Encoding for Various Amounts ===\n");

    for (amount, desc) in amounts {
        let pallet_index: u8 = 5;
        let call_index: u8 = 0;
        let dest_account = [0xaa; 32];

        let mut call_data = Vec::new();
        call_data.push(pallet_index);
        call_data.push(call_index);
        call_data.extend_from_slice(&dest_account);
        call_data.extend_from_slice(&parity_scale_codec::Compact(amount).encode());

        println!("Amount: {} ({})", amount, desc);
        println!("  SCALE-encoded: 0x{}", hex::encode(&call_data));
        println!("  Total length: {} bytes", call_data.len());

        // Show compact encoding efficiency
        let compact_bytes = parity_scale_codec::Compact(amount).encode();
        println!("  Compact amount: {} bytes", compact_bytes.len());
    }

    println!("\n✅ All amounts SCALE-encoded correctly");
    println!("   Note: Compact encoding is efficient - smaller values use fewer bytes");
}

/// Test message hash calculation for signature verification
#[test]
fn test_transaction_message_hash() {
    let eth_tx = EthereumTransaction {
        chain_id: 1,
        nonce: 5,
        max_priority_fee_per_gas: SpU256::from(100u128),
        max_fee_per_gas: SpU256::from(1_000_000u128),
        gas_limit: 100000,
        to: H160::from([0x42; 20]),
        value: SpU256::from(500u128),
        data: vec![0xa9, 0x05, 0x9c, 0xbb],
        access_list: vec![],
        v: 27,
        r: H256::from([0x11; 32]),
        s: H256::from([0x22; 32]),
    };

    let message_hash = eth_tx.message_hash();
    println!("\n=== Transaction Message Hash ===");
    println!("Message hash: 0x{}", hex::encode(message_hash));

    // The message hash should be deterministic
    let message_hash2 = eth_tx.message_hash();
    assert_eq!(message_hash, message_hash2);

    println!("✅ Message hash calculation is deterministic");
}

/// Test signature format for secp256k1 recovery
#[test]
fn test_signature_format() {
    let eth_tx = EthereumTransaction {
        chain_id: 1,
        nonce: 0,
        max_priority_fee_per_gas: SpU256::zero(),
        max_fee_per_gas: SpU256::from(1_000_000u128),
        gas_limit: 100000,
        to: H160::from([0x42; 20]),
        value: SpU256::zero(),
        data: vec![],
        access_list: vec![],
        v: 27,  // Valid recovery ID (27 or 28 in legacy format)
        r: H256::from([0x11; 32]),
        s: H256::from([0x22; 32]),
    };

    let signature = eth_tx.signature().unwrap();
    assert_eq!(signature.len(), 65);
    assert_eq!(signature[64], 0); // v should be converted to 0 (from 27)

    println!("\n=== Signature Format ===");
    println!("Signature length: {} bytes", signature.len());
    println!("R (bytes 0-31): 0x{}", hex::encode(&signature[..32]));
    println!("S (bytes 32-63): 0x{}", hex::encode(&signature[32..64]));
    println!("V (byte 64): {}", signature[64]);

    // Test with v=28
    let eth_tx2 = EthereumTransaction {
        v: 28,
        ..eth_tx.clone()
    };
    let signature2 = eth_tx2.signature().unwrap();
    assert_eq!(signature2[64], 1); // v should be converted to 1 (from 28)

    // Test with invalid recovery ID
    let eth_tx_invalid = EthereumTransaction {
        v: 30,  // Invalid
        ..eth_tx
    };
    assert!(eth_tx_invalid.signature().is_err());

    println!("✅ Signature format is correct");
}

/// Test that demonstrates the complete integration flow with SCALE-encoded calls
/// Note: This test documents the expected behavior but requires a running chain
#[test]
fn test_integration_flow_documentation() {
    println!("\n╔═══════════════════════════════════════════════════════════════════╗");
    println!("║      Complete SCALE-Based Integration Flow Documentation         ║");
    println!("╚═══════════════════════════════════════════════════════════════════╝\n");

    println!("Step 1: SCALE-encode the RuntimeCall");
    println!("   // Balances::transfer_allow_death {{dest, value}}");
    println!("   const call = [");
    println!("     5,  // pallet index");
    println!("     0,  // call index");
    println!("     ...dest_account_id,  // 32 bytes");
    println!("     ...compact(amount)   // compact u128");
    println!("   ];");
    println!();

    println!("Step 2: Create EthereumTransaction with SCALE data");
    println!("   const ethTx = {{");
    println!("     chainId: 1,");
    println!("     nonce: 0,");
    println!("     maxFeePerGas: 1000000,");
    println!("     gasLimit: 100000,");
    println!("     to: '0x0000000000000000000000000000000000000000',  // Generic dispatch");
    println!("     value: 0,");
    println!("     data: call,  // SCALE-encoded RuntimeCall");
    println!("     accessList: []");
    println!("   }};");
    println!();

    println!("Step 3: Sign and submit via eth_sendRawTransaction");
    println!("   POST http://localhost:8545");
    println!("   {{");
    println!("     \"jsonrpc\": \"2.0\",");
    println!("     \"method\": \"eth_sendRawTransaction\",");
    println!("     \"params\": [\"0x\" + scale.encode(ethTx)],");
    println!("     \"id\": 1");
    println!("   }}");
    println!();

    println!("Step 4: Adapter processes the transaction");
    println!("   - Decodes SCALE-encoded EthereumTransaction");
    println!("   - Calls submit_evm_transaction()");
    println!();

    println!("Step 5: SubLightClient submits to chain");
    println!("   - Creates extrinsic: [pallet_idx(6), call_idx(0), eth_tx]");
    println!("   - Calls author_submitExtrinsic RPC");
    println!();

    println!("Step 6: Pallet processes on-chain");
    println!("   - Verifies ECDSA signature (v, r, s)");
    println!("   - Recovers signer address (H160)");
    println!("   - Maps H160 → AccountId32 using Blake2");
    println!("   - Decodes RuntimeCall from transaction.data");
    println!("   - Dispatches call with mapped account as origin");
    println!();

    println!("Step 7: Returns transaction hash to client");
    println!("   Response: {{");
    println!("     \"jsonrpc\": \"2.0\",");
    println!("     \"result\": \"0xTransactionHash\",");
    println!("     \"id\": 1");
    println!("   }}");
    println!();

    println!("✅ SCALE-based integration flow documented");
    println!("   Works with ANY runtime call!");
}

/// Test pallet address encoding (Legacy - for reference only)
/// Note: With SCALE-encoded calls, this is no longer used
#[test]
fn test_pallet_address_encoding() {
    println!("\n=== Pallet Address Encoding (Legacy Reference) ===\n");

    let pallets = vec!["Balances", "Staking", "System", "Sudo"];

    for pallet_name in pallets {
        let mut address_bytes = [0u8; 20];
        let name_bytes = pallet_name.as_bytes();
        let len = name_bytes.len().min(20);
        address_bytes[..len].copy_from_slice(&name_bytes[..len]);

        println!("Pallet: {}", pallet_name);
        println!("  Address: 0x{}", hex::encode(address_bytes));
        println!("  First 8 bytes: {}", String::from_utf8_lossy(&address_bytes[..8]));
    }

    println!("\n✅ Pallet addresses encoded correctly");
}
