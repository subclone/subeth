//! Primitive types used in the library.

use alloy_rpc_types_eth::Transaction;
use jsonrpsee::types::ErrorObject;
use serde::{Deserialize, Serialize};
use subxt::blocks::Block;
use subxt::OnlineClient;
/// Chain configuration - uses PolkadotConfig for Polkadot, SubstrateConfig for local chain
#[cfg(feature = "polkadot")]
pub type ChainConfig = subxt::PolkadotConfig;

#[cfg(not(feature = "polkadot"))]
pub type ChainConfig = subxt::SubstrateConfig;

pub type SubstrateBlock = Block<ChainConfig, OnlineClient<ChainConfig>>;
pub type EthTransaction = Transaction;

/// Subxt mappings - uses Polkadot metadata when `polkadot` feature is enabled,
/// otherwise uses local chain metadata for development/testing with local node.
#[cfg(feature = "polkadot")]
#[subxt::subxt(runtime_metadata_path = "../artifacts/polkadot_metadata_full.scale")]
mod src_chain {}

#[cfg(not(feature = "polkadot"))]
#[subxt::subxt(runtime_metadata_path = "../artifacts/local_metadata.scale")]
mod src_chain {}

pub use src_chain::*;

/// General error type for the Subeth library.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SubEthError {
    /// Request failed
    RequestFailed(&'static str),
    /// Response failed
    ResponseFailed,
    /// Serde related error
    SerdeError(&'static str),
    /// Errors related to adapter
    AdapterError { message: String },
    /// Not supported yet
    Unsupported,
}

impl From<&'static str> for SubEthError {
    fn from(e: &'static str) -> Self {
        log::error!("Generic error: {:?}", e);
        SubEthError::RequestFailed(e)
    }
}

impl From<serde::de::value::Error> for SubEthError {
    fn from(e: serde::de::value::Error) -> Self {
        log::error!("Serde error: {:?}", e);
        SubEthError::SerdeError("Deserialization error")
    }
}

impl From<serde_json::Error> for SubEthError {
    fn from(e: serde_json::Error) -> Self {
        log::error!("Serde json error: {:?}", e);
        SubEthError::SerdeError("JSON deserialization error")
    }
}

impl From<jsonrpsee::types::ErrorObjectOwned> for SubEthError {
    fn from(e: jsonrpsee::types::ErrorObjectOwned) -> Self {
        log::error!("jsonrpsee error: {:?}", e);
        SubEthError::ResponseFailed
    }
}

impl From<subxt::Error> for SubEthError {
    fn from(e: subxt::Error) -> Self {
        log::error!("subxt error: {:?}", e);
        SubEthError::AdapterError {
            message: format!("Subxt err: {:?}", e),
        }
    }
}

/// JSON-RPC internal error (EIP-1474)
pub const INTERNAL_ERROR_CODE: i32 = -32603;
/// JSON-RPC "method not supported" error (EIP-1474)
pub const METHOD_NOT_SUPPORTED_CODE: i32 = -32004;

impl From<SubEthError> for ErrorObject<'_> {
    fn from(error: SubEthError) -> Self {
        match error {
            SubEthError::RequestFailed(msg) => {
                ErrorObject::owned(INTERNAL_ERROR_CODE, msg, None::<()>)
            }
            SubEthError::ResponseFailed => {
                ErrorObject::owned(INTERNAL_ERROR_CODE, "Response failed", None::<()>)
            }
            SubEthError::SerdeError(msg) => {
                ErrorObject::owned(INTERNAL_ERROR_CODE, msg, None::<()>)
            }
            SubEthError::AdapterError { message } => {
                ErrorObject::owned(INTERNAL_ERROR_CODE, "Adapter error", Some(message))
            }
            SubEthError::Unsupported => ErrorObject::owned(
                METHOD_NOT_SUPPORTED_CODE,
                "Method not supported",
                None::<()>,
            ),
        }
    }
}

impl From<()> for SubEthError {
    fn from(_: ()) -> Self {
        SubEthError::AdapterError {
            message: "generic adapter error".to_string(),
        }
    }
}
