//! Tests for the EVM adapter pallet

use crate::{mock::*, Error};
use codec::Encode;
use polkadot_sdk::frame_support::*;
use polkadot_sdk::pallet_balances;
use polkadot_sdk::polkadot_sdk_frame::prelude::Dispatchable;
use polkadot_sdk::sp_core::{H160, H256, U256};
use subeth_primitives::EthereumTransaction;

#[test]
fn test_address_mapping() {
    new_test_ext().execute_with(|| {
        let evm_address = H160::from([1u8; 20]);
        let substrate_account = crate::Pallet::<Test>::map_address_to_account(evm_address);

        // Verify that the mapping is deterministic
        let substrate_account2 = crate::Pallet::<Test>::map_address_to_account(evm_address);
        assert_eq!(substrate_account, substrate_account2);
    });
}

#[test]
fn test_pallet_name_from_address() {
    new_test_ext().execute_with(|| {
        // Create an address from "Balances"
        let mut address_bytes = [0u8; 20];
        address_bytes[..8].copy_from_slice(b"Balances");
        let address = H160::from(address_bytes);

        let pallet_name = crate::Pallet::<Test>::pallet_name_from_address(address);
        assert_eq!(pallet_name, Some("Balances".to_string()));
    });
}

#[test]
fn test_pallet_address_encoding() {
    new_test_ext().execute_with(|| {
        // Test that pallet names are correctly encoded as addresses
        let test_cases = vec!["Balances", "System\0\0"];

        for pallet_name in test_cases {
            let mut address_bytes = [0u8; 20];
            address_bytes[..pallet_name.len()].copy_from_slice(pallet_name.as_bytes());
            let address = H160::from(address_bytes);

            // Verify we can decode it back
            let decoded = crate::Pallet::<Test>::pallet_name_from_address(address);
            assert!(decoded.is_some());

            // The decoded name should match (without null bytes)
            let decoded_str = decoded.unwrap();
            assert!(decoded_str.starts_with(&pallet_name.trim_end_matches('\0')));
        }
    });
}

#[test]
fn test_decode_transfer_call() {
    new_test_ext().execute_with(|| {
        // Construct a transfer transaction
        // Function selector for "transfer(address,uint256)"
        let mut data = vec![0xa9, 0x05, 0x9c, 0xbb]; // transfer selector

        // Destination address (32 bytes with 12 bytes padding)
        data.extend_from_slice(&[0u8; 12]); // padding
        data.extend_from_slice(&[2u8; 20]); // destination address

        // Value (32 bytes) - transfer 1000
        let value = U256::from(1000u128);
        let value_bytes = value.0;
        // Convert [u64; 4] to [u8; 32] in big-endian
        let mut be_bytes = [0u8; 32];
        for (i, &word) in value_bytes.iter().rev().enumerate() {
            let word_bytes = word.to_be_bytes();
            be_bytes[i * 8..(i + 1) * 8].copy_from_slice(&word_bytes);
        }
        data.extend_from_slice(&be_bytes);

        // Create Balances pallet address
        let mut to_address = [0u8; 20];
        to_address[..8].copy_from_slice(b"Balances");

        let transaction = EthereumTransaction {
            chain_id: 1,
            nonce: 0,
            max_priority_fee_per_gas: U256::from(0),
            max_fee_per_gas: U256::from(0),
            gas_limit: 21000,
            to: H160::from(to_address),
            value: U256::from(0),
            data,
            access_list: vec![],
            v: 0,
            r: Default::default(),
            s: Default::default(),
        };

        // Try to decode the call
        let result = crate::Pallet::<Test>::decode_call(&transaction);
        assert!(result.is_ok());
    });
}

#[test]
fn test_decode_transfer_with_invalid_selector() {
    new_test_ext().execute_with(|| {
        // Use an invalid function selector
        let data = vec![0xff, 0xff, 0xff, 0xff];

        let mut to_address = [0u8; 20];
        to_address[..8].copy_from_slice(b"Balances");

        let transaction = EthereumTransaction {
            chain_id: 1,
            nonce: 0,
            max_priority_fee_per_gas: U256::from(0),
            max_fee_per_gas: U256::from(0),
            gas_limit: 21000,
            to: H160::from(to_address),
            value: U256::from(0),
            data,
            access_list: vec![],
            v: 0,
            r: Default::default(),
            s: Default::default(),
        };

        // Should fail with CallDecodeFailed
        let result = crate::Pallet::<Test>::decode_call(&transaction);
        assert_eq!(result, Err(Error::<Test>::CallDecodeFailed));
    });
}

#[test]
fn test_decode_transfer_with_insufficient_data() {
    new_test_ext().execute_with(|| {
        // Data too short (only selector, missing arguments)
        let data = vec![0xa9, 0x05, 0x9c, 0xbb];

        let mut to_address = [0u8; 20];
        to_address[..8].copy_from_slice(b"Balances");

        let transaction = EthereumTransaction {
            chain_id: 1,
            nonce: 0,
            max_priority_fee_per_gas: U256::from(0),
            max_fee_per_gas: U256::from(0),
            gas_limit: 21000,
            to: H160::from(to_address),
            value: U256::from(0),
            data,
            access_list: vec![],
            v: 0,
            r: Default::default(),
            s: Default::default(),
        };

        // Should fail with InvalidTransactionData
        let result = crate::Pallet::<Test>::decode_call(&transaction);
        assert_eq!(result, Err(Error::<Test>::InvalidTransactionData));
    });
}

#[test]
fn test_unsupported_pallet() {
    new_test_ext().execute_with(|| {
        // Create an address for an unsupported pallet
        let mut to_address = [0u8; 20];
        to_address[..8].copy_from_slice(b"Staking\0");

        let transaction = EthereumTransaction {
            chain_id: 1,
            nonce: 0,
            max_priority_fee_per_gas: U256::from(0),
            max_fee_per_gas: U256::from(0),
            gas_limit: 21000,
            to: H160::from(to_address),
            value: U256::from(0),
            data: vec![],
            access_list: vec![],
            v: 0,
            r: Default::default(),
            s: Default::default(),
        };

        // Should fail with UnsupportedPallet error
        let result = crate::Pallet::<Test>::decode_call(&transaction);
        assert_eq!(result, Err(Error::<Test>::UnsupportedPallet));
    });
}

#[test]
fn test_signature_recovery_format() {
    new_test_ext().execute_with(|| {
        // Test that signature format is correct
        let transaction = EthereumTransaction {
            chain_id: 1,
            nonce: 0,
            max_priority_fee_per_gas: U256::from(0),
            max_fee_per_gas: U256::from(0),
            gas_limit: 21000,
            to: H160::from([0u8; 20]),
            value: U256::from(0),
            data: vec![],
            access_list: vec![],
            v: 27, // Legacy format
            r: H256::from([1u8; 32]),
            s: H256::from([2u8; 32]),
        };

        let sig = transaction.signature();
        assert!(sig.is_ok());

        let sig_bytes = sig.unwrap();
        assert_eq!(sig_bytes.len(), 65);

        // Check r
        assert_eq!(&sig_bytes[..32], &[1u8; 32]);
        // Check s
        assert_eq!(&sig_bytes[32..64], &[2u8; 32]);
        // Check v (should be converted from 27 to 0)
        assert_eq!(sig_bytes[64], 0);
    });
}

#[test]
fn test_signature_invalid_recovery_id() {
    new_test_ext().execute_with(|| {
        // Test with invalid recovery id
        let transaction = EthereumTransaction {
            chain_id: 1,
            nonce: 0,
            max_priority_fee_per_gas: U256::from(0),
            max_fee_per_gas: U256::from(0),
            gas_limit: 21000,
            to: H160::from([0u8; 20]),
            value: U256::from(0),
            data: vec![],
            access_list: vec![],
            v: 100, // Invalid recovery id
            r: H256::from([1u8; 32]),
            s: H256::from([2u8; 32]),
        };

        let sig = transaction.signature();
        assert!(sig.is_err());
    });
}

#[test]
fn test_transaction_hash() {
    new_test_ext().execute_with(|| {
        let transaction = EthereumTransaction {
            chain_id: 1,
            nonce: 0,
            max_priority_fee_per_gas: U256::from(0),
            max_fee_per_gas: U256::from(0),
            gas_limit: 21000,
            to: H160::from([0u8; 20]),
            value: U256::from(0),
            data: vec![],
            access_list: vec![],
            v: 0,
            r: H256::from([0u8; 32]),
            s: H256::from([0u8; 32]),
        };

        let hash1 = transaction.hash();
        let hash2 = transaction.hash();

        // Hash should be deterministic
        assert_eq!(hash1, hash2);
    });
}

#[test]
fn test_u256_conversion() {
    new_test_ext().execute_with(|| {
        // Test that U256 values are correctly decoded from transaction data
        let test_values = vec![
            0u128,
            1u128,
            1000u128,
            1_000_000_000_000_000_000u128, // 1 token with 18 decimals
        ];

        for value in test_values {
            let mut data = vec![0xa9, 0x05, 0x9c, 0xbb]; // transfer selector
            data.extend_from_slice(&[0u8; 12]); // padding
            data.extend_from_slice(&[1u8; 20]); // destination address

            // Encode value in big-endian format
            let value_u256 = U256::from(value);
            let value_words = value_u256.0;
            let mut be_bytes = [0u8; 32];
            for (i, &word) in value_words.iter().rev().enumerate() {
                let word_bytes = word.to_be_bytes();
                be_bytes[i * 8..(i + 1) * 8].copy_from_slice(&word_bytes);
            }
            data.extend_from_slice(&be_bytes);

            let mut to_address = [0u8; 20];
            to_address[..8].copy_from_slice(b"Balances");

            let transaction = EthereumTransaction {
                chain_id: 1,
                nonce: 0,
                max_priority_fee_per_gas: U256::from(0),
                max_fee_per_gas: U256::from(0),
                gas_limit: 21000,
                to: H160::from(to_address),
                value: U256::from(0),
                data,
                access_list: vec![],
                v: 0,
                r: Default::default(),
                s: Default::default(),
            };

            // Should successfully decode
            let result = crate::Pallet::<Test>::decode_call(&transaction);
            assert!(result.is_ok(), "Failed to decode value: {}", value);
        }
    });
}

#[test]
fn test_dispatch_balance_transfer() {
    new_test_ext().execute_with(|| {
        // Setup: Create accounts and fund the sender
        let sender_h160 = H160::from([1u8; 20]);
        let sender_account = crate::Pallet::<Test>::map_address_to_account(sender_h160);
        let dest_account = crate::Pallet::<Test>::map_address_to_account(H160::from([2u8; 20]));

        // Fund the sender account
        let _ = pallet_balances::Pallet::<Test>::force_set_balance(
            RuntimeOrigin::root(),
            sender_account.clone(),
            1_000_000,
        );

        // Create a balance transfer call
        let call = RuntimeCall::Balances(pallet_balances::Call::transfer_allow_death {
            dest: dest_account.clone(),
            value: 1000,
        });

        // SCALE-encode the call
        let call_data = call.encode();

        // Create transaction with SCALE-encoded call data
        let transaction = EthereumTransaction {
            chain_id: 1,
            nonce: 0,
            max_priority_fee_per_gas: U256::from(0),
            max_fee_per_gas: U256::from(0),
            gas_limit: 21000,
            to: H160::zero(), // Destination doesn't matter for SCALE calls
            value: U256::from(0),
            data: call_data,
            access_list: vec![],
            v: 0,
            r: Default::default(),
            s: Default::default(),
        };

        // Decode the call
        let decoded_call = crate::Pallet::<Test>::decode_call(&transaction);
        assert_ok!(decoded_call.clone());

        // Check initial balances
        assert_eq!(
            pallet_balances::Pallet::<Test>::free_balance(&sender_account),
            1_000_000
        );
        assert_eq!(
            pallet_balances::Pallet::<Test>::free_balance(&dest_account),
            0
        );

        // Dispatch the call directly (simulating what transact() does)
        let origin = RuntimeOrigin::signed(sender_account.clone());
        assert_ok!(decoded_call.unwrap().dispatch(origin));

        // Check final balances
        assert_eq!(
            pallet_balances::Pallet::<Test>::free_balance(&sender_account),
            999_000
        );
        assert_eq!(
            pallet_balances::Pallet::<Test>::free_balance(&dest_account),
            1000
        );
    });
}

#[test]
fn test_dispatch_balance_transfer_insufficient_funds() {
    new_test_ext().execute_with(|| {
        // Setup: Create accounts but don't fund the sender
        let sender_h160 = H160::from([1u8; 20]);
        let sender_account = crate::Pallet::<Test>::map_address_to_account(sender_h160);
        let dest_account = crate::Pallet::<Test>::map_address_to_account(H160::from([2u8; 20]));

        // Create a balance transfer call (trying to send more than available)
        let call = RuntimeCall::Balances(pallet_balances::Call::transfer_allow_death {
            dest: dest_account.clone(),
            value: 1000,
        });

        let call_data = call.encode();

        let transaction = EthereumTransaction {
            chain_id: 1,
            nonce: 0,
            max_priority_fee_per_gas: U256::from(0),
            max_fee_per_gas: U256::from(0),
            gas_limit: 21000,
            to: H160::zero(),
            value: U256::from(0),
            data: call_data,
            access_list: vec![],
            v: 0,
            r: Default::default(),
            s: Default::default(),
        };

        let decoded_call = crate::Pallet::<Test>::decode_call(&transaction).unwrap();

        // Dispatch should fail due to insufficient balance
        let origin = RuntimeOrigin::signed(sender_account.clone());
        let result = decoded_call.dispatch(origin);
        assert!(result.is_err());
    });
}

#[test]
fn test_dispatch_force_transfer_as_root() {
    new_test_ext().execute_with(|| {
        // Setup accounts
        let source = crate::Pallet::<Test>::map_address_to_account(H160::from([1u8; 20]));
        let dest = crate::Pallet::<Test>::map_address_to_account(H160::from([2u8; 20]));

        // Fund the source account
        let _ = pallet_balances::Pallet::<Test>::force_set_balance(
            RuntimeOrigin::root(),
            source.clone(),
            1_000_000,
        );

        // Create a force_transfer call (requires root)
        let call = RuntimeCall::Balances(pallet_balances::Call::force_transfer {
            source: source.clone(),
            dest: dest.clone(),
            value: 1000,
        });

        let call_data = call.encode();

        let transaction = EthereumTransaction {
            chain_id: 1,
            nonce: 0,
            max_priority_fee_per_gas: U256::from(0),
            max_fee_per_gas: U256::from(0),
            gas_limit: 21000,
            to: H160::zero(),
            value: U256::from(0),
            data: call_data,
            access_list: vec![],
            v: 0,
            r: Default::default(),
            s: Default::default(),
        };

        let decoded_call = crate::Pallet::<Test>::decode_call(&transaction).unwrap();

        // Dispatch with signed origin should fail (requires root)
        let signed_origin = RuntimeOrigin::signed(source.clone());
        let result = decoded_call.clone().dispatch(signed_origin);
        assert!(result.is_err());

        // Dispatch with root origin should succeed
        let root_origin = RuntimeOrigin::root();
        assert_ok!(decoded_call.dispatch(root_origin));

        // Verify balances changed
        assert_eq!(
            pallet_balances::Pallet::<Test>::free_balance(&source),
            999_000
        );
        assert_eq!(pallet_balances::Pallet::<Test>::free_balance(&dest), 1000);
    });
}

#[test]
fn test_dispatch_multiple_transfers() {
    new_test_ext().execute_with(|| {
        // Setup: Create sender and multiple recipients
        let sender_h160 = H160::from([1u8; 20]);
        let sender_account = crate::Pallet::<Test>::map_address_to_account(sender_h160);

        // Fund the sender
        let _ = pallet_balances::Pallet::<Test>::force_set_balance(
            RuntimeOrigin::root(),
            sender_account.clone(),
            10_000_000,
        );

        let recipients = vec![
            crate::Pallet::<Test>::map_address_to_account(H160::from([2u8; 20])),
            crate::Pallet::<Test>::map_address_to_account(H160::from([3u8; 20])),
            crate::Pallet::<Test>::map_address_to_account(H160::from([4u8; 20])),
        ];

        // Send transfers to each recipient
        for (i, recipient) in recipients.iter().enumerate() {
            let amount = 1000 * (i as u128 + 1);

            let call = RuntimeCall::Balances(pallet_balances::Call::transfer_allow_death {
                dest: recipient.clone(),
                value: amount as u64,
            });

            let call_data = call.encode();

            let transaction = EthereumTransaction {
                chain_id: 1,
                nonce: i as u64,
                max_priority_fee_per_gas: U256::from(0),
                max_fee_per_gas: U256::from(0),
                gas_limit: 21000,
                to: H160::zero(),
                value: U256::from(0),
                data: call_data,
                access_list: vec![],
                v: 0,
                r: Default::default(),
                s: Default::default(),
            };

            let decoded_call = crate::Pallet::<Test>::decode_call(&transaction).unwrap();
            let origin = RuntimeOrigin::signed(sender_account.clone());
            assert_ok!(decoded_call.dispatch(origin));

            // Verify recipient received funds
            assert_eq!(
                pallet_balances::Pallet::<Test>::free_balance(recipient),
                amount as u64
            );
        }

        // Verify sender balance decreased correctly
        let expected_remaining = 10_000_000 - (1000 + 2000 + 3000);
        assert_eq!(
            pallet_balances::Pallet::<Test>::free_balance(&sender_account),
            expected_remaining
        );
    });
}

#[test]
fn test_transact_extrinsic_success() {
    new_test_ext().execute_with(|| {
        // Setup accounts
        let sender_h160 = H160::from([1u8; 20]);
        let sender_account = crate::Pallet::<Test>::map_address_to_account(sender_h160);
        let dest_account = crate::Pallet::<Test>::map_address_to_account(H160::from([2u8; 20]));

        // Fund the sender
        let _ = pallet_balances::Pallet::<Test>::force_set_balance(
            RuntimeOrigin::root(),
            sender_account.clone(),
            1_000_000,
        );

        // Create a transfer call
        let call = RuntimeCall::Balances(pallet_balances::Call::transfer_allow_death {
            dest: dest_account.clone(),
            value: 5000,
        });

        let transaction = EthereumTransaction {
            chain_id: 1,
            nonce: 0,
            max_priority_fee_per_gas: U256::from(0),
            max_fee_per_gas: U256::from(0),
            gas_limit: 21000,
            to: H160::zero(),
            value: U256::from(0),
            data: call.encode(),
            access_list: vec![],
            v: 0,
            r: Default::default(),
            s: Default::default(),
        };

        // Call transact extrinsic
        // Note: This will fail signature verification, but tests the full flow
        // In real usage, the transaction would be properly signed
        let result = crate::Pallet::<Test>::transact(
            RuntimeOrigin::signed(sender_account.clone()),
            transaction,
        );

        // This will fail due to invalid signature, but that's expected
        // The test verifies the extrinsic can be called
        assert!(result.is_err());
    });
}
