//! Primitive types used in the library.

use jsonrpsee::types::ErrorObject;
use serde::{Deserialize, Serialize};
use smoldot_light::HandleRpcError;

/// Pallet to contract address mapping
pub(crate) struct PalletContractMapping;

impl PalletContractMapping {
    /// Get the contract address for a given pallet
    pub fn get_contract_address(pallet: &str) -> Option<String> {
        match pallet {
            "Balances" => Some("0x1".repeat(20)),
            "Contracts" => Some("0x2".repeat(20)),
            "Sudo" => Some("0x3".repeat(20)),
            "System" => Some("0x4".repeat(20)),
            _ => None,
        }
    }
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
}

impl From<HandleRpcError> for SubEthError {
    fn from(e: HandleRpcError) -> Self {
        match e {
            HandleRpcError::TooManyPendingRequests { json_rpc_request } => {
                log::error!("Too many pending requests: {:?}", json_rpc_request);
                SubEthError::RequestFailed("Too many pending requests")
            }
        }
    }
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

// impl From<jsonrpsee::types::ErrorObject> for SubEthError {
//     fn from(e: jsonrpsee::types::ErrorObject) -> Self {
//         log::error!("Error: {:?}", e);
//         SubEthError::ResponseFailed
//     }
// }

impl From<SubEthError> for ErrorObject<'_> {
    fn from(error: SubEthError) -> Self {
        match error {
            SubEthError::RequestFailed(msg) => ErrorObject::owned(500, msg, None::<()>),
            SubEthError::ResponseFailed => ErrorObject::owned(500, "Response failed", None::<()>),
            SubEthError::SerdeError(msg) => ErrorObject::owned(500, msg, None::<()>),
        }
    }
}
