use serde::de::DeserializeOwned;
use serde_derive::{Deserialize, Serialize};
use thiserror::Error;

/// Port number of elements.
pub type Port = u8;

/// Element result value for C
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DcElementResult {
    Err,
    Close,
    MsgBuf,
}

/// General element configuration type
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ElementConf(serde_json::Value);

/// Deserialization error from ElementConf
#[derive(Debug, Error)]
#[error("{0}")]
pub struct ElementConfDeserializeError(#[from] serde_json::Error);

impl Default for ElementConf {
    fn default() -> Self {
        ElementConf(serde_json::Value::Object(serde_json::map::Map::new()))
    }
}

impl ElementConf {
    /// Deserialize to specific configuration type
    pub fn to_conf<T: DeserializeOwned>(&self) -> Result<T, ElementConfDeserializeError> {
        Ok(serde_json::from_value(self.0.clone())?)
    }

    /// Deserialize `s` as the default format.
    pub fn from_default_format(s: &str) -> Result<Self, ElementConfDeserializeError> {
        Ok(serde_json::from_str(s)?)
    }
}

/// Deserialize `s` as the default format.
pub fn deserialize_default_format<T: DeserializeOwned>(
    s: &str,
) -> Result<T, ElementConfDeserializeError> {
    Ok(serde_json::from_str(s)?)
}

/// Result value of elements
pub type ElementResult = Result<ElementValue, anyhow::Error>;

impl From<ElementValue> for DcElementResult {
    fn from(value: ElementValue) -> Self {
        match value {
            ElementValue::Close => DcElementResult::Close,
            ElementValue::MsgBuf => DcElementResult::MsgBuf,
        }
    }
}

/// Return value from element
#[derive(Debug)]
pub enum ElementValue {
    Close,
    MsgBuf,
}

/// Receive error
#[derive(Debug, Error)]
#[error("channel receive failed. sender task will be closed.")]
pub struct ReceiveError;

/// Type check error
#[derive(Debug, Error)]
#[error("sending type check failed.")]
pub struct TypeCheckError;
