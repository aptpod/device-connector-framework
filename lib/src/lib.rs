#![allow(clippy::missing_safety_doc)]
#![warn(unsafe_op_in_unsafe_fn)]

#[macro_use]
mod utils;

pub use dc_common::conf;

pub mod channel;
pub mod context;
pub mod element;
pub mod loader;
pub mod log;
pub mod metadata;
pub mod msg;
pub mod msg_buf;
pub mod msg_receiver;
pub mod pipeline;
pub mod plugin;
pub mod process;
pub mod runner;
pub mod task;

#[cfg(test)]
mod tests;
