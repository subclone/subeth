//! Primitive types used in the library.

use serde::{Deserialize, Serialize};
use smoldot_light::HandleRpcError;

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

impl From<jsonrpsee::types::ErrorObject> for SubEthError {
    fn from(e: jsonrpsee::types::ErrorObject) -> Self {
        log::error!("Error: {:?}", e);
        SubEthError::ResponseFailed
    }
}
