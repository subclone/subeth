//! Mock runtime for testing

use crate as pallet_evm_adapter;
use frame::testing_prelude::*;
use polkadot_sdk::{polkadot_sdk_frame as frame, *};

type Block = frame_system::mocking::MockBlock<Test>;

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
	pub enum Test
	{
		System: frame_system,
		Balances: pallet_balances,
		EvmAdapter: pallet_evm_adapter,
	}
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
	type Block = Block;
	type AccountData = pallet_balances::AccountData<<Test as pallet_balances::Config>::Balance>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
	type AccountStore = System;
}

impl pallet_evm_adapter::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
}

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

	pallet_balances::GenesisConfig::<Test> {
		balances: vec![
			// Fund some test accounts
			([1u8; 32].into(), 1000000),
			([2u8; 32].into(), 1000000),
		],
	}
	.assimilate_storage(&mut t)
	.unwrap();

	t.into()
}
