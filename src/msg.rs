use std::mem::MaybeUninit;

use sys::DcMsg;

use crate::{Metadata, MetadataId, MetadataValue};

/// Reference counted message.
#[repr(transparent)]
pub struct Msg(DcMsg);

unsafe impl Send for Msg {}
unsafe impl Sync for Msg {}

impl Msg {
    /// Create `Msg` from `DcMsg`.
    ///
    /// # Safety
    /// Given `msg` must be valid.
    #[inline]
    pub unsafe fn new(msg: DcMsg) -> Self {
        Self(msg)
    }

    /// Convert `Msg` to `DcMsg`.
    #[inline]
    pub fn into_raw(self) -> DcMsg {
        let msg = self.0;
        std::mem::forget(self);
        msg
    }

    /// Get bytes data from this message.
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        unsafe {
            let mut data: MaybeUninit<*const u8> = MaybeUninit::uninit();
            let mut len: MaybeUninit<usize> = MaybeUninit::uninit();
            sys::dc_msg_get_data(&self.0, data.as_mut_ptr(), len.as_mut_ptr());
            std::slice::from_raw_parts(data.assume_init(), len.assume_init())
        }
    }

    /// Get an metadata.
    #[inline]
    pub fn metadata(&self, id: MetadataId) -> Option<Metadata> {
        let id = id.into_raw();
        let metadata = unsafe { sys::dc_msg_get_metadata(&self.0, id) };

        if let Some(id) = MetadataId::from_raw(metadata.id) {
            let value = unsafe { MetadataValue::from_raw(metadata.type_, metadata.value) };
            Some(Metadata { id, value })
        } else {
            None
        }
    }
}

impl Clone for Msg {
    #[inline]
    fn clone(&self) -> Self {
        Self(unsafe { sys::dc_msg_clone(&self.0) })
    }
}

impl Drop for Msg {
    #[inline]
    fn drop(&mut self) {
        unsafe { sys::dc_msg_free(self.0) };
    }
}

impl std::fmt::Debug for Msg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Msg")
    }
}
