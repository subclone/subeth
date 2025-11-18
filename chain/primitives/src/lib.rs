//! # Subeth Primitives
//!
//! Shared types and utilities used by both the EVM adapter pallet and the Subeth RPC adapter.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::vec::Vec;
use alloy_primitives::{Address, B256, U256 as AlloyU256};
use parity_scale_codec::{Decode, DecodeWithMemTracking, Encode};
use scale_info::TypeInfo;
use sp_core::{hashing::keccak_256, H160, H256, U256};

/// Helper function to convert U256 to little-endian bytes
pub fn u256_to_le_bytes(value: &U256) -> [u8; 32] {
    let mut bytes = [0u8; 32];
    for (i, &word) in value.0.iter().enumerate() {
        let word_bytes = word.to_le_bytes();
        bytes[i * 8..(i + 1) * 8].copy_from_slice(&word_bytes);
    }
    bytes
}

/// Represents an Ethereum transaction for the pallet
///
/// This is a simplified version supporting EIP-1559 transactions
#[derive(Clone, Eq, PartialEq, Encode, Decode, DecodeWithMemTracking, Debug, TypeInfo)]
pub struct EthereumTransaction {
    /// Chain ID
    pub chain_id: u64,
    /// Nonce
    pub nonce: u64,
    /// Max priority fee per gas
    pub max_priority_fee_per_gas: U256,
    /// Max fee per gas
    pub max_fee_per_gas: U256,
    /// Gas limit
    pub gas_limit: u64,
    /// Destination address (pallet identifier)
    pub to: H160,
    /// Value to transfer
    pub value: U256,
    /// Call data (function selector + encoded arguments)
    pub data: Vec<u8>,
    /// Access list (not used in this MVP)
    pub access_list: Vec<(H160, Vec<H256>)>,
    /// Signature V
    pub v: u64,
    /// Signature R
    pub r: H256,
    /// Signature S
    pub s: H256,
}

impl EthereumTransaction {
    /// Calculate the transaction hash
    pub fn hash(&self) -> H256 {
        let encoded = self.encode();
        H256::from(keccak_256(&encoded))
    }

    /// Get the message hash that was signed
    pub fn message_hash(&self) -> [u8; 32] {
        let mut message = Vec::new();

        // EIP-1559 transaction type
        message.push(0x02);

        // Simplified message construction (in production, use proper RLP encoding)
        message.extend_from_slice(&self.chain_id.to_le_bytes());
        message.extend_from_slice(&self.nonce.to_le_bytes());

        // Convert U256 to bytes
        message.extend_from_slice(&u256_to_le_bytes(&self.max_priority_fee_per_gas));
        message.extend_from_slice(&u256_to_le_bytes(&self.max_fee_per_gas));
        message.extend_from_slice(&self.gas_limit.to_le_bytes());
        message.extend_from_slice(self.to.as_bytes());
        message.extend_from_slice(&u256_to_le_bytes(&self.value));

        message.extend_from_slice(&self.data);

        keccak_256(&message)
    }

    /// Get the signature in the format expected by secp256k1_ecdsa_recover
    ///
    /// Returns a 65-byte signature: [r(32) || s(32) || v(1)]
    pub fn signature(&self) -> Result<[u8; 65], ()> {
        let mut signature = [0u8; 65];

        // Copy r (32 bytes)
        signature[..32].copy_from_slice(self.r.as_bytes());

        // Copy s (32 bytes)
        signature[32..64].copy_from_slice(self.s.as_bytes());

        // Copy v (1 byte)
        // For EIP-1559, v is either 0 or 1 (recovery id)
        // If v is 27 or 28 (legacy format), convert to 0 or 1
        let recovery_id = if self.v >= 27 {
            (self.v - 27) as u8
        } else {
            self.v as u8
        };

        if recovery_id > 1 {
            return Err(());
        }

        signature[64] = recovery_id;

        Ok(signature)
    }
}

// Conversion utilities for adapter
pub mod conversions {
    use super::*;

    /// Convert alloy Address to sp_core H160
    pub fn alloy_address_to_h160(address: Address) -> H160 {
        H160::from_slice(address.as_slice())
    }

    /// Convert alloy U256 to sp_core U256
    pub fn alloy_u256_to_sp_u256(value: AlloyU256) -> U256 {
        // Get bytes in little-endian format
        let bytes = value.to_le_bytes::<32>();

        // Convert bytes to u64 words in little-endian
        let mut words = [0u64; 4];
        for i in 0..4 {
            words[i] = u64::from_le_bytes([
                bytes[i * 8],
                bytes[i * 8 + 1],
                bytes[i * 8 + 2],
                bytes[i * 8 + 3],
                bytes[i * 8 + 4],
                bytes[i * 8 + 5],
                bytes[i * 8 + 6],
                bytes[i * 8 + 7],
            ]);
        }

        U256(words)
    }

    /// Convert alloy B256 to sp_core H256
    pub fn alloy_b256_to_h256(value: B256) -> H256 {
        H256::from_slice(value.as_slice())
    }
}
