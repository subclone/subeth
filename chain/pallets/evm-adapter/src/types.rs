//! Types for EVM adapter pallet

use alloc::vec::Vec;
use codec::{Decode, DecodeWithMemTracking, Encode};
use polkadot_sdk::{
    sp_core::{H160, H256, U256},
    sp_io::hashing::keccak_256,
};
use scale_info::TypeInfo;

/// Helper function to convert U256 to little-endian bytes
fn u256_to_le_bytes(value: &U256) -> [u8; 32] {
    let mut bytes = [0u8; 32];
    for (i, &word) in value.0.iter().enumerate() {
        let word_bytes = word.to_le_bytes();
        bytes[i * 8..(i + 1) * 8].copy_from_slice(&word_bytes);
    }
    bytes
}

/// Represents an Ethereum transaction
///
/// This is a simplified version supporting EIP-1559 transactions
#[derive(Clone, Eq, PartialEq, Encode, Decode, Debug, TypeInfo, DecodeWithMemTracking)]
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
    ///
    /// This is the keccak256 hash of the RLP-encoded transaction
    pub fn hash(&self) -> H256 {
        // For simplicity, we use scale encoding instead of RLP
        // In production, this should use RLP encoding
        let encoded = self.encode();
        H256::from(keccak_256(&encoded))
    }

    /// Get the message hash that was signed
    ///
    /// For EIP-1559 transactions: keccak256(0x02 || rlp([chain_id, nonce, ...]))
    /// For simplicity in this MVP, we use a simplified version
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

/// Simple transaction request (for easier construction in tests)
#[derive(Clone, Eq, PartialEq, Encode, Decode, Debug, TypeInfo)]
pub struct TransactionRequest {
    /// From address
    pub from: H160,
    /// To address
    pub to: H160,
    /// Value
    pub value: U256,
    /// Data
    pub data: Vec<u8>,
    /// Gas limit
    pub gas_limit: Option<u64>,
    /// Nonce
    pub nonce: Option<u64>,
}
