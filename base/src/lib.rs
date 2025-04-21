pub mod file;
pub mod fixed_size;
pub mod null;
pub mod print_log;
pub mod process;
pub mod stat;
pub mod stdio;
pub mod tcp;
pub mod tee;
pub mod text;
pub mod udp;

dc_core::define_plugin!(
    "base";
    file::FileSrcElement,
    file::FileSinkElement,
    fixed_size::SplitByFixedSizeFilterElement,
    print_log::PrintLogFilterElement,
    process::ProcessSrcElement,
    process::RepeatProcessSrcElement,
    stat::StatFilterElement,
    stdio::StdoutSinkElement,
    tcp::TcpSrcElement,
    tcp::TcpSinkElement,
    tee::TeeFilterElement,
    tee::TeeSrcElement,
    text::JsonSplitFilterElement,
    text::SplitByDelimiterFilterElement,
    text::TextSrcElement,
    null::NullSinkElement,
    udp::UdpSrcElement,
    udp::UdpSinkElement,
);
