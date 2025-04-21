use byteorder::{ByteOrder, NativeEndian};
use libc::c_void;
use std::{ptr::NonNull, sync::Arc};

use crate::metadata::{DcMetadata, DcMetadataId, DcMetadataType};

const META_DATA_SIZE: usize = std::mem::size_of::<DcMetadata>();

unsafe impl Send for DcMsg {}

/// Reference counted message.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DcMsg {
    _ptr: NonNull<c_void>,
    _size: usize,
}

/// Wrapper of DcMsg in core
#[repr(transparent)]
pub struct Msg(DcMsg);

impl DcMsg {
    /// Format of data
    /// ```text
    /// 0                                                              32
    /// +---------------+---------------+---------------+---------------+
    /// |                          Size of Data                         +
    /// +---------------+---------------+---------------+---------------+
    /// |                          Size of Data                         +
    /// +---------------+---------------+---------------+---------------+
    /// |                     Data
    /// +---------------+---------------+-----------//--+
    /// |              Array of DcMetadata
    /// +---------------+---------------+-----------//--+
    /// ```
    unsafe fn new(data: &[u8]) -> DcMsg {
        let msg: *const [u8] = Arc::into_raw(Arc::from(data));
        unsafe { Self::from_ptr(msg) }
    }

    unsafe fn from_ptr(data: *const [u8]) -> DcMsg {
        unsafe {
            Self {
                _ptr: NonNull::new_unchecked(data as *const u8 as *mut c_void), // should use data.as_ptr() than cast if it become stable
                _size: data.len(),
            }
        }
    }

    unsafe fn as_ptr(&self) -> *const [u8] {
        std::ptr::slice_from_raw_parts(self._ptr.as_ptr() as *const u8, self._size)
    }
}

/// Clone a DcMsg. Increases the reference counter.
#[no_mangle]
pub unsafe extern "C" fn dc_msg_clone(msg: *const DcMsg) -> DcMsg {
    unsafe {
        Arc::increment_strong_count((*msg).as_ptr());
        *msg
    }
}

/// Free a DcMsg. Decrease the reference counter.
#[no_mangle]
pub unsafe extern "C" fn dc_msg_free(msg: DcMsg) {
    let ptr: *const [u8] = unsafe { msg.as_ptr() };
    let msg: Arc<[u8]> = unsafe { Arc::from_raw(ptr) };
    std::mem::drop(msg);
}

/// Get data from a message.
#[no_mangle]
pub unsafe extern "C" fn dc_msg_get_data(msg: *const DcMsg, data: *mut *const u8, len: *mut usize) {
    unsafe {
        let ptr: *const [u8] = (*msg).as_ptr();
        let slice: &[u8] = &*ptr;
        let data_len = NativeEndian::read_u64(slice) as usize;

        data.write(slice.as_ptr().add(8));
        len.write(data_len);
    }
}

/// Get metadata from a message.
#[no_mangle]
pub unsafe extern "C" fn dc_msg_get_metadata(msg: *const DcMsg, id: DcMetadataId) -> DcMetadata {
    unsafe {
        let ptr: *const [u8] = (*msg).as_ptr();
        let slice: &[u8] = &*ptr;
        let data_len = NativeEndian::read_u64(slice) as usize;

        let mut p = slice.as_ptr().add(data_len + 8);
        let p_end = slice.as_ptr().add(slice.len());

        while p < p_end {
            let metadata: DcMetadata = std::ptr::read_unaligned(p as _);
            if metadata.r#type != DcMetadataType::Empty && metadata.id == id {
                return metadata;
            }
            p = p.add(META_DATA_SIZE);
        }

        std::mem::zeroed()
    }
}

/// Set metadata to a message.
#[no_mangle]
#[allow(clippy::never_loop)]
pub unsafe extern "C" fn dc_msg_set_metadata(msg: *mut DcMsg, metadata: DcMetadata) {
    unsafe {
        let p_msg = msg;
        // let ptr: *const [u8] = std::mem::transmute(*p_msg);
        let ptr: *const [u8] = (*msg).as_ptr();
        let mut msg: Arc<[u8]> = Arc::from_raw(ptr);
        let mut metadata_full = false;

        if let Some(slice) = Arc::get_mut(&mut msg) {
            let data_len = NativeEndian::read_u64(slice) as usize;
            let mut p = slice.as_mut_ptr().add(data_len + 8);
            let p_end = slice.as_mut_ptr().add(slice.len());

            // Find empty or same id metadata and write
            while p < p_end {
                let m: DcMetadata = std::ptr::read_unaligned(p as _);
                if m.r#type == DcMetadataType::Empty || m.id == metadata.id {
                    std::ptr::write_unaligned(p as _, metadata);
                    std::mem::forget(msg);
                    return;
                }
                p = p.add(META_DATA_SIZE);
            }

            metadata_full = true;
        }

        let slice: &[u8] = &*ptr;
        let data_len = NativeEndian::read_u64(slice) as usize;
        let need_extend = if metadata_full {
            true
        } else {
            let mut p = slice.as_ptr().add(data_len + 8);
            let p_end = slice.as_ptr().add(slice.len());
            'find_loop: loop {
                // Find empty metadata or same id metadata
                while p < p_end {
                    let m: DcMetadata = std::ptr::read_unaligned(p as _);
                    if m.r#type == DcMetadataType::Empty || m.id == metadata.id {
                        break 'find_loop false;
                    }
                    p = p.add(META_DATA_SIZE);
                }
                break true;
            }
        };

        let new_msg: Arc<[u8]> = if need_extend {
            let mut buf = Vec::with_capacity(slice.len() + META_DATA_SIZE);
            buf.extend_from_slice(slice);
            let metadata: [u8; META_DATA_SIZE] = std::mem::transmute(metadata);
            buf.extend_from_slice(&metadata);
            Arc::from(buf)
        } else {
            let mut new_msg: Arc<[u8]> = Arc::from(slice);
            if let Some(slice) = Arc::get_mut(&mut new_msg) {
                let mut p = slice.as_mut_ptr().add(data_len + 8);
                let p_end = slice.as_mut_ptr().add(slice.len());

                // Find empty or same id metadata and write
                while p < p_end {
                    let m: DcMetadata = std::ptr::read_unaligned(p as _);
                    if m.r#type == DcMetadataType::Empty || m.id == metadata.id {
                        std::ptr::write_unaligned(p as _, metadata);
                        break;
                    }
                    p = p.add(META_DATA_SIZE);
                }
            } else {
                std::hint::unreachable_unchecked()
            }
            new_msg
        };
        let new_msg: *const [u8] = Arc::into_raw(new_msg);
        *p_msg = DcMsg::from_ptr(new_msg);
        std::mem::drop(msg);
    }
}

impl Msg {
    pub unsafe fn new(msg: DcMsg) -> Self {
        Self(msg)
    }

    pub fn into_raw(self) -> DcMsg {
        let msg = self.0;
        std::mem::forget(self);
        msg
    }

    pub(crate) unsafe fn from_data(data: &[u8]) -> Self {
        Self(unsafe { DcMsg::new(data) })
    }
}

impl Clone for Msg {
    fn clone(&self) -> Self {
        Self(unsafe { dc_msg_clone(&self.0) })
    }
}

impl Drop for Msg {
    fn drop(&mut self) {
        unsafe { dc_msg_free(self.0) };
    }
}
