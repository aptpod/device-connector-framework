pub use anyhow::Error;

use thiserror::Error;

use crate::task::TaskPort;
use common::MsgType;

pub use common::ReceiveError;

/// Error during build elements
#[derive(Debug, Error)]
pub enum ElementBuildError {
    #[error("{0}")]
    ConfDeserialize(serde_yaml::Error),
    #[error("{0}")]
    UnknownElement(#[from] UnknownElementError),
    #[error("element construct failed\n{0}")]
    New(crate::error::Error),
    #[error("{0}")]
    Other(String),
}

/// Unknown element error
#[derive(Debug, Error)]
#[error("unknown element \"{0}\"")]
pub struct UnknownElementError(pub String);

/// Unknown element error
#[derive(Debug, Error)]
#[error("{0}")]
pub struct ElementAppendError(pub String);

/// Port parse error
#[derive(Debug, Error)]
#[error("{0}")]
pub struct TaskPortParseError(pub String);

/// Type check error
#[derive(Debug, Error)]
#[error("message type \"{0}\" is not acceptable by task {1}")]
pub struct TypeCheckError(pub(crate) MsgType, pub(crate) TaskPort);

/// Plugin element execution error
#[derive(Debug, Error)]
#[error("plugin element execution error")]
pub struct PluginElementExecutionError;
