use std::{ffi::CString, fmt::Display};

use crate::log::*;

macro_rules! core_log {
    ($level:ident, $($arg:tt)+) => {
        crate::utils::core_log(crate::log::DcLogLevel::$level, module_path!(), format_args!($($arg)+))
    };
}

pub fn core_log<T: Display>(level: DcLogLevel, module: &str, msg: T) {
    let module = CString::new(module).unwrap();
    let msg = CString::new(msg.to_string()).unwrap();

    unsafe {
        dc_log(
            level,
            module.as_ptr(),
            b"core\0".as_ptr() as *const _,
            msg.as_ptr(),
        );
    }
}

pub fn debug_without_newline<T: std::fmt::Debug>(a: T) -> String {
    let msg = format!("{:?}", a);
    msg.replace("\n", " ")
}
