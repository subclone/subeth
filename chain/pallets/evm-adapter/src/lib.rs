//! # EVM Adapter Pallet
//!
//! This pallet provides EVM compatibility for Substrate chains by:
//! - Decoding Ethereum transactions into Substrate extrinsics
//! - Verifying ECDSA signatures
//! - Mapping EVM addresses (AccountId20) to Substrate accounts (AccountId32)
//! - Dispatching FRAME calls based on transaction data
//!
//! ## Overview
//!
//! The pallet acts as a bridge between Ethereum-style transactions and Substrate FRAME calls.
//! It interprets the `to` address as a pallet identifier and the `data` field as the call data.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub mod types;

use alloc::vec::Vec;
use codec::Decode;
use polkadot_sdk::polkadot_sdk_frame as frame;
use polkadot_sdk::sp_io::crypto::secp256k1_ecdsa_recover;
use types::EthereumTransaction;

pub use pallet::*;

#[frame::pallet]
pub mod pallet {
    use super::*;
    use frame::prelude::*;
    use polkadot_sdk::pallet_balances;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: polkadot_sdk::frame_system::Config + pallet_balances::Config {
        #[allow(deprecated)]
        type RuntimeEvent: From<Event<Self>>
            + IsType<<Self as polkadot_sdk::frame_system::Config>::RuntimeEvent>;
        /// The overarching call type.
        type RuntimeCall: Dispatchable<RuntimeOrigin = Self::RuntimeOrigin>
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
    pub enum Error<T> {
        /// Invalid signature
        InvalidSignature,
        /// Failed to recover signer
        SignerRecoveryFailed,
        /// Invalid transaction data
        InvalidTransactionData,
        /// Unsupported pallet
        UnsupportedPallet,
        /// Failed to decode call
        CallDecodeFailed,
        /// Failed to dispatch call
        DispatchFailed,
        /// Invalid recovery id
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
        fn verify_and_recover_signer(transaction: &EthereumTransaction) -> Result<H160, Error<T>> {
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
        fn map_address_to_account(address: H160) -> T::AccountId {
            let mut input = [0u8; 32];
            input[..20].copy_from_slice(address.as_bytes());
            let hash = polkadot_sdk::sp_io::hashing::blake2_256(&input);
            T::AccountId::decode(&mut &hash[..]).expect("32 bytes can always decode to AccountId")
        }

        /// Map a pallet name to an EVM address (reverse of what adapter does)
        fn pallet_name_from_address(address: H160) -> Option<Vec<u8>> {
            // Try to convert the address to a string (pallet name)
            let bytes = address.as_bytes();
            let name_bytes: Vec<u8> = bytes.iter().copied().take_while(|&b| b != 0).collect();

            if name_bytes.is_empty() {
                None
            } else {
                Some(name_bytes)
            }
        }

        /// Decode the transaction into a runtime call
        ///
        /// The transaction's `to` field contains the pallet name (first 8 chars as bytes)
        /// The transaction's `data` field contains the encoded call
        fn decode_call(
            transaction: &EthereumTransaction,
        ) -> Result<<T as Config>::RuntimeCall, Error<T>> {
            // Get pallet name from `to` address
            let pallet_name = Self::pallet_name_from_address(transaction.to)
                .ok_or(Error::<T>::UnsupportedPallet)?;

            // For now, we only support the Balances pallet
            if pallet_name == b"Balances" {
                Self::decode_balances_call(transaction)
            } else {
                Err(Error::<T>::UnsupportedPallet)
            }
        }

        /// Decode a Balances pallet call from transaction data
        ///
        /// Expected data format:
        /// - First 4 bytes: function selector (keccak256 of function signature)
        /// - Remaining bytes: ABI-encoded arguments
        ///
        /// For simplicity in this MVP, we support:
        /// - transfer(address,uint256) -> maps to transfer_allow_death
        fn decode_balances_call(
            transaction: &EthereumTransaction,
        ) -> Result<<T as Config>::RuntimeCall, Error<T>> {
            let data = &transaction.data;

            // We need at least 4 bytes for the function selector
            if data.len() < 4 {
                return Err(Error::<T>::InvalidTransactionData);
            }

            // Extract function selector (first 4 bytes)
            let selector = &data[..4];

            // Function selector for "transfer(address,uint256)"
            // keccak256("transfer(address,uint256)") = 0xa9059cbb...
            const TRANSFER_SELECTOR: [u8; 4] = [0xa9, 0x05, 0x9c, 0xbb];

            if selector == TRANSFER_SELECTOR {
                // Decode ABI-encoded arguments
                // Arguments are: address (32 bytes, but only last 20 bytes used) + uint256 (32 bytes)
                if data.len() < 68 {
                    // 4 (selector) + 32 (address) + 32 (value)
                    return Err(Error::<T>::InvalidTransactionData);
                }

                // Extract destination address (bytes 16-36, as first 12 bytes of the 32-byte slot are padding)
                let mut dest_address = [0u8; 20];
                dest_address.copy_from_slice(&data[16..36]);
                let dest_h160 = H160::from(dest_address);
                let dest_account = Self::map_address_to_account(dest_h160);

                // Extract value (bytes 36-68)
                let value_bytes = &data[36..68];
                let value_u256 = U256::from_big_endian(value_bytes);

                // Convert U256 to the balance type
                // For this MVP, we assume balance fits in u128
                let value: u128 = value_u256
                    .try_into()
                    .map_err(|_| Error::<T>::InvalidTransactionData)?;

                // Create the balances transfer call
                use frame::traits::StaticLookup;
                let call = pallet_balances::Call::<T>::transfer_allow_death {
                    dest: <T::Lookup as StaticLookup>::unlookup(dest_account),
                    value: value.saturated_into(),
                };

                Ok(call.into())
            } else {
                Err(Error::<T>::CallDecodeFailed)
            }
        }
    }
}
