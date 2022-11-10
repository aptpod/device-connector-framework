use common::{DcMsg, DcMsgBuf, DcMsgInner, Msg, SendableMsg};

const INITIAL_CAP: usize = 1024;

pub struct MsgBufInner {
    buf: Vec<u8>,
}

impl MsgBufInner {
    pub fn new() -> Self {
        MsgBufInner {
            buf: Vec::with_capacity(INITIAL_CAP),
        }
    }

    pub fn clear(&mut self) {
        self.buf.clear();
    }

    pub fn into_ffi(self) -> DcMsgBuf {
        let msg_buf = Box::new(self);

        DcMsgBuf {
            inner: Box::into_raw(msg_buf) as *mut _,
            port: 0,
        }
    }

    /// Get message without clone.
    pub fn get_msg(&self) -> DcMsg {
        DcMsg {
            inner: DcMsgInner {
                msg_ref: self.buf.as_ptr(),
            },
            len: self.buf.len(),
            capacity: 0,
            drop: None,
        }
    }

    pub fn get_msg_cloned(&self) -> DcMsg {
        let mut v = std::mem::ManuallyDrop::new(self.buf.clone());
        DcMsg {
            inner: DcMsgInner {
                owned: v.as_mut_ptr(),
            },
            len: v.len(),
            capacity: v.capacity(),
            drop: Some(msg_cloned_drop),
        }
    }
}

pub fn msg_buf_clear(msg_buf: &mut DcMsgBuf) {
    let inner = unsafe { &mut *(msg_buf.inner as *mut MsgBufInner) };
    inner.clear();
}

unsafe extern "C" fn msg_cloned_drop(p: *mut u8, len: usize, capacity: usize) {
    let _v = Vec::from_raw_parts(p, len, capacity);
}

pub fn msg_clone(msg: &Msg) -> SendableMsg {
    let data: &[u8] = msg.as_bytes();

    let mut v = std::mem::ManuallyDrop::new(data.to_vec());
    let msg = DcMsg {
        inner: DcMsgInner {
            owned: v.as_mut_ptr(),
        },
        len: v.len(),
        capacity: v.capacity(),
        drop: Some(msg_cloned_drop),
    };
    SendableMsg(unsafe { Msg::new(msg) })
}

#[test]
fn msg_buf_test() {
    let mut msg_buf = MsgBufInner::new().into_ffi();
    let p: *mut DcMsgBuf = &mut msg_buf as *mut _;

    let original_data = [10, 20, 30, 40];

    unsafe {
        common::dc_msg_buf_write(p, original_data[0..2].as_ptr(), 2);
        common::dc_msg_buf_write(p, original_data[2..3].as_ptr(), 2);
    }

    let msg = unsafe { (*((*p).inner as *mut MsgBufInner)).get_msg() };

    unsafe {
        let data = std::slice::from_raw_parts(msg.inner.msg_ref, msg.len);
        assert_eq!(data, &original_data);
        common::dc_msg_free(msg);
    }

    let msg = unsafe { (*((*p).inner as *mut MsgBufInner)).get_msg_cloned() };

    unsafe {
        let data = std::slice::from_raw_parts(msg.inner.msg_ref, msg.len);
        assert_eq!(data, &original_data);
        common::dc_msg_free(msg);
    }
}
