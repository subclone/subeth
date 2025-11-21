//! # EVM Adapter Pallet
//!
//! This pallet provides EVM compatibility for Substrate chains by:
//! - Accepting Ethereum-style transactions (EIP-1559)
//! - Verifying ECDSA signatures
//! - Mapping EVM addresses (H160/AccountId20) to Substrate accounts (AccountId32)
//! - Decoding SCALE-encoded RuntimeCall from transaction data
//! - Dispatching calls with the recovered account as origin
//!
//! ## Overview
//!
//! The pallet acts as a bridge between Ethereum-style transactions and Substrate FRAME calls.
//!
//! **Transaction Structure:**
//! - `to`: Can be any address (currently not validated, reserved for future use)
//! - `data`: SCALE-encoded RuntimeCall (pallet_index + call_index + params)
//! - Signature fields (`v`, `r`, `s`): ECDSA signature over the transaction
//!
//! **Flow:**
//! 1. Verify ECDSA signature and recover signer (H160 address)
//! 2. Map H160 → AccountId32 using Blake2-256 hash
//! 3. Decode `data` field as SCALE-encoded RuntimeCall
//! 4. Dispatch call with mapped account as signed origin
//!
//! This works with **any** runtime call - Balances, Staking, Governance, Democracy, Utility, etc.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

use alloc::vec::Vec;
use codec::Decode;
use polkadot_sdk::sp_io::crypto::secp256k1_ecdsa_recover;
use polkadot_sdk::{
    polkadot_sdk_frame as frame,
    sp_core::{H160, H256},
};
use subeth_primitives::EthereumTransaction;

pub use pallet::*;

#[frame::pallet]
pub mod pallet {
    use super::*;
    use frame::prelude::*;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: polkadot_sdk::frame_system::Config {
        #[allow(deprecated)]
        type RuntimeEvent: From<Event<Self>>
            + IsType<<Self as polkadot_sdk::frame_system::Config>::RuntimeEvent>;
        /// The overarching call type that can be dispatched.
        /// Must be SCALE-decodable.
        type RuntimeCall: Dispatchable<RuntimeOrigin = Self::RuntimeOrigin> + Decode;
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// An EVM transaction was successfully executed
        TransactionExecuted {
            from: H160,
            to: H160,
            transaction_hash: H256,
        },
        /// An EVM transaction failed
        TransactionFailed {
            from: H160,
            to: H160,
            error: Vec<u8>,
        },
    }

    #[pallet::error]
    #[derive(PartialEq)]
    pub enum Error<T> {
        /// Failed to recover signer from signature
        SignerRecoveryFailed,
        /// Failed to decode SCALE-encoded RuntimeCall from transaction data
        CallDecodeFailed,
        /// Call dispatch failed
        DispatchFailed,
        /// Invalid ECDSA recovery id (must be 0 or 1)
        InvalidRecoveryId,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Submit an Ethereum transaction to be executed on the Substrate chain.
        ///
        /// This function:
        /// 1. Verifies the transaction signature
        /// 2. Maps the EVM address to a Substrate account
        /// 3. Decodes the transaction data into a FRAME call
        /// 4. Dispatches the call
        ///
        /// # Parameters
        /// - `origin`: Should be signed (for MVP)
        /// - `transaction`: The Ethereum transaction to execute
        #[pallet::call_index(0)]
        #[pallet::weight(Weight::from_parts(10_000, 0) + T::DbWeight::get().writes(1))]
        pub fn transact(origin: OriginFor<T>, transaction: EthereumTransaction) -> DispatchResult {
            // For MVP, we accept transactions from any signed origin
            // In production, this should be an unsigned transaction with proper validation
            let _ = ensure_signed(origin)?;

            // Verify signature and recover signer
            let from = Self::verify_and_recover_signer(&transaction)?;

            // Decode the call from transaction data
            let call = Self::decode_call(&transaction)?;

            // Map EVM address to Substrate account
            let substrate_account = Self::map_address_to_account(from);

            // Dispatch the call with the mapped account as origin
            let origin = frame_system::RawOrigin::Signed(substrate_account).into();
            let result = call.dispatch(origin);

            match result {
                Ok(_) => {
                    Self::deposit_event(Event::TransactionExecuted {
                        from,
                        to: transaction.to,
                        transaction_hash: transaction.hash(),
                    });
                    Ok(())
                }
                Err(e) => {
                    Self::deposit_event(Event::TransactionFailed {
                        from,
                        to: transaction.to,
                        error: alloc::format!("{:?}", e.error).into_bytes(),
                    });
                    Err(Error::<T>::DispatchFailed.into())
                }
            }
        }
    }

    impl<T: Config> Pallet<T> {
        /// Verify ECDSA signature and recover the signer address
        pub fn verify_and_recover_signer(
            transaction: &EthereumTransaction,
        ) -> Result<H160, Error<T>> {
            let message_hash = transaction.message_hash();
            let signature = transaction
                .signature()
                .map_err(|_| Error::<T>::InvalidRecoveryId)?;

            // Recover the public key from the signature
            let pubkey = secp256k1_ecdsa_recover(&signature, &message_hash)
                .map_err(|_| Error::<T>::SignerRecoveryFailed)?;

            // Get the Ethereum address from the public key
            // Address is the last 20 bytes of the keccak256 hash of the public key
            let address_hash = polkadot_sdk::sp_io::hashing::keccak_256(&pubkey);
            let mut address = [0u8; 20];
            address.copy_from_slice(&address_hash[12..]);

            Ok(H160::from(address))
        }

        /// Map an EVM address (H160/AccountId20) to a Substrate account (AccountId32)
        ///
        /// This uses the same logic as in the adapter: hash the address to get AccountId32
        pub fn map_address_to_account(address: H160) -> T::AccountId {
            let mut input = [0u8; 32];
            input[..20].copy_from_slice(address.as_bytes());
            let hash = polkadot_sdk::sp_io::hashing::blake2_256(&input);
            T::AccountId::decode(&mut &hash[..]).expect("32 bytes can always decode to AccountId")
        }

        /// Decode the transaction data into a runtime call
        ///
        /// The transaction's `data` field contains a SCALE-encoded RuntimeCall:
        /// - First byte: pallet index
        /// - Second byte: call index
        /// - Remaining bytes: SCALE-encoded call parameters
        ///
        /// This works with any runtime call that can be SCALE-decoded.
        pub fn decode_call(
            transaction: &EthereumTransaction,
        ) -> Result<<T as Config>::RuntimeCall, Error<T>> {
            // Decode the data field as a SCALE-encoded RuntimeCall
            <T as Config>::RuntimeCall::decode(&mut &transaction.data[..])
                .map_err(|_| Error::<T>::CallDecodeFailed)
        }
    }
}
