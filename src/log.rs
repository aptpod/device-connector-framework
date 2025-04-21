#![cfg_attr(not(feature = "log-0_4"), allow(unused))]

use std::ffi::{c_char, CString};

use log::{Level, LevelFilter, Metadata, Record};
use sys::DcLogLevel;

/// Log level.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

struct DcLogger {
    level: Level,
    plugin: *const c_char,
}

unsafe impl Send for DcLogger {}
unsafe impl Sync for DcLogger {}

/// Initialize the logger.
pub fn init_log(plugin_name: &str, level: Option<LogLevel>) {
    if let Some(level) = level {
        let level = match level {
            LogLevel::Error => sys::DcLogLevel_Error,
            LogLevel::Warn => sys::DcLogLevel_Warn,
            LogLevel::Info => sys::DcLogLevel_Info,
            LogLevel::Debug => sys::DcLogLevel_Debug,
            LogLevel::Trace => sys::DcLogLevel_Trace,
        };
        unsafe { sys::dc_log_init(level) };
    }

    init_log_plugin_name(plugin_name);
}

fn level_to_dc_level(level: Level) -> DcLogLevel {
    match level {
        Level::Error => sys::DcLogLevel_Error,
        Level::Warn => sys::DcLogLevel_Warn,
        Level::Info => sys::DcLogLevel_Info,
        Level::Debug => sys::DcLogLevel_Debug,
        Level::Trace => sys::DcLogLevel_Trace,
    }
}

impl log::Log for DcLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let level = level_to_dc_level(record.level());
        let module = record.module_path().unwrap_or_default();
        let module = CString::new(module).unwrap();
        let msg = format!("{}\0", record.args());
        let msg = CString::from_vec_with_nul(msg.into_bytes()).unwrap();

        unsafe { sys::dc_log(level, module.as_ptr(), self.plugin, msg.as_ptr()) };
    }

    fn flush(&self) {}
}

#[cfg(feature = "log-0_4")]
fn init_log_plugin_name(plugin_name: &str) {
    let level = unsafe { sys::dc_log_get_level() };
    let (level, level_filter) = match level {
        sys::DcLogLevel_Trace => (Level::Trace, LevelFilter::Trace),
        sys::DcLogLevel_Debug => (Level::Debug, LevelFilter::Debug),
        sys::DcLogLevel_Info => (Level::Info, LevelFilter::Info),
        sys::DcLogLevel_Warn => (Level::Warn, LevelFilter::Warn),
        sys::DcLogLevel_Error => (Level::Error, LevelFilter::Error),
        _ => {
            panic!("dc_log_get_level() returned invalid value");
        }
    };
    let plugin = CString::new(plugin_name).unwrap().into_raw();
    let logger = DcLogger { level, plugin };
    if log::set_boxed_logger(Box::new(logger)).is_err() {
        log::warn!(
            "multiple log init in {}. Maybe the same plugin is loaded.",
            plugin_name,
        );
        return;
    }
    log::set_max_level(level_filter);
}

#[cfg(not(feature = "log-0_4"))]
fn init_log_plugin_name(_plugin_name: &str) {}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn log() {
        init_log("logtest", Some(LogLevel::Trace));

        log::trace!("Test log (TRACE)");
        log::debug!("Test log (DEBUG)");
        log::info!("Test log (INFO)");
        log::warn!("Test log (WARN)");
        log::error!("Test log (ERROR)");
    }
}
