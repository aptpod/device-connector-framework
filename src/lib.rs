pub extern crate device_connector_common as common;

/// Basic elements for Device Connector.
pub mod base;
mod channel;
mod close;
/// Configuration types.
pub mod conf;
pub mod element;
/// Error types.
pub mod error;
mod finalizer;
mod loaded_plugin;
mod msg_buf;
mod pipeline;
mod plugin;
pub mod process;
mod runner;
mod task;
mod type_check;

#[doc(hidden)]
pub mod macros;

pub use element::*;
pub use error::Error;
pub use loaded_plugin::*;
pub use runner::*;
pub use task::task_closing;

pub use common::{
    ElementConf, ElementResult, ElementValue, Msg, MsgReceiver, MsgReceiverBuf, MsgReceiverRead,
    Pipeline, Port,
};

pub use base::EmptyElementConf;
