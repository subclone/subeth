//! Tests for the EVM adapter pallet

use crate::{mock::*, types::*, Error, Event};
use frame::testing_prelude::*;
use polkadot_sdk::{polkadot_sdk_frame as frame, sp_core::{H160, U256}};

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
		let mut value_bytes = [0u8; 32];
		value.to_big_endian(&mut value_bytes);
		data.extend_from_slice(&value_bytes);

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
