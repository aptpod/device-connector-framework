use std::ffi::{c_char, c_void, CStr, CString};
use std::ptr::addr_of_mut;

pub use anyhow::{Context, Error};
use serde::de::DeserializeOwned;
use sys::{DcElementResult, DcFinalizer, DcMsgReceiver, DcPipeline, DcPlugin};

use crate::conf::Port;
use crate::{Msg, MsgReceiver, MsgType, Pipeline, ReceiveError};

pub type ElementResult = Result<ElementValue, Error>;
pub type ElementFinalizer = Box<dyn FnOnce() -> Result<(), Error> + Send>;

pub enum ElementValue {
    Close,
    Msg(Port, Msg),
    MsgBuf,
}

pub trait ElementBuildable: Sized + 'static {
    /// Configuration type for this element.
    type Config: DeserializeOwned;

    /// Name of this element. Must be unique in elements.
    const NAME: &'static str;

    /// Description of this element.
    const DESCRIPTION: &'static str = "";

    /// Configuration document of this element.
    const CONFIG_DOC: &'static str = "";

    /// The number of receiving ports.
    const RECV_PORTS: Port = 0;

    /// The number of sending ports.
    const SEND_PORTS: Port = 0;

    /// String metadata ids to use in this element.
    const METADATA_IDS: &'static [&'static str] = &[];

    /// Returns receivable message type of this element.
    fn recv_msg_types() -> Vec<Vec<MsgType>> {
        Vec::new()
    }

    /// Returns send message type of this element.
    fn send_msg_types() -> Vec<Vec<MsgType>> {
        Vec::new()
    }

    /// Create element from config.
    fn new(conf: Self::Config) -> Result<Self, Error>;

    /// Get message from `receiver` and returns the result of this element.
    fn next(&mut self, pipeline: &mut Pipeline, receiver: &mut MsgReceiver) -> ElementResult;

    /// Returns the finalizer of this element.
    fn finalizer(&mut self) -> Result<Option<ElementFinalizer>, Error> {
        Ok(None)
    }
}

/// Configuration struct for elements that don't receive any configuration.
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmptyElementConf {}

#[doc(hidden)]
pub unsafe fn register_element_to_plugin<E: ElementBuildable>(plugin: *mut DcPlugin) {
    unsafe {
        let name =
            CString::new(E::NAME).unwrap_or_else(|_| panic!("Invalid element name: {}", E::NAME));

        let element = sys::dc_element_new(
            name.as_ptr(),
            E::RECV_PORTS,
            E::SEND_PORTS,
            Some(element_new::<E>),
            Some(element_next::<E>),
            Some(element_free::<E>),
        );

        let desc = CString::new(E::DESCRIPTION)
            .unwrap_or_else(|_| panic!("Invalid element description: {}", E::NAME));
        sys::dc_element_set_description(element, desc.as_ptr());

        let config_doc = CString::new(E::CONFIG_DOC)
            .unwrap_or_else(|_| panic!("Invalid config doc: {}", E::NAME));
        sys::dc_element_set_config_doc(element, config_doc.as_ptr());

        for (port, recv_msg_types) in E::recv_msg_types()
            .into_iter()
            .take(Port::MAX as usize)
            .enumerate()
        {
            for recv_msg_type in recv_msg_types {
                let recv_msg_type =
                    CString::new(recv_msg_type.to_string()).expect("Convert recv_msg_type");
                sys::dc_element_append_recv_msg_type(element, port as _, recv_msg_type.as_ptr());
            }
        }

        for (port, send_msg_types) in E::send_msg_types()
            .into_iter()
            .take(Port::MAX as usize)
            .enumerate()
        {
            for send_msg_type in send_msg_types {
                let send_msg_type =
                    CString::new(send_msg_type.to_string()).expect("Convert send_msg_type");
                sys::dc_element_append_send_msg_type(element, port as _, send_msg_type.as_ptr());
            }
        }

        for metadata in E::METADATA_IDS {
            let metadata = CString::new(*metadata).expect("Convert metadata id");
            sys::dc_element_append_metadata_id(element, metadata.as_ptr());
        }

        sys::dc_element_set_finalizer_creator(element, Some(element_finalizer_creater::<E>));
        sys::dc_plugin_register_element(plugin, element);
    }
}

unsafe extern "C-unwind" fn element_new<E: ElementBuildable>(config: *const c_char) -> *mut c_void {
    let config = match unsafe { CStr::from_ptr(config) }.to_str() {
        Ok(config) => config,
        Err(e) => {
            log::error!("invalid string for config: {:?}", e);
            return std::ptr::null_mut();
        }
    };
    let config: E::Config = match serde_json::from_str(config) {
        Ok(config) => config,
        Err(e) => {
            log::error!("invalid config for element {}: {:?}", E::NAME, e);
            return std::ptr::null_mut();
        }
    };

    let element = match E::new(config) {
        Ok(element) => element,
        Err(e) => {
            log::error!("element {} new failed: {:?}", E::NAME, e);
            return std::ptr::null_mut();
        }
    };
    Box::into_raw(Box::new(element)) as *mut _
}

unsafe extern "C-unwind" fn element_next<E: ElementBuildable>(
    element: *mut c_void,
    pipeline: *mut DcPipeline,
    msg_receiver: *mut DcMsgReceiver,
) -> DcElementResult {
    let result = {
        let element: &mut E = unsafe { &mut *(element as *mut E) };
        let pipeline: &mut Pipeline = unsafe { &mut *(pipeline as *mut Pipeline) };
        let msg_receiver: &mut MsgReceiver = unsafe { &mut *(msg_receiver as *mut MsgReceiver) };

        element.next(pipeline, msg_receiver)
    };

    match result {
        Ok(value) => match value {
            ElementValue::Close => sys::DcElementResult_Close,
            ElementValue::Msg(port, msg) => {
                unsafe { sys::dc_pipeline_set_result_msg(pipeline, port, msg.into_raw()) };
                sys::DcElementResult_Msg
            }
            ElementValue::MsgBuf => sys::DcElementResult_MsgBuf,
        },
        Err(e) => {
            // Normal close if ReceiveError
            if e.is::<ReceiveError>() {
                return sys::DcElementResult_Close;
            }
            if let Some(io_error) = e.downcast_ref::<std::io::Error>() {
                if let Some(inner_error) = io_error.get_ref() {
                    if inner_error.is::<ReceiveError>() {
                        return sys::DcElementResult_Close;
                    }
                }
            }

            // Set error message
            let e = format!("{:?}", e);
            let e = CString::new(e).unwrap();

            unsafe { sys::dc_pipeline_set_err_msg(pipeline, e.as_ptr()) };

            sys::DcElementResult_Err
        }
    }
}

unsafe extern "C-unwind" fn element_free<E: ElementBuildable>(element: *mut c_void) {
    let _element: Box<E> = unsafe { Box::from_raw(element as *mut E) };
}

unsafe extern "C-unwind" fn element_finalizer_creater<E: ElementBuildable>(
    element: *mut c_void,
    finalizer: *mut DcFinalizer,
) -> bool {
    let element: &mut E = unsafe { &mut *(element as *mut E) };

    match element.finalizer() {
        Ok(Some(f)) => {
            let boxed_finalizer: Box<ElementFinalizer> = Box::new(f);
            unsafe {
                addr_of_mut!((*finalizer).f).write(Some(call_finalizer));
                addr_of_mut!((*finalizer).context).write(Box::into_raw(boxed_finalizer) as *mut _);
            }
            true
        }
        Ok(None) => true,
        Err(e) => {
            log::error!("Creating finalizer of element {} failed: {:?}", E::NAME, e);
            false
        }
    }
}

unsafe extern "C-unwind" fn call_finalizer(context: *mut c_void) -> bool {
    let finalizer: Box<ElementFinalizer> =
        unsafe { Box::from_raw(context as *mut ElementFinalizer) };
    if let Err(e) = finalizer() {
        log::error!("Finalizer execution failed: {}", e);
        false
    } else {
        true
    }
}
