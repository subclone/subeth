//! Types for converting ETH-like types to Substrate types and vice versa

use alloy_primitives::Address;
use sp_core::{blake2_128, blake2_256, twox_128, twox_256, twox_64};
use subxt::{metadata::types::StorageHasher, utils::AccountId32};

/// Pallet to contract address mapping
pub(crate) struct PalletContractMapping;

impl PalletContractMapping {
    /// Get the contract address for a given pallet
    pub fn contract_address(pallet: &str) -> Address {
        // take only 8 chars from the pallet name
        let prefix = pallet.chars().take(8).collect::<String>();
        Self::pad_to_eth_address(&prefix)
    }

    /// Get the pallet name for a given contract address
    pub fn pallet_name(address: Address) -> Option<String> {
        // try to convert the address to a string
        let prefix = String::from_utf8(address.to_vec()).ok()?;
        Some(prefix.trim_end_matches('\0').to_string())
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
pub(crate) struct AddressMapping;

impl AddressMapping {
    /// Hash `AccountId20` to get `AccountId32`
    pub fn to_ss58(address: Address) -> AccountId32 {
        let mut input = [0u8; 32];
        input[..20].copy_from_slice(&address.to_vec());
        let hash = blake2_256(&input);
        AccountId32::from(hash)
    }

    /// Truncate `AccountId32` to get `AccountId20`
    pub fn to_address(account_id: AccountId32) -> Address {
        let inner: &[u8; 32] = account_id.as_ref();
        Address::from_slice(&inner[..20])
    }
}

/// Pallet storage read structure
#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
pub struct StorageKey {
    /// Storage item name
    pub name: String,
    /// The rest of the keys, could be multiple if n-map storage
    pub keys: Vec<Vec<u8>>,
}

/// Hash the key to get the storage key
pub fn hash_key(key: &[u8], hasher: &StorageHasher) -> Vec<u8> {
    match hasher {
        StorageHasher::Blake2_128 => blake2_128(key).to_vec(),
        StorageHasher::Blake2_256 => blake2_256(key).to_vec(),
        StorageHasher::Blake2_128Concat => {
            let hash = blake2_128(key);
            let mut result = Vec::with_capacity(32 + key.len());
            result.extend_from_slice(&hash);
            result.extend_from_slice(key);
            result
        }
        StorageHasher::Twox128 => twox_128(key).to_vec(),
        StorageHasher::Twox256 => twox_256(key).to_vec(),
        StorageHasher::Twox64Concat => {
            let hash = twox_64(key);
            let mut result = Vec::with_capacity(8 + key.len());
            result.extend_from_slice(&hash);
            result.extend_from_slice(key);
            result
        }
        StorageHasher::Identity => key.to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::{address, hex};

    #[test]
    fn test_hash_and_truncate() {
        let address = Address::from([1u8; 20]);
        let expected_account_id =
            hex!("0x8b304616ddedac8267d0381d53301825902eb056a70fc56b90e84efa492a015b");
        let account_id = AddressMapping::to_ss58(address);
        let account_id_raw: &[u8] = account_id.as_ref();
        assert_eq!(account_id_raw, expected_account_id);

        let account_id = AccountId32::from([1u8; 32]);
        let new_address = AddressMapping::to_address(account_id);

        assert_eq!(address, new_address);
    }

    #[test]
    fn test_pallet_mapping_works() {
        let balances = PalletContractMapping::contract_address("Balances");
        let staking = PalletContractMapping::contract_address("Staking");
        let democracy = PalletContractMapping::contract_address("democrac");
        let treasury = PalletContractMapping::contract_address("treasury");

        assert_eq!(
            balances,
            address!("0x42616c616e636573000000000000000000000000")
        );
        assert_eq!(
            staking,
            address!("0x5374616b696e6700000000000000000000000000")
        );
        assert_eq!(
            democracy,
            address!("0x64656d6f63726163000000000000000000000000")
        );
        assert_eq!(
            treasury,
            address!("0x7472656173757279000000000000000000000000")
        );

        assert_eq!(
            PalletContractMapping::pallet_name(balances).unwrap(),
            "Balances"
        );
        assert_eq!(
            PalletContractMapping::pallet_name(staking).unwrap(),
            "Staking"
        );
        assert_eq!(
            PalletContractMapping::pallet_name(democracy).unwrap(),
            "democrac"
        );
        assert_eq!(
            PalletContractMapping::pallet_name(treasury).unwrap(),
            "treasury"
        );
    }
}
