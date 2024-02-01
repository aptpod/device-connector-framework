//! Base elements for Device Connector.

use crate::ElementBank;
use serde::Deserialize;

/// Configuration struct for elements that don't receive any configuration.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmptyElementConf {}

mod file;
mod fixed_size;
mod null;
mod print_log;
mod process;
mod stat;
mod stdout;
mod tcp;
mod tee;
mod text;
mod udp;

pub use file::*;
pub use fixed_size::*;
pub use null::*;
pub use print_log::*;
pub use process::*;
pub use stat::*;
pub use stdout::*;
pub use tcp::*;
pub use tee::*;
pub use text::*;
pub use udp::*;

pub(crate) fn append_to_bank(bank: &mut ElementBank) {
    bank.append_from_buildable::<FileSinkElement>().unwrap();
    bank.append_from_buildable::<FileSrcElement>().unwrap();
    bank.append_from_buildable::<SplitByFixedSizeFilterElement>()
        .unwrap();
    bank.append_from_buildable::<PrintLogFilterElement>()
        .unwrap();
    bank.append_from_buildable::<StatFilterElement>().unwrap();
    bank.append_from_buildable::<StdoutSinkElement>().unwrap();
    bank.append_from_buildable::<NullSinkElement>().unwrap();
    bank.append_from_buildable::<ProcessSrcElement>().unwrap();
    bank.append_from_buildable::<RepeatProcessSrcElement>()
        .unwrap();
    bank.append_from_buildable::<TcpSrcElement>().unwrap();
    bank.append_from_buildable::<TcpSinkElement>().unwrap();
    bank.append_from_buildable::<TeeSrcElement>().unwrap();
    bank.append_from_buildable::<TeeFilterElement>().unwrap();
    bank.append_from_buildable::<TextSrcElement>().unwrap();
    bank.append_from_buildable::<SplitByDelimiterFilterElement>()
        .unwrap();
    bank.append_from_buildable::<JsonSplitFilterElement>()
        .unwrap();
    bank.append_from_buildable::<UdpSrcElement>().unwrap();
    bank.append_from_buildable::<UdpSinkElement>().unwrap();
}
