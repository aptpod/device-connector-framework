use bytes::BufMut;
use libc::size_t;
use std::{marker::PhantomData, mem::ManuallyDrop};

use crate::Port;

/// Dummy type for Vec<u8>
#[repr(C)]
pub struct DcMsgBufInner {}

/// Message buffer
#[repr(C)]
pub struct DcMsgBuf {
    /// Pointer to MsgBufInner
    pub inner: *mut DcMsgBufInner,
    pub port: Port,
}

unsafe impl Send for DcMsgBuf {}

/// Rusty DcMsgBuf for ElementBuildable::next().
pub struct MsgBuf<'a> {
    marker: PhantomData<&'a ()>,
    msg_buf: *mut DcMsgBuf,
}

impl<'a> MsgBuf<'a> {
    /// # Safety
    /// `msg_buf` must be valid.
    pub unsafe fn new(msg_buf: *mut DcMsgBuf) -> Self {
        MsgBuf {
            marker: PhantomData,
            msg_buf,
        }
    }

    /// # Safety
    /// TODO: unsafe implementation because ignores memory allocation method.
    /// This will be solved by stable memory allocator.
    pub unsafe fn as_vec(&self) -> &Vec<u8> {
        &*((*self.msg_buf).inner as *const Vec<u8>)
    }

    /// # Safety
    /// TODO: unsafe implementation because ignores memory allocation method.
    /// This will be solved by stable memory allocator.
    pub unsafe fn as_vec_mut(&mut self) -> &mut Vec<u8> {
        &mut *((*self.msg_buf).inner as *mut Vec<u8>)
    }
}

impl<'a> std::io::Write for MsgBuf<'a> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let v = unsafe { self.as_vec_mut() };
        v.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

unsafe impl<'a> BufMut for MsgBuf<'a> {
    fn remaining_mut(&self) -> usize {
        unsafe {
            let v = self.as_vec();
            v.remaining_mut()
        }
    }

    unsafe fn advance_mut(&mut self, cnt: usize) {
        let v = self.as_vec_mut();
        v.advance_mut(cnt);
    }

    fn chunk_mut(&mut self) -> &mut bytes::buf::UninitSlice {
        let v = unsafe { self.as_vec_mut() };
        v.chunk_mut()
    }
}

/// # Safety
/// `msg_buf` and `data` must be a valid pointer.
#[no_mangle]
pub unsafe extern "C" fn dc_msg_buf_write(msg_buf: *mut DcMsgBuf, data: *const u8, len: size_t) {
    let mut msg_buf = MsgBuf::new(msg_buf);
    let data = std::slice::from_raw_parts(data, len);
    msg_buf.put(data);
}

#[doc(hidden)]
#[repr(C)]
pub union DcMsgInner {
    /// Pointer from Vec<u8>.
    pub owned: *mut u8,
    /// Pointer to buffer.
    pub msg_ref: *const u8,
}

unsafe impl Send for DcMsgInner {}

/// Message passing between tasks
#[repr(C)]
pub struct DcMsg {
    /// Pointer to MsgBufInner
    pub inner: DcMsgInner,
    pub len: usize,
    pub capacity: usize,
    pub drop: Option<unsafe extern "C" fn(*mut u8, usize, usize)>,
}

/// # Safety
/// `msg` must be a valid pointer.
#[no_mangle]
pub unsafe extern "C" fn dc_msg_free(msg: DcMsg) {
    if let Some(drop) = msg.drop {
        drop(msg.inner.owned, msg.len, msg.capacity);
    }
}

/// Rusty DcMsg for ElementBuildable::next().
pub struct Msg<'a> {
    _marker: PhantomData<&'a ()>,
    msg: DcMsg,
}

#[doc(hidden)]
pub struct SendableMsg(pub Msg<'static>);

impl<'a> Msg<'a> {
    /// # Safety
    /// `msg` must be a valid DcMsg and not owned by others.
    pub unsafe fn new(msg: DcMsg) -> Msg<'a> {
        Msg {
            _marker: PhantomData,
            msg,
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.msg.inner.msg_ref, self.msg.len) }
    }

    pub fn into_ffi(self) -> DcMsg {
        let msg = ManuallyDrop::new(self);
        unsafe { std::mem::transmute_copy(&msg.msg) }
    }
}

impl<'a> Drop for Msg<'a> {
    fn drop(&mut self) {
        if let Some(drop) = self.msg.drop {
            unsafe {
                drop(self.msg.inner.owned, self.msg.len, self.msg.capacity);
            }
        }
    }
}
