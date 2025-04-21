use std::{ffi::CStr, str::FromStr};

use dc_common::msg_type::MsgType;
use libc::{c_char, c_void};
use semver::Version;

pub use crate::{msg_receiver::DcMsgReceiver, pipeline::DcPipeline};

extern "C" {
    pub fn dc_plugin_init(dc_plugin: *mut DcPlugin) -> bool;
}

/// Port number
pub type DcPort = u8;

/// Device connector plugin
#[repr(C)]
pub struct DcPlugin {
    _data: [u8; 0],
    _marker: core::marker::PhantomData<(*mut u8, core::marker::PhantomPinned)>,
}

pub struct Plugin {
    pub name: String,
    pub version: Version,
    pub elements: Vec<&'static Element>,
    pub authors: String,
}

impl Default for Plugin {
    fn default() -> Self {
        Self {
            name: "".into(),
            version: Version::new(3, 0, 0),
            elements: Vec::new(),
            authors: "".into(),
        }
    }
}

pub type DcFinalizerFunc = Option<unsafe extern "C-unwind" fn(*mut c_void) -> bool>;

/// Finalizer for element
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct DcFinalizer {
    pub f: DcFinalizerFunc,
    pub context: *mut c_void,
}

impl Default for DcFinalizer {
    fn default() -> Self {
        Self {
            context: std::ptr::null_mut(),
            f: None,
        }
    }
}

unsafe impl Send for DcFinalizer {}

/// Device connector element
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct DcElement {
    _data: [u8; 0],
    _marker: core::marker::PhantomData<(*mut u8, core::marker::PhantomPinned)>,
}

/// Element result
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DcElementResult {
    Err,
    Close,
    Msg,
    MsgBuf,
}

pub type DcElementNewFunc = unsafe extern "C-unwind" fn(config: *const c_char) -> *mut c_void;
pub type DcElementNextFunc = unsafe extern "C-unwind" fn(
    element: *mut c_void,
    *mut DcPipeline,
    *mut DcMsgReceiver,
) -> DcElementResult;
pub type DcElementFreeFunc = unsafe extern "C-unwind" fn(element: *mut c_void);
pub type DcElementFinalizerCreatorFunc =
    unsafe extern "C-unwind" fn(element: *mut c_void, finalizer: *mut DcFinalizer) -> bool;

#[derive(Debug)]
pub struct Element {
    pub name: String,
    pub description: String,
    pub config_doc: String,
    pub recv_ports: DcPort,
    pub send_ports: DcPort,
    pub recv_msg_types: Vec<Vec<MsgType>>,
    pub send_msg_types: Vec<Vec<MsgType>>,
    pub metadata_ids: Vec<String>,
    pub new: DcElementNewFunc,
    pub next: DcElementNextFunc,
    pub free: DcElementFreeFunc,
    pub finalizer_creator: Option<DcElementFinalizerCreatorFunc>,
}

/// Set name to this plugin.
#[no_mangle]
pub unsafe extern "C" fn dc_plugin_set_name(plugin: *mut DcPlugin, name: *const c_char) -> bool {
    let plugin: &mut Plugin = unsafe { &mut *(plugin as *mut Plugin) };
    let name = match unsafe { CStr::from_ptr(name) }.to_str() {
        Ok(name) => name,
        Err(e) => {
            core_log!(Error, "invalid name: {}", e);
            return false;
        }
    };
    plugin.name = name.to_owned();
    true
}

/// Set framework version to this plugin.
#[no_mangle]
pub unsafe extern "C" fn dc_plugin_set_version(
    plugin: *mut DcPlugin,
    version: *const c_char,
) -> bool {
    let plugin: &mut Plugin = unsafe { &mut *(plugin as *mut Plugin) };
    let version = match unsafe { CStr::from_ptr(version) }.to_str() {
        Ok(version) => version,
        Err(e) => {
            core_log!(Error, "invalid version: {}", e);
            return false;
        }
    };
    let version = match Version::from_str(version) {
        Ok(t) => t,
        Err(e) => {
            core_log!(Error, "invalid version: {}", e);
            return false;
        }
    };
    plugin.version = version;
    true
}

/// Register a element to this plugin.
#[no_mangle]
pub unsafe extern "C" fn dc_plugin_register_element(
    plugin: *mut DcPlugin,
    element: *const DcElement,
) {
    unsafe {
        let plugin: &mut Plugin = &mut *(plugin as *mut Plugin);
        plugin.elements.push(&*(element as *const Element));
    }
}

/// Set authors to this plugin.
#[no_mangle]
pub unsafe extern "C" fn dc_plugin_set_authors(
    plugin: *mut DcPlugin,
    authors: *const c_char,
) -> bool {
    let plugin: &mut Plugin = unsafe { &mut *(plugin as *mut Plugin) };
    let authors = match unsafe { CStr::from_ptr(authors) }.to_str() {
        Ok(authors) => authors,
        Err(e) => {
            core_log!(Error, "invalid authors: {}", e);
            return false;
        }
    };
    plugin.authors = authors.to_owned();
    true
}

/// Create an element.
#[no_mangle]
pub unsafe extern "C" fn dc_element_new(
    name: *const c_char,
    recv_ports: DcPort,
    send_ports: DcPort,
    new: DcElementNewFunc,
    next: DcElementNextFunc,
    free: DcElementFreeFunc,
) -> *mut DcElement {
    let name = match unsafe { CStr::from_ptr(name) }.to_str() {
        Ok(name) => name.to_owned(),
        Err(e) => {
            core_log!(Error, "invalid name for element: {}", e);
            return std::ptr::null_mut();
        }
    };
    let element = Box::new(Element {
        name,
        description: String::new(),
        config_doc: String::new(),
        recv_ports,
        send_ports,
        recv_msg_types: vec![Vec::new(); recv_ports as usize],
        send_msg_types: vec![Vec::new(); send_ports as usize],
        metadata_ids: Vec::new(),
        new,
        next,
        free,
        finalizer_creator: None,
    });
    Box::into_raw(element) as *mut _
}

/// Set a description to an element.
#[no_mangle]
pub unsafe extern "C" fn dc_element_set_description(element: *mut DcElement, desc: *const c_char) {
    unsafe {
        let element: &mut Element = &mut *(element as *mut Element);
        let desc = match CStr::from_ptr(desc).to_str() {
            Ok(desc) => desc.to_owned(),
            Err(e) => {
                core_log!(Error, "invalid description for element: {}", e);
                return;
            }
        };
        element.description = desc;
    }
}

/// Set a configration document to an element.
#[no_mangle]
pub unsafe extern "C" fn dc_element_set_config_doc(
    element: *mut DcElement,
    config_doc: *const c_char,
) {
    unsafe {
        let element: &mut Element = &mut *(element as *mut Element);
        let config_doc = match CStr::from_ptr(config_doc).to_str() {
            Ok(config_doc) => config_doc.to_owned(),
            Err(e) => {
                core_log!(Error, "invalid config document for element: {}", e);
                return;
            }
        };
        element.config_doc = config_doc;
    }
}

/// Set a message type for receiving to an element.
#[no_mangle]
pub unsafe extern "C" fn dc_element_append_recv_msg_type(
    element: *mut DcElement,
    port: DcPort,
    msg_type: *const c_char,
) -> bool {
    let element: &mut Element = unsafe { &mut *(element as *mut Element) };
    let t = match unsafe { CStr::from_ptr(msg_type) }.to_str() {
        Ok(t) => t,
        Err(e) => {
            core_log!(Error, "invalid msg_type: {}", e);
            return false;
        }
    };
    let t = match MsgType::from_str(t) {
        Ok(t) => t,
        Err(e) => {
            core_log!(Error, "invalid msg_type: {}", e);
            return false;
        }
    };
    if let Some(msg_types) = element.recv_msg_types.get_mut(port as usize) {
        msg_types.push(t);
        true
    } else {
        core_log!(
            Error,
            "tried append recv message type to invalid port: {}",
            port
        );
        false
    }
}

/// Set a message type for sending to an element.
#[no_mangle]
pub unsafe extern "C" fn dc_element_append_send_msg_type(
    element: *mut DcElement,
    port: DcPort,
    msg_type: *const c_char,
) -> bool {
    let element: &mut Element = unsafe { &mut *(element as *mut Element) };
    let t = match unsafe { CStr::from_ptr(msg_type) }.to_str() {
        Ok(t) => t,
        Err(e) => {
            core_log!(Error, "invalid msg_type: {}", e);
            return false;
        }
    };
    let t = match MsgType::from_str(t) {
        Ok(t) => t,
        Err(e) => {
            core_log!(Error, "invalid msg_type: {}", e);
            return false;
        }
    };
    if let Some(msg_types) = element.send_msg_types.get_mut(port as usize) {
        msg_types.push(t);
        true
    } else {
        core_log!(
            Error,
            "tried append send message type to invalid port: {}",
            port
        );
        false
    }
}

/// Set a metadata id to an element.
#[no_mangle]
pub unsafe extern "C" fn dc_element_append_metadata_id(
    element: *mut DcElement,
    metadata_id: *const c_char,
) -> bool {
    let element: &mut Element = unsafe { &mut *(element as *mut Element) };
    let metadata_id = match unsafe { CStr::from_ptr(metadata_id) }.to_str() {
        Ok(metadata_id) => metadata_id,
        Err(e) => {
            core_log!(Error, "invalid metadata_id: {}", e);
            return false;
        }
    };
    element.metadata_ids.push(metadata_id.into());
    true
}

/// Set finalizer creator to an element.
#[no_mangle]
pub unsafe extern "C-unwind" fn dc_element_set_finalizer_creator(
    element: *mut DcElement,
    f: DcElementFinalizerCreatorFunc,
) {
    let element: &mut Element = unsafe { &mut *(element as *mut Element) };
    element.finalizer_creator = Some(f);
}
