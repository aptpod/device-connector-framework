use std::{marker::PhantomData, mem::MaybeUninit};

use bytes::{buf::UninitSlice, BufMut};
use sys::DcMsgBuf;

use crate::{Metadata, Msg};

/// Message buffer to create `Msg`.
pub struct MsgBuf(*mut DcMsgBuf);

extern "C" {
    fn dc_msg_buf_advance(msg_buf: *mut DcMsgBuf, cnt: usize);
    fn dc_msg_buf_get_uninit(msg_buf: *mut DcMsgBuf, data: *mut *mut u8, len: *mut usize);
}

impl Default for MsgBuf {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl MsgBuf {
    #[inline]
    pub fn new() -> Self {
        Self(unsafe { sys::dc_msg_buf_new() })
    }

    #[inline]
    pub fn extend(&mut self, data: &[u8]) {
        unsafe { sys::dc_msg_buf_write(self.0, data.as_ptr(), data.len()) }
    }

    #[inline]
    pub fn set_metadata(&mut self, metadata: Metadata) {
        let id = metadata.id.into_raw();
        let (type_, value) = metadata.value.into_raw();
        let metadata = sys::DcMetadata { id, type_, value };

        unsafe { sys::dc_msg_buf_set_metadata(self.0, metadata) }
    }

    #[inline]
    pub fn len(&self) -> usize {
        unsafe { sys::dc_msg_buf_get_len(self.0) }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[inline]
    pub fn take_msg(&mut self) -> Msg {
        unsafe { Msg::new(sys::dc_msg_buf_take_msg(self.0)) }
    }
}

impl Drop for MsgBuf {
    fn drop(&mut self) {
        unsafe { sys::dc_msg_buf_free(self.0) }
    }
}

impl std::io::Write for MsgBuf {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.extend(buf);
        Ok(buf.len())
    }

    #[inline]
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }

    #[inline]
    fn write_all(&mut self, buf: &[u8]) -> std::io::Result<()> {
        self.extend(buf);
        Ok(())
    }
}

unsafe impl BufMut for MsgBuf {
    #[inline]
    fn remaining_mut(&self) -> usize {
        isize::MAX as usize - self.len() - 8
    }

    #[inline]
    unsafe fn advance_mut(&mut self, cnt: usize) {
        unsafe { dc_msg_buf_advance(self.0, cnt) };
    }

    #[inline]
    fn chunk_mut(&mut self) -> &mut UninitSlice {
        unsafe {
            let mut data: MaybeUninit<*mut u8> = MaybeUninit::uninit();
            let mut len: MaybeUninit<usize> = MaybeUninit::uninit();
            dc_msg_buf_get_uninit(self.0, data.as_mut_ptr(), len.as_mut_ptr());
            &mut UninitSlice::from_raw_parts_mut(data.assume_init(), len.assume_init())[..]
        }
    }

    #[inline]
    fn put_slice(&mut self, src: &[u8]) {
        self.extend(src);
    }
}

/// Message buffer to create `Msg`.
pub struct MsgBufRef<'a>(*mut DcMsgBuf, PhantomData<&'a mut ()>);

impl MsgBufRef<'_> {
    pub(crate) unsafe fn new(msg_buf: *mut DcMsgBuf) -> Self {
        Self(msg_buf, PhantomData)
    }

    #[inline]
    pub fn extend(&mut self, data: &[u8]) {
        unsafe { sys::dc_msg_buf_write(self.0, data.as_ptr(), data.len()) }
    }

    #[inline]
    pub fn set_metadata(&mut self, metadata: Metadata) {
        let id = metadata.id.into_raw();
        let (type_, value) = metadata.value.into_raw();
        let metadata = sys::DcMetadata { id, type_, value };

        unsafe { sys::dc_msg_buf_set_metadata(self.0, metadata) }
    }

    #[inline]
    pub fn len(&self) -> usize {
        unsafe { sys::dc_msg_buf_get_len(self.0) }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl std::io::Write for MsgBufRef<'_> {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.extend(buf);
        Ok(buf.len())
    }

    #[inline]
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }

    #[inline]
    fn write_all(&mut self, buf: &[u8]) -> std::io::Result<()> {
        self.extend(buf);
        Ok(())
    }
}

unsafe impl BufMut for MsgBufRef<'_> {
    #[inline]
    fn remaining_mut(&self) -> usize {
        isize::MAX as usize - self.len() - 8
    }

    #[inline]
    unsafe fn advance_mut(&mut self, cnt: usize) {
        unsafe { dc_msg_buf_advance(self.0, cnt) };
    }

    #[inline]
    fn chunk_mut(&mut self) -> &mut UninitSlice {
        unsafe {
            let mut data: MaybeUninit<*mut u8> = MaybeUninit::uninit();
            let mut len: MaybeUninit<usize> = MaybeUninit::uninit();
            dc_msg_buf_get_uninit(self.0, data.as_mut_ptr(), len.as_mut_ptr());
            &mut UninitSlice::from_raw_parts_mut(data.assume_init(), len.assume_init())[..]
        }
    }

    #[inline]
    fn put_slice(&mut self, src: &[u8]) {
        self.extend(src);
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use bytes::BufMut;

    #[test]
    fn msg_buf() {
        let mut msg_buf = MsgBuf::new();

        msg_buf.put_u32(10);

        let msg = msg_buf.take_msg();
        assert_eq!(msg.as_bytes(), &[0, 0, 0, 10]);
    }
}
