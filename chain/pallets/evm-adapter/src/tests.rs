//! Tests for the EVM adapter pallet

use crate::{mock::*, Error};
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
		assert_eq!(pallet_name, Some(b"Balances".to_vec()));
	});
}

#[test]
fn test_pallet_address_encoding() {
	new_test_ext().execute_with(|| {
		// Test that pallet names are correctly encoded as addresses
		let test_cases = vec![
			"Balances",
			"System\0\0",
		];

		for pallet_name in test_cases {
			let mut address_bytes = [0u8; 20];
			address_bytes[..pallet_name.len()].copy_from_slice(pallet_name.as_bytes());
			let address = H160::from(address_bytes);

			// Verify we can decode it back
			let decoded = crate::Pallet::<Test>::pallet_name_from_address(address);
			assert!(decoded.is_some());

			// The decoded name should match (without null bytes)
			let decoded_bytes = decoded.unwrap();
			let decoded_str = String::from_utf8_lossy(&decoded_bytes);
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
