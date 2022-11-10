use crate::element::*;
use crate::error::Error;
use crate::ElementConf;
use anyhow::bail;
use common::{DcElement, DcElementResult, DcMsgReceiver, DcPipeline, ElementResult};
use device_connector_common::DcFinalizer;
use libc::c_void;
use std::{
    ffi::{CStr, CString},
    str::FromStr,
};

impl ElementBank {
    pub(crate) fn append_plugin(&mut self, element: DcElement) -> Result<(), Error> {
        let name = unsafe { CStr::from_ptr(element.name) }.to_str()?;

        let builder = ElementBuilder::Plugin(element);

        let acceptable_msg_types_list = if element.acceptable_msg_types.is_null() {
            vec![vec![]]
        } else {
            let mut acceptable_msg_types_str_list = Vec::new();
            for i in 0..element.recv_ports {
                let s = unsafe { CStr::from_ptr(*element.acceptable_msg_types.offset(i as isize)) }
                    .to_str()?
                    .to_owned();
                acceptable_msg_types_str_list.push(s);
            }
            str_to_msg_types(&acceptable_msg_types_str_list)?
        };
        let acceptable_msg_types = move || acceptable_msg_types_list.clone();

        let ports = (element.recv_ports, element.send_ports);

        self.append(name, builder, Box::new(acceptable_msg_types), ports)?;
        Ok(())
    }
}

struct MoveValue {
    element_ptr: *mut c_void,
    next: unsafe extern "C" fn(
        element: *mut c_void,
        *mut DcPipeline,
        *mut DcMsgReceiver,
    ) -> DcElementResult,
    free: unsafe extern "C" fn(element: *mut c_void),
}

unsafe impl Send for MoveValue {}

impl Drop for MoveValue {
    fn drop(&mut self) {
        unsafe {
            (self.free)(self.element_ptr);
        }
    }
}

pub fn build_plugin_element(
    element: DcElement,
    conf: &ElementConf,
) -> Result<ElementExecutable, Error> {
    let config_format = unsafe { CStr::from_ptr(element.config_format) }.to_str()?;
    let conf_str = match config_format {
        "json" => serde_json::to_string(conf)?,
        "yaml" => serde_yaml::to_string(conf)?,
        _ => {
            bail!("unknown config format \"{}\"", config_format);
        }
    };

    let conf_cstr = CString::new(conf_str)?;
    let element_ptr = unsafe { (element.new)(conf_cstr.as_ptr() as *const _) };

    if element_ptr.is_null() {
        bail!("element plugin new() failed");
    }

    let move_value = MoveValue {
        element_ptr,
        next: element.next,
        free: element.free,
    };

    let next_boxed = move |pipeline: *mut DcPipeline,
                           msg_receiver: *mut DcMsgReceiver|
          -> ElementResult {
        let _ = &move_value;
        let result = unsafe { (move_value.next)(move_value.element_ptr, pipeline, msg_receiver) };
        match result {
            DcElementResult::Close => Ok(ElementValue::Close),
            DcElementResult::MsgBuf => Ok(ElementValue::MsgBuf),
            DcElementResult::Err => Err(crate::error::PluginElementExecutionError.into()),
        }
    };

    let finalizer: Option<ElementFinalizer> = unsafe {
        let mut finalizer = DcFinalizer {
            f: None,
            context: std::ptr::null_mut(),
        };

        if (element.finalizer)(element_ptr, &mut finalizer as _) {
            if finalizer.f.is_some() {
                let finalizer = move || {
                    let _ = &finalizer;
                    if !(finalizer.f.unwrap())(finalizer.context) {
                        bail!("Finalizer execution failed")
                    }
                    Ok(())
                };
                Some(Box::new(finalizer))
            } else {
                None
            }
        } else {
            bail!("Error occured during calling plugin finalizer creator")
        }
    };

    Ok(ElementExecutable {
        next_boxed: Box::new(next_boxed),
        finalizer,
    })
}

pub fn str_to_msg_types(s: &[String]) -> Result<Vec<Vec<MsgType>>, Error> {
    let mut list = Vec::new();

    for s in s {
        let mut list_per_port = Vec::new();

        for t in s.split(',') {
            list_per_port.push(MsgType::from_str(t)?);
        }

        list.push(list_per_port);
    }

    Ok(list)
}
