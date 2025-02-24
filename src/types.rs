//! Primitive types used in the library.

use jsonrpsee::types::ErrorObject;
use serde::{Deserialize, Serialize};
use subxt::{
    config::substrate::{BlakeTwo256, SubstrateHeader},
    Config, PolkadotConfig,
};

/// Subxt mappings
#[subxt::subxt(runtime_metadata_path = "./artifacts/polkadot_metadata_full.scale")]

mod src_chain {}
pub use src_chain::*;

/// Configuration of the chain
pub enum ChainConfig {}
impl Config for ChainConfig {
    type Hash = subxt::utils::H256;
    type AccountId = <PolkadotConfig as Config>::AccountId;
    type Address = <PolkadotConfig as Config>::Address;
    type Signature = <PolkadotConfig as Config>::Signature;
    type Hasher = BlakeTwo256;
    type Header = SubstrateHeader<u32, BlakeTwo256>;
    type AssetId = <PolkadotConfig as Config>::AssetId;
    type ExtrinsicParams = subxt::config::signed_extensions::AnyOf<
        Self,
        (
            subxt::config::signed_extensions::CheckSpecVersion,
            subxt::config::signed_extensions::CheckTxVersion,
            subxt::config::signed_extensions::CheckNonce,
            subxt::config::signed_extensions::CheckGenesis<Self>,
            subxt::config::signed_extensions::CheckMortality<Self>,
            subxt::config::signed_extensions::ChargeAssetTxPayment<Self>,
            subxt::config::signed_extensions::ChargeTransactionPayment,
            subxt::config::signed_extensions::CheckMetadataHash,
        ),
    >;
}

/// General error type for the Subeth library.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SubEthError {
    /// Request failed
    RequestFailed(&'static str),
    /// Response failed
    ResponseFailed,
    /// Serde related error
    SerdeError(&'static str),
    /// Conversion error
    ConversionError,
}

impl From<&'static str> for SubEthError {
    fn from(e: &'static str) -> Self {
        log::error!("Error: {:?}", e);
        SubEthError::RequestFailed(e)
    }
}

impl From<serde::de::value::Error> for SubEthError {
    fn from(e: serde::de::value::Error) -> Self {
        log::error!("Error: {:?}", e);
        SubEthError::SerdeError("Deserialization error")
    }
}

impl From<serde_json::Error> for SubEthError {
    fn from(e: serde_json::Error) -> Self {
        log::error!("Error: {:?}", e);
        SubEthError::SerdeError("JSON deserialization error")
    }
}

impl From<jsonrpsee::types::ErrorObjectOwned> for SubEthError {
    fn from(e: jsonrpsee::types::ErrorObjectOwned) -> Self {
        log::error!("Error: {:?}", e);
        SubEthError::ResponseFailed
    }
}

impl From<subxt::Error> for SubEthError {
    fn from(e: subxt::Error) -> Self {
        log::error!("Error: {:?}", e);
        SubEthError::RequestFailed("Subxt error")
    }
}

impl From<SubEthError> for ErrorObject<'_> {
    fn from(error: SubEthError) -> Self {
        match error {
            SubEthError::RequestFailed(msg) => ErrorObject::owned(500, msg, None::<()>),
            SubEthError::ResponseFailed => ErrorObject::owned(500, "Response failed", None::<()>),
            SubEthError::SerdeError(msg) => ErrorObject::owned(500, msg, None::<()>),
            SubEthError::ConversionError => ErrorObject::owned(500, "Conversion error", None::<()>),
        }
    }
}

impl From<()> for SubEthError {
    fn from(_: ()) -> Self {
        SubEthError::ConversionError
    }
}
