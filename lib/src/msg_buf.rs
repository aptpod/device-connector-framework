use std::sync::atomic::{AtomicUsize, Ordering};

use byteorder::{ByteOrder, NativeEndian};
use libc::size_t;

use crate::{
    metadata::{DcMetadata, META_DATA_SIZE},
    msg::{DcMsg, Msg},
};

static METADATA_PADDING: AtomicUsize = AtomicUsize::new(0);

pub fn set_metadata_padding(metadata_padding: usize) {
    METADATA_PADDING.store(metadata_padding, Ordering::Relaxed);
}

/// Message buffer
#[repr(C)]
pub struct DcMsgBuf {
    _data: [u8; 0],
    _marker: core::marker::PhantomData<(*mut u8, core::marker::PhantomPinned)>,
}

pub struct MsgBuf {
    buf: Vec<u8>,
    metadata: Vec<DcMetadata>,
}

/// Create a message buffer.
#[no_mangle]
pub extern "C" fn dc_msg_buf_new() -> *mut DcMsgBuf {
    Box::into_raw(Box::new(MsgBuf::new())) as *mut _
}

/// Write data to a message buffer.
#[no_mangle]
pub unsafe extern "C" fn dc_msg_buf_write(msg_buf: *mut DcMsgBuf, data: *const u8, len: size_t) {
    unsafe {
        let msg_buf: &mut MsgBuf = &mut *(msg_buf as *mut MsgBuf);
        let data_slice = std::slice::from_raw_parts(data, len);
        msg_buf.buf.extend_from_slice(data_slice);
    }
}

/// Set metadata to a message buffer.
#[no_mangle]
pub unsafe extern "C" fn dc_msg_buf_set_metadata(msg_buf: *mut DcMsgBuf, metadata: DcMetadata) {
    unsafe {
        let msg_buf: &mut MsgBuf = &mut *(msg_buf as *mut MsgBuf);
        msg_buf.metadata.push(metadata);
    }
}

/// Take message from a message buffer. Clears the buffer.
#[no_mangle]
pub unsafe extern "C" fn dc_msg_buf_take_msg(msg_buf: *mut DcMsgBuf) -> DcMsg {
    unsafe {
        let msg_buf: &mut MsgBuf = &mut *(msg_buf as *mut MsgBuf);
        msg_buf.take_msg().into_raw()
    }
}

/// Get the current bytes length of this buffer.
#[no_mangle]
pub unsafe extern "C" fn dc_msg_buf_get_len(msg_buf: *const DcMsgBuf) -> usize {
    unsafe {
        let msg_buf: &MsgBuf = &*(msg_buf as *const MsgBuf);
        msg_buf.buf.len() - 8
    }
}

/// Free a message buffer.
#[no_mangle]
pub unsafe extern "C" fn dc_msg_buf_free(msg_buf: *mut DcMsgBuf) {
    unsafe {
        let _: Box<MsgBuf> = Box::from_raw(msg_buf as *mut MsgBuf);
    }
}

/// For `BufMut` implementation
/// cbindgen:ignore
#[doc(hidden)]
#[no_mangle]
pub unsafe extern "C" fn dc_msg_buf_advance(msg_buf: *mut DcMsgBuf, cnt: usize) {
    unsafe {
        let msg_buf: &mut MsgBuf = &mut *(msg_buf as *mut MsgBuf);
        let buf = &mut msg_buf.buf;

        let len = buf.len();
        let remaining = buf.capacity() - len;

        assert!(
            cnt <= remaining,
            "cannot advance past `remaining_mut`: {:?} <= {:?}",
            cnt,
            remaining
        );

        buf.set_len(len + cnt);
    }
}

/// For `BufMut` implementation
/// cbindgen:ignore
#[doc(hidden)]
#[no_mangle]
pub unsafe extern "C" fn dc_msg_buf_get_uninit(
    msg_buf: *mut DcMsgBuf,
    data: *mut *mut u8,
    len: *mut usize,
) {
    unsafe {
        let msg_buf: &mut MsgBuf = &mut *(msg_buf as *mut MsgBuf);
        let buf = &mut msg_buf.buf;

        if buf.capacity() == buf.len() {
            buf.reserve(64);
        }

        let cap = buf.capacity();
        let current_len = buf.len();

        let ptr = buf.as_mut_ptr();

        data.write(ptr.add(current_len));
        len.write(cap - current_len);
    }
}

impl MsgBuf {
    pub(crate) fn new() -> Self {
        let mut buf = Vec::with_capacity(256);
        buf.resize(8, 0);
        Self {
            buf,
            metadata: Vec::new(),
        }
    }

    pub(crate) fn take_msg(&mut self) -> Msg {
        let len = self.buf.len() - 8;
        NativeEndian::write_u64(&mut self.buf[0..8], len as u64);

        for metadata in &self.metadata {
            let metadata = metadata.as_array();
            self.buf.extend_from_slice(&metadata);
        }

        let metadata_padding = METADATA_PADDING.load(Ordering::Relaxed);

        if metadata_padding > 0 {
            self.buf
                .resize(self.buf.len() + META_DATA_SIZE * metadata_padding, 0);
        }

        let msg = unsafe { Msg::from_data(&self.buf) };
        self.clear();
        msg
    }

    pub(crate) fn clear(&mut self) {
        self.buf.resize(8, 0);
        self.metadata.clear();
    }
}
