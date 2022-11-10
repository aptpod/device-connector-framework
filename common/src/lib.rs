mod defines;
mod msg_buf;
mod msg_receiver;
mod msg_type;
mod pipeline;

pub use defines::*;
pub use msg_buf::*;
pub use msg_receiver::*;
pub use msg_type::*;
pub use pipeline::*;

use libc::{c_char, c_void, size_t};

/// Device connector plugin
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct DcPlugin {
    pub version: *const c_char,
    pub n_element: size_t,
    pub elements: *const DcElement,
}

unsafe impl Send for DcPlugin {}

/// Finalizer for element
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct DcFinalizer {
    pub f: Option<unsafe extern "C" fn(*mut c_void) -> bool>,
    pub context: *mut c_void,
}

unsafe impl Send for DcFinalizer {}

/// Device connector element
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct DcElement {
    /// Element Name. Must have static lifetime.
    pub name: *const c_char,
    /// The number of receiver ports.
    pub recv_ports: Port,
    /// The number of sender ports.
    pub send_ports: Port,
    /// Acceptable MsgType.
    pub acceptable_msg_types: *const *const c_char,
    /// Config text format passed to new(). Must have static lifetime.
    pub config_format: *const c_char,
    /// Create new element.
    pub new: unsafe extern "C" fn(config: *const c_char) -> *mut c_void,
    /// Execute element and returns next value.
    pub next: unsafe extern "C" fn(
        element: *mut c_void,
        *mut DcPipeline,
        *mut DcMsgReceiver,
    ) -> DcElementResult,
    /// Returns element finalizer.
    pub finalizer: unsafe extern "C" fn(element: *mut c_void, finalizer: *mut DcFinalizer) -> bool,
    /// Free used element.
    pub free: unsafe extern "C" fn(element: *mut c_void),
}

/// Initialize plugin. Must be called at first in dc_load().
/// # Safety
/// `plugin_name` must points valid null-​terminated string.
#[no_mangle]
pub unsafe extern "C" fn dc_init(plugin_name: *const c_char) {
    let plugin_name = std::ffi::CStr::from_ptr(plugin_name).to_string_lossy();
    let suffix = format!(" ({})\n", plugin_name);
    let suffix = Box::leak(suffix.into_boxed_str());

    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .format_suffix(suffix)
        .init();
    log::info!("dc_init()");
}
