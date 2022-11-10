use crate::element::Port;
use crate::error::TypeCheckError;
use crate::task::TaskId;
use crate::type_check::TypeChecker;
use common::{DcMsgBuf, DcMsgType, DcPipeline, DcPipelineInner, MsgType};

/// Pipeline handler from elements.
pub struct PipelineInner {
    pub(crate) self_taskid: Option<TaskId>,
    tc: TypeChecker,
    send_msg_type_checked: bool,
    pub(crate) msg_buf: DcMsgBuf,
}

impl PipelineInner {
    pub fn new(tc: TypeChecker) -> Self {
        PipelineInner {
            self_taskid: None,
            tc,
            send_msg_type_checked: false,
            msg_buf: crate::msg_buf::MsgBufInner::new().into_ffi(),
        }
    }

    pub fn clone(&self) -> Self {
        PipelineInner {
            self_taskid: self.self_taskid,
            tc: self.tc.clone(),
            send_msg_type_checked: self.send_msg_type_checked,
            msg_buf: crate::msg_buf::MsgBufInner::new().into_ffi(),
        }
    }

    pub fn self_taskid(&self) -> TaskId {
        self.self_taskid
            .expect("called self_taskid from out of task")
    }

    /// Checks message type to send.
    pub fn recheck_send_msg_type(
        &mut self,
        port: Port,
        msg_type: MsgType,
    ) -> Result<(), TypeCheckError> {
        self.tc.check(self.self_taskid(), msg_type, port)?;
        self.send_msg_type_checked = true;
        Ok(())
    }

    /// check_send_msg_type() is already called or not.
    pub fn send_msg_type_checked(&self) -> bool {
        self.send_msg_type_checked
    }

    pub fn into_ffi(self) -> DcPipeline {
        let pipeline = Box::new(self);

        DcPipeline {
            inner: Box::into_raw(pipeline) as *mut _,
            send_msg_type_checked,
            check_send_msg_type,
            msg_buf,
        }
    }
}

unsafe fn send_msg_type_checked(inner: *mut DcPipelineInner) -> bool {
    let inner: &mut PipelineInner = &mut *(inner as *mut PipelineInner);
    inner.send_msg_type_checked()
}

unsafe fn check_send_msg_type(
    inner: *mut DcPipelineInner,
    port: Port,
    msg_type: DcMsgType,
) -> bool {
    let inner: &mut PipelineInner = &mut *(inner as *mut PipelineInner);
    let msg_type = MsgType::from_ffi(msg_type);
    inner.recheck_send_msg_type(port, msg_type).is_ok()
}

unsafe fn msg_buf(inner: *mut DcPipelineInner) -> *mut DcMsgBuf {
    let inner: &mut PipelineInner = &mut *(inner as *mut PipelineInner);
    crate::msg_buf::msg_buf_clear(&mut inner.msg_buf);
    &mut inner.msg_buf
}
