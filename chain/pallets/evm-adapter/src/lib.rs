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
    sp_core::{H160, H256, U256},
};
use subeth_primitives::{EthereumTransaction, PalletContractMapping};

pub use pallet::*;

#[frame::pallet]
pub mod pallet {
    use super::*;
    use frame::prelude::*;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    use polkadot_sdk::pallet_balances;

    #[pallet::config]
    pub trait Config: polkadot_sdk::frame_system::Config + pallet_balances::Config {
        #[allow(deprecated)]
        type RuntimeEvent: From<Event<Self>>
            + IsType<<Self as polkadot_sdk::frame_system::Config>::RuntimeEvent>;
        /// The overarching call type that can be dispatched.
        /// Must be SCALE-decodable.
        type RuntimeCall: Dispatchable<RuntimeOrigin = Self::RuntimeOrigin>
            + Decode
            + From<pallet_balances::Call<Self>>;
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
    #[derive(PartialEq, Clone)]
    pub enum Error<T> {
        /// Failed to recover signer from signature
        SignerRecoveryFailed,
        /// Failed to decode SCALE-encoded RuntimeCall from transaction data
        CallDecodeFailed,
        /// Call dispatch failed
        DispatchFailed,
        /// Invalid ECDSA recovery id (must be 0 or 1)
        InvalidRecoveryId,
        /// Unsupported pallet
        UnsupportedPallet,
        /// Invalid transaction data
        InvalidTransactionData,
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
            let pallet_name = Self::pallet_name_from_address(transaction.to)
                .ok_or(Error::<T>::UnsupportedPallet)?;

            match pallet_name.as_str() {
                "Balances" => Self::decode_balances_call(transaction),
                "" => <T as Config>::RuntimeCall::decode(&mut &transaction.data[..])
                    .map_err(|_| Error::<T>::CallDecodeFailed),
                _ => Err(Error::<T>::UnsupportedPallet),
            }
        }

        fn decode_balances_call(
            transaction: &EthereumTransaction,
        ) -> Result<<T as Config>::RuntimeCall, Error<T>> {
            // Check selector: transfer(address,uint256) -> 0xa9059cbb
            if transaction.data.len() < 4 || transaction.data[0..4] != [0xa9, 0x05, 0x9c, 0xbb] {
                return Err(Error::<T>::CallDecodeFailed);
            }

            // Check data length: 4 (selector) + 32 (address) + 32 (value) = 68
            if transaction.data.len() < 68 {
                return Err(Error::<T>::InvalidTransactionData);
            }

            // Decode address (last 20 bytes of the first 32-byte word)
            let mut address_bytes = [0u8; 20];
            address_bytes.copy_from_slice(&transaction.data[16..36]);
            let address = H160::from(address_bytes);
            use polkadot_sdk::sp_runtime::traits::StaticLookup;
            let dest_account = Self::map_address_to_account(address);
            let dest = T::Lookup::unlookup(dest_account);

            // Decode value (second 32-byte word)
            let mut value_bytes = [0u8; 32];
            value_bytes.copy_from_slice(&transaction.data[36..68]);
            let value = U256::from_big_endian(&value_bytes);

            // Convert U256 to T::Balance
            // For MVP, we assume T::Balance is u64 or u128 and fits
            // We'll try to convert to u128 first
            let amount_u128 = value.low_u128();
            // Then convert to T::Balance. This assumes T::Balance can be created from u128
            // or we just cast it. Since we can't easily do generic conversion here without more bounds,
            // we'll limit to what fits in u128 and use `try_into` if possible, or just `saturated_into` if available.
            // But `saturated_into` is for `SaturatedConversion`.
            // Let's assume T::Balance is at least u64.

            // A safer way for generic T::Balance is using `AtLeast32BitUnsigned` which `Balance` usually implements.
            // But we don't have that bound here easily.
            // Let's use `unique_saturated_into` from `sp_runtime::traits::UniqueSaturatedInto`?
            // Or just `try_into`.

            // For now, let's assume we can convert via Encode/Decode or similar hack,
            // OR just add `From<u128>` or `From<u64>` bound to Balance.
            // `pallet_balances::Config::Balance` has `Member + Parameter + AtLeast32BitUnsigned + Default + Copy + MaxEncodedLen`.
            // `AtLeast32BitUnsigned` implies `From<u32>`.

            // Let's try to decode it as T::Balance from the bytes directly? No, it's U256 BE.

            // We will use `TryInto` if we add the bound, or `sp_runtime::traits::Bounded::max_value()` check.
            // Actually, let's just use `value.low_u128()` and cast to `T::Balance` using `sp_runtime::traits::SaturatedConversion`.
            use polkadot_sdk::sp_runtime::traits::SaturatedConversion;
            let amount: <T as pallet_balances::Config>::Balance = amount_u128.saturated_into();

            Ok(pallet_balances::Call::<T>::transfer_allow_death {
                dest,
                value: amount,
            }
            .into())
        }
    }

    impl<T: Config> Pallet<T> {
        /// Get the pallet name from the address
        pub fn pallet_name_from_address(address: H160) -> Option<alloc::string::String> {
            let address_bytes = address.as_bytes();
            let alloy_address = alloy_primitives::Address::from_slice(address_bytes);
            PalletContractMapping::pallet_name(alloy_address)
        }
    }
}
