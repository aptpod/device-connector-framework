//! Base elements for Device Connector.

use crate::ElementBank;
use serde_derive::Deserialize;

/// Configuration struct for elements that don't receive any configuration.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmptyElementConf {}

mod file;
mod null;
mod print_log;
mod process;
mod stat;
mod stdout;
mod text;

pub use file::*;
pub use null::*;
pub use print_log::*;
pub use process::*;
pub use stat::*;
pub use stdout::*;
pub use text::*;

pub(crate) fn append_to_bank(bank: &mut ElementBank) {
    bank.append_from_buildable::<FileSinkElement>().unwrap();
    bank.append_from_buildable::<FileSrcElement>().unwrap();
    bank.append_from_buildable::<PrintLogFilterElement>()
        .unwrap();
    bank.append_from_buildable::<StatFilterElement>().unwrap();
    bank.append_from_buildable::<StdoutSinkElement>().unwrap();
    bank.append_from_buildable::<NullSinkElement>().unwrap();
    bank.append_from_buildable::<ProcessSrcElement>().unwrap();
    bank.append_from_buildable::<TextSrcElement>().unwrap();
}
