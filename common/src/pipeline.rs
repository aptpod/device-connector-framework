use crate::{DcMsgBuf, DcMsgType, MsgBuf, MsgType, Port, TypeCheckError};

#[doc(hidden)]
#[repr(C)]
pub struct DcPipelineInner {}

/// Handler for device connector pipeline.
#[repr(C)]
pub struct DcPipeline {
    /// Pointer to `Box<PipelineInner>`
    pub inner: *mut DcPipelineInner,
    pub send_msg_type_checked: unsafe fn(*mut DcPipelineInner) -> bool,
    pub check_send_msg_type: unsafe fn(*mut DcPipelineInner, Port, DcMsgType) -> bool,
    pub msg_buf: unsafe fn(*mut DcPipelineInner) -> *mut DcMsgBuf,
}

unsafe impl Send for DcPipeline {}

/// # Safety
/// `pipeline` must be a valid pointer.
#[no_mangle]
pub unsafe extern "C" fn dc_pipeline_send_msg_type_checked(pipeline: *mut DcPipeline) -> bool {
    let pipeline: &mut DcPipeline = &mut *pipeline;
    (pipeline.send_msg_type_checked)(pipeline.inner)
}

/// # Safety
/// `pipeline` must be a valid pointer.
#[no_mangle]
pub unsafe extern "C" fn dc_pipeline_check_send_msg_type(
    pipeline: *mut DcPipeline,
    port: u8,
    msg_type: DcMsgType,
) -> bool {
    let pipeline: &mut DcPipeline = &mut *pipeline;
    (pipeline.check_send_msg_type)(pipeline.inner, port, msg_type)
}

/// # Safety
/// `pipeline` must be a valid pointer.
#[no_mangle]
pub unsafe extern "C" fn dc_pipeline_msg_buf(pipeline: *mut DcPipeline) -> *mut DcMsgBuf {
    let pipeline: &mut DcPipeline = &mut *pipeline;
    (pipeline.msg_buf)(pipeline.inner)
}

/// Rusty DcPipeline for ElementBuildable::next().
pub struct Pipeline(*mut DcPipeline);

impl Pipeline {
    /// # Safety
    /// `pipeline` must be a valid pointer.
    pub unsafe fn new(pipeline: *mut DcPipeline) -> Self {
        Pipeline(pipeline)
    }

    pub fn send_msg_type_checked(&mut self) -> bool {
        unsafe { dc_pipeline_send_msg_type_checked(self.0) }
    }

    pub fn recheck_send_msg_type(
        &mut self,
        port: Port,
        msg_type: MsgType,
    ) -> Result<(), TypeCheckError> {
        let msg_type = msg_type.into_ffi();
        if unsafe { dc_pipeline_check_send_msg_type(self.0, port, msg_type) } {
            Ok(())
        } else {
            Err(TypeCheckError)
        }
    }

    /// Checks message type to send.
    ///
    /// On the first call, checks message type to send and returns the result. On subsequent calls, always returns `Ok`.
    pub fn check_send_msg_type<F>(&mut self, port: Port, msg_type: F) -> Result<(), TypeCheckError>
    where
        F: FnOnce() -> MsgType,
    {
        if self.send_msg_type_checked() {
            let msg_type = msg_type();
            self.recheck_send_msg_type(port, msg_type)?;
        }
        Ok(())
    }

    #[allow(clippy::needless_lifetimes)]
    pub fn msg_buf<'a>(&'a mut self, port: Port) -> MsgBuf<'a> {
        assert_eq!(port, 0);
        unsafe {
            let pipeline: &mut DcPipeline = &mut *self.0;
            MsgBuf::new((pipeline.msg_buf)(pipeline.inner))
        }
    }
}
