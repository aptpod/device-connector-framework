use std::{ffi::CStr, sync::Arc};

use libc::c_char;

use crate::{
    context::Context,
    metadata::DcMetadataId,
    msg::{DcMsg, Msg},
    msg_buf::{DcMsgBuf, MsgBuf},
    plugin::DcPort,
};

/// DcPipeline provides interaction with the runtime context.
#[repr(C)]
pub struct DcPipeline {
    _data: [u8; 0],
    _marker: core::marker::PhantomData<(*mut u8, core::marker::PhantomPinned)>,
}

pub struct Pipeline {
    pub(crate) context: Arc<Context>,
    msg_buf: Vec<(bool, MsgBuf)>,
    pub(crate) msg: Option<(DcPort, Msg)>,
    pub(crate) err_msg: String,
}

impl Pipeline {
    pub fn new(context: Arc<Context>, n_port: DcPort) -> Self {
        Self {
            context,
            msg_buf: (0..n_port).map(|_| (false, MsgBuf::new())).collect(),
            msg: None,
            err_msg: String::new(),
        }
    }

    pub fn take_msg_port_0(&mut self) -> Msg {
        self.msg_buf[0].1.take_msg()
    }

    pub fn take_msgs(&mut self) -> impl Iterator<Item = (DcPort, Msg)> + '_ {
        self.msg_buf
            .iter_mut()
            .enumerate()
            .filter_map(|(i, (flag, msg_buf))| {
                let written = *flag;
                *flag = false;

                if written {
                    Some((i as DcPort, msg_buf.take_msg()))
                } else {
                    None
                }
            })
    }
}

/// Set an error message.
#[no_mangle]
pub unsafe extern "C" fn dc_pipeline_set_err_msg(
    pipeline: *mut DcPipeline,
    err_msg: *const c_char,
) {
    unsafe {
        let pipeline: &mut Pipeline = &mut *(pipeline as *mut Pipeline);
        let err_msg = match CStr::from_ptr(err_msg).to_str() {
            Ok(err_msg) => err_msg,
            Err(e) => {
                core_log!(Error, "invalid error message: {}", e);
                return;
            }
        };
        pipeline.err_msg = err_msg.into();
    }
}

/// Set a message as a result in next() function.
#[no_mangle]
pub unsafe extern "C" fn dc_pipeline_set_result_msg(
    pipeline: *mut DcPipeline,
    port: DcPort,
    msg: DcMsg,
) {
    let pipeline: &mut Pipeline = unsafe { &mut *(pipeline as *mut Pipeline) };
    pipeline.msg = Some((port, unsafe { Msg::new(msg) }));
}

/// Get DcMsgBuf for specified port. MUST NOT specify the port that DcMsgBuf already have been gotten.
#[no_mangle]
pub unsafe extern "C" fn dc_pipeline_get_msg_buf(
    pipeline: *mut DcPipeline,
    port: DcPort,
) -> *mut DcMsgBuf {
    let pipeline: &mut Pipeline = unsafe { &mut *(pipeline as *mut Pipeline) };
    let msg_buf_with_flag = &mut pipeline.msg_buf[port as usize];
    msg_buf_with_flag.0 = true;
    &mut msg_buf_with_flag.1 as *mut MsgBuf as *mut DcMsgBuf
}

/// Get this execution is closing.
#[no_mangle]
pub unsafe extern "C" fn dc_pipeline_get_closing(pipeline: *const DcPipeline) -> bool {
    let pipeline: &Pipeline = unsafe { &*(pipeline as *const Pipeline) };
    pipeline.context.closing()
}

/// Set flag that this execution is closing.
#[no_mangle]
pub unsafe extern "C" fn dc_pipeline_close(pipeline: *mut DcPipeline) {
    let pipeline: &mut Pipeline = unsafe { &mut *(pipeline as *mut Pipeline) };
    pipeline.context.close()
}

/// Get DcMetadataId from string id. Return zero if given string is invalid or unknown.
#[no_mangle]
pub unsafe extern "C" fn dc_pipeline_get_metadata_id(
    pipeline: *const DcPipeline,
    string_id: *const c_char,
) -> DcMetadataId {
    let pipeline: &Pipeline = unsafe { &*(pipeline as *const Pipeline) };
    let string_id = match unsafe { CStr::from_ptr(string_id) }.to_str() {
        Ok(string_id) => string_id,
        Err(e) => {
            core_log!(Error, "invalid string for metadata id: {}", e);
            return 0;
        }
    };

    if let Some(id) = pipeline.context.metadata_id_list.id(string_id) {
        id
    } else {
        core_log!(Error, "unknown metadata id: {}", string_id);
        0
    }
}
