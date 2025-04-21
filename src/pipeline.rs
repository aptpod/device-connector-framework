use std::{ffi::CString, marker::PhantomData};

use crate::Port;
use sys::DcPipeline;

use crate::{MetadataId, MsgBufRef};

/// `Pipeline` provides interaction with the runtime context.
pub struct Pipeline {
    _marker: PhantomData<*mut ()>,
}

impl Pipeline {
    /// Get `MsgBufRef` for specified port.
    ///
    /// # Panics
    /// This function will panics if given ports have duplicated values.
    #[inline]
    pub fn msg_buf<T: Ports>(&mut self, ports: T) -> T::MsgBufRefs<'_> {
        ports.msg_buf(self)
    }

    /// Get this execution is closing.
    #[inline]
    pub fn closing(&mut self) -> bool {
        let pipeline = self as *mut _ as *mut DcPipeline;
        unsafe { sys::dc_pipeline_get_closing(pipeline) }
    }

    /// Set flag that this execution is closing.
    #[inline]
    pub fn close(&mut self) {
        let pipeline = self as *mut _ as *mut DcPipeline;
        unsafe { sys::dc_pipeline_close(pipeline) }
    }

    /// Get MetadataId from string id.
    ///
    /// # Panics
    ///
    /// Panics if given string is invalid or unknown.
    #[inline]
    pub fn metadata_id(&mut self, string_id: &str) -> MetadataId {
        const BUF_SIZE: usize = 64;

        let pipeline = self as *mut _ as *mut DcPipeline;
        let len = string_id.len();
        let id = if len < BUF_SIZE {
            if string_id.contains('\0') {
                panic!("Invalid string_id");
            }

            let mut buf = [0u8; BUF_SIZE];
            buf[0..len].copy_from_slice(string_id.as_bytes());
            buf[len] = b'\0';
            unsafe { sys::dc_pipeline_get_metadata_id(pipeline, buf.as_ptr() as _) }
        } else {
            let cstr = CString::new(string_id).expect("Invalid string_id");
            unsafe { sys::dc_pipeline_get_metadata_id(pipeline, cstr.as_ptr()) }
        };
        MetadataId::from_raw(id).expect("Cannot get MetadataId")
    }
}

/// Represents one or multiple port numbers.
pub trait Ports: private::Sealed {
    type MsgBufRefs<'a>;
    fn msg_buf(self, pipeline: &mut Pipeline) -> Self::MsgBufRefs<'_>;
}

impl Ports for Port {
    type MsgBufRefs<'a> = MsgBufRef<'a>;
    fn msg_buf(self, pipeline: &mut Pipeline) -> Self::MsgBufRefs<'_> {
        let pipeline = pipeline as *mut _ as *mut DcPipeline;
        unsafe {
            let msg_buf = sys::dc_pipeline_get_msg_buf(pipeline, self);
            MsgBufRef::new(msg_buf)
        }
    }
}

impl Ports for (Port, Port) {
    type MsgBufRefs<'a> = (MsgBufRef<'a>, MsgBufRef<'a>);
    fn msg_buf(self, pipeline: &mut Pipeline) -> Self::MsgBufRefs<'_> {
        assert_ne!(self.0, self.1);

        let pipeline = pipeline as *mut _ as *mut DcPipeline;
        let msg_buf_0 = unsafe {
            let msg_buf = sys::dc_pipeline_get_msg_buf(pipeline, self.0);
            MsgBufRef::new(msg_buf)
        };
        let msg_buf_1 = unsafe {
            let msg_buf = sys::dc_pipeline_get_msg_buf(pipeline, self.1);
            MsgBufRef::new(msg_buf)
        };

        (msg_buf_0, msg_buf_1)
    }
}

impl Ports for (Port, Port, Port) {
    type MsgBufRefs<'a> = (MsgBufRef<'a>, MsgBufRef<'a>, MsgBufRef<'a>);
    fn msg_buf(self, pipeline: &mut Pipeline) -> Self::MsgBufRefs<'_> {
        assert_ne!(self.0, self.1);
        assert_ne!(self.0, self.2);

        let pipeline = pipeline as *mut _ as *mut DcPipeline;
        let msg_buf_0 = unsafe {
            let msg_buf = sys::dc_pipeline_get_msg_buf(pipeline, self.0);
            MsgBufRef::new(msg_buf)
        };
        let msg_buf_1 = unsafe {
            let msg_buf = sys::dc_pipeline_get_msg_buf(pipeline, self.1);
            MsgBufRef::new(msg_buf)
        };
        let msg_buf_2 = unsafe {
            let msg_buf = sys::dc_pipeline_get_msg_buf(pipeline, self.2);
            MsgBufRef::new(msg_buf)
        };

        (msg_buf_0, msg_buf_1, msg_buf_2)
    }
}

mod private {
    use super::Port;

    pub trait Sealed {}

    impl Sealed for Port {}
    impl Sealed for (Port, Port) {}
    impl Sealed for (Port, Port, Port) {}
}
