// Import c types for macros
pub mod ctypes {
    pub use libc::{c_char, c_void};
}

/// Implement dc_load by types that implement ElementBuildable
#[macro_export]
macro_rules! define_dc_load {
    ($($t:path,)*) => {
        #[no_mangle]
        unsafe extern "C" fn dc_load(plugin: *mut $crate::common::DcPlugin) -> bool {
            use $crate::macros::ctypes::*;
            use $crate::common::{
                DcElement, DcElementResult, DcFinalizer, DcMsgReceiver, DcPipeline, dc_init
            };
            use $crate::{
                ElementBuildable, ElementConf, ElementExecutable, ElementFinalizer, ElementValue,
                Pipeline, MsgReceiver,
            };
            use std::ffi::{CStr, CString};

            let plugin = if let Some(plugin) = plugin.as_mut() {
                plugin
            } else {
                std::process::abort();
            };

            let crate_name_c_str = CString::new(env!("CARGO_CRATE_NAME")).unwrap().into_raw();
            dc_init(crate_name_c_str);

            plugin.version = CString::new("0.1.0").unwrap().into_raw();

            let mut elements: Vec<DcElement> = Vec::new();

            $({
                use $t as TargetElement;

                unsafe extern "C" fn new(config: *const c_char) -> *mut c_void {
                    let config_str = CStr::from_ptr(config).to_string_lossy();
                    let conf: <TargetElement as ElementBuildable>::Config
                        = match $crate::common::deserialize_default_format(&config_str) {
                        Ok(conf) => conf,
                        Err(e) => {
                            log::error!("{:?}", e);
                            return std::ptr::null_mut();
                        }
                    };

                    let element = match TargetElement::new(conf) {
                        Ok(element) => element,
                        Err(e) => {
                            log::error!("{:?}", e);
                            return std::ptr::null_mut();
                        }
                    };

                    let element = Box::new(element);
                    let element_ptr = Box::into_raw(element) as *mut _;
                    element_ptr
                }

                unsafe extern "C" fn next(
                    element: *mut c_void,
                    pipeline: *mut DcPipeline,
                    msg_receiver: *mut DcMsgReceiver,
                ) -> DcElementResult {
                    let element_ptr = element as *mut TargetElement;
                    let element: &mut TargetElement = &mut *element_ptr;

                    let mut pipeline = Pipeline::new(pipeline);
                    let mut msg_receiver = MsgReceiver::new(msg_receiver);
                    let result = element.next(&mut pipeline, &mut msg_receiver);

                    match result {
                        Ok(ElementValue::Close) => DcElementResult::Close,
                        Ok(ElementValue::MsgBuf) => DcElementResult::MsgBuf,
                        Err(e) => {
                            log::error!("{:?}", e);
                            DcElementResult::Err
                        }
                    }
                }

                unsafe extern "C" fn finalizer(
                    element: *mut c_void,
                    finalizer: *mut DcFinalizer,
                ) -> bool {
                    let element_ptr = element as *mut TargetElement;
                    let element: &mut TargetElement = &mut *element_ptr;

                    match element.finalizer() {
                        Ok(Some(f)) => {
                            unsafe extern "C" fn finalizer_caller(context: *mut c_void) -> bool {
                                let context: *mut ElementFinalizer = context as _;
                                let f: Box<ElementFinalizer> = Box::from_raw(context);
                                match f() {
                                    Ok(_) => true,
                                    Err(e) => {
                                        log::error!("{:?}", e);
                                        false
                                    }
                                }
                            }

                            let context = Box::into_raw(Box::new(f));
                            let finalizer: &mut DcFinalizer = &mut *finalizer;

                            *finalizer = DcFinalizer {
                                f: Some(finalizer_caller),
                                context: context as *mut _,
                            };
                            true
                        },
                        Ok(None) => {
                            true
                        },
                        Err(e) => {
                            log::error!("{:?}", e);
                            false
                        }
                    }
                }

                unsafe extern "C" fn free(element: *mut c_void) {
                    let element_ptr = element as *mut TargetElement;
                    let _element: Box<TargetElement> = Box::from_raw(element_ptr);
                }

                let acceptable_msg_types = if TargetElement::RECV_PORTS == 0 {
                    std::ptr::null()
                } else if let Some(acceptable_msg_types)
                    = $crate::macros::acceptable_msg_types_to_cstr_list(
                        TargetElement::acceptable_msg_types(),
                        TargetElement::RECV_PORTS as usize,
                    ) {
                    acceptable_msg_types
                } else {
                    log::error!("invalid acceptable_msg_types");
                    std::ptr::null()
                };

                let element = DcElement {
                    name: CString::new(TargetElement::NAME).unwrap().into_raw(),
                    recv_ports: TargetElement::RECV_PORTS,
                    send_ports: TargetElement::SEND_PORTS,
                    acceptable_msg_types,
                    config_format: CString::new("json").unwrap().into_raw(),
                    new,
                    next,
                    finalizer,
                    free,
                };

                elements.push(element);
            })*

            let elements = std::mem::ManuallyDrop::new(elements);
            plugin.n_element = elements.len();
            plugin.elements = elements.as_ptr();

            true
        }
    };

    ($($t:path),*) => {
        $crate::define_dc_load!($($t,)*);
    }
}

use crate::MsgType;
use std::{ffi::CString, fmt::Write};

pub fn acceptable_msg_types_to_cstr_list(
    msg_types: Vec<Vec<MsgType>>,
    recv_port: usize,
) -> Option<*const *const ctypes::c_char> {
    let mut list: Vec<*const ctypes::c_char> = Vec::new();
    for port in 0..recv_port {
        let mut s = String::new();
        let msg_type_list_for_port = msg_types.get(port)?;
        for (i, msg_type) in msg_type_list_for_port.iter().enumerate() {
            if i != 0 {
                write!(&mut s, ",").unwrap();
            }
            write!(&mut s, "{}", msg_type).unwrap();
        }
        let p = CString::new(s).unwrap().into_raw();
        list.push(p);
    }

    let list = std::mem::ManuallyDrop::new(list);

    Some(list.as_ptr())
}
