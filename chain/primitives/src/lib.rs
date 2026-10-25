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
        // Match on the full u64 so out-of-range values can't wrap into 0/1 when cast
        let recovery_id = match self.v {
            0 | 1 => self.v as u8,
            27 | 28 => (self.v - 27) as u8,
            _ => return Err(()),
        };

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

use alloc::string::String;
use sp_core::blake2_256;

/// Pallet to contract address mapping
pub struct PalletContractMapping;

impl PalletContractMapping {
    /// Get the contract address for a given pallet
    pub fn contract_address(pallet: &str) -> Address {
        // take only 8 chars from the pallet name
        let prefix = pallet.chars().take(8).collect::<String>();
        Self::pad_to_eth_address(&prefix)
    }

    /// Get the pallet name for a given contract address
    ///
    /// The address must be an ASCII identifier padded with trailing zero bytes, as produced by
    /// [`Self::contract_address`]. The zero address yields an empty name.
    pub fn pallet_name(address: Address) -> Option<String> {
        let bytes = address.as_slice();
        let len = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
        let (name, padding) = bytes.split_at(len);

        if padding.iter().any(|&b| b != 0)
            || !name.iter().all(|&b| b.is_ascii_alphanumeric() || b == b'_')
        {
            return None;
        }

        Some(name.iter().map(|&b| b as char).collect())
    }

    /// Pad the given string to a valid Ethereum address (20 bytes)
    fn pad_to_eth_address(prefix: &str) -> Address {
        let mut address = [0u8; 20];
        let prefix_bytes = prefix.as_bytes();
        let len = prefix_bytes.len().min(20);
        address[..len].copy_from_slice(&prefix_bytes[..len]);
        Address::from(address)
    }
}

/// Address mapping logic
pub struct AddressMapping;

impl AddressMapping {
    /// Hash `Address` (H160) to get `AccountId32` (as [u8; 32])
    pub fn to_ss58(address: Address) -> [u8; 32] {
        let mut input = [0u8; 32];
        input[..20].copy_from_slice(&address.to_vec());
        blake2_256(&input)
    }

    /// Truncate `AccountId32` (as [u8; 32]) to get `Address` (H160)
    pub fn to_address(account_id: &[u8; 32]) -> Address {
        Address::from_slice(&account_id[..20])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tx_with_v(v: u64) -> EthereumTransaction {
        EthereumTransaction {
            chain_id: 42,
            nonce: 0,
            max_priority_fee_per_gas: U256::zero(),
            max_fee_per_gas: U256::zero(),
            gas_limit: 21_000,
            to: H160::zero(),
            value: U256::zero(),
            data: Vec::new(),
            access_list: Vec::new(),
            v,
            r: H256::repeat_byte(1),
            s: H256::repeat_byte(2),
        }
    }

    #[test]
    fn pallet_name_requires_zero_padded_identifier() {
        let balances = PalletContractMapping::contract_address("Balances");
        assert_eq!(
            PalletContractMapping::pallet_name(balances).as_deref(),
            Some("Balances")
        );
        // The zero address is the generic-call address and maps to an empty name
        assert_eq!(
            PalletContractMapping::pallet_name(Address::ZERO).as_deref(),
            Some("")
        );

        let mut bytes = [0u8; 20];
        // Non-zero bytes after the padding started: not a pallet address
        bytes[..3].copy_from_slice(b"Bal");
        bytes[10] = b'x';
        assert_eq!(
            PalletContractMapping::pallet_name(Address::from(bytes)),
            None
        );
        // Printable-but-not-identifier bytes (an ordinary EOA can easily look like this)
        let eoa = Address::from([
            0x41, 0x20, 0x7f, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b,
            0x0c, 0x0d, 0x0e, 0x0f, 0x10, 0x11,
        ]);
        assert_eq!(PalletContractMapping::pallet_name(eoa), None);
    }

    #[test]
    fn signature_accepts_only_valid_recovery_ids() {
        assert_eq!(tx_with_v(0).signature().unwrap()[64], 0);
        assert_eq!(tx_with_v(1).signature().unwrap()[64], 1);
        assert_eq!(tx_with_v(27).signature().unwrap()[64], 0);
        assert_eq!(tx_with_v(28).signature().unwrap()[64], 1);

        // 283 - 27 = 256 would wrap to 0 if truncated to u8
        for v in [2, 26, 29, 283, 284, u64::MAX] {
            assert!(
                tx_with_v(v).signature().is_err(),
                "v = {v} must be rejected"
            );
        }
    }
}
