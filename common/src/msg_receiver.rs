use crate::{DcMsg, Msg, Port, ReceiveError};

#[doc(hidden)]
#[repr(C)]
pub struct DcMsgReceiverInner {}

/// Handler for device connector pipeline.
#[repr(C)]
pub struct DcMsgReceiver {
    /// Pointer to Box<MsgReceiverInner>
    pub inner: *mut DcMsgReceiverInner,
    pub recv: unsafe extern "C" fn(*mut DcMsgReceiverInner, Port, *mut DcMsg) -> bool,
    pub recv_any: unsafe extern "C" fn(*mut DcMsgReceiverInner, *mut Port, *mut DcMsg) -> bool,
}

unsafe impl Send for DcMsgReceiver {}

/// # Safety
/// `pipeline` and `msg` must be a valid pointer.
#[no_mangle]
pub unsafe extern "C" fn dc_msg_receiver_recv(
    msg_receiver: *mut DcMsgReceiver,
    port: Port,
    msg: *mut DcMsg,
) -> bool {
    let msg_receiver: &mut DcMsgReceiver = &mut *msg_receiver;
    (msg_receiver.recv)(msg_receiver.inner, port, msg)
}

/// # Safety
/// `pipeline`, `port` and `msg` must be a valid pointer.
#[no_mangle]
pub unsafe extern "C" fn dc_msg_receiver_recv_any(
    msg_receiver: *mut DcMsgReceiver,
    port: *mut Port,
    msg: *mut DcMsg,
) -> bool {
    let msg_receiver: &mut DcMsgReceiver = &mut *msg_receiver;
    (msg_receiver.recv_any)(msg_receiver.inner, port, msg)
}

/// Rusty DcMsgReceiver for ElementBuildable::next().

pub struct MsgReceiver(*mut DcMsgReceiver);

impl MsgReceiver {
    /// # Safety
    /// `msg_receiver` must be a valid pointer.
    pub unsafe fn new(msg_receiver: *mut DcMsgReceiver) -> Self {
        MsgReceiver(msg_receiver)
    }

    #[allow(clippy::needless_lifetimes)]
    pub fn recv<'a>(&'a mut self, port: Port) -> Result<Msg<'a>, ReceiveError> {
        unsafe {
            let mut msg: DcMsg = std::mem::zeroed();

            if dc_msg_receiver_recv(self.0, port, &mut msg) {
                Ok(Msg::new(msg))
            } else {
                Err(ReceiveError)
            }
        }
    }

    #[allow(clippy::needless_lifetimes)]
    pub fn recv_any_port<'a>(&'a mut self) -> Result<(Port, Msg<'a>), ReceiveError> {
        unsafe {
            let mut msg: DcMsg = std::mem::zeroed();
            let mut port: Port = 0;

            if dc_msg_receiver_recv_any(self.0, &mut port, &mut msg) {
                Ok((port, Msg::new(msg)))
            } else {
                Err(ReceiveError)
            }
        }
    }

    pub fn read<'r, 'b>(&'r mut self, buf: &'b mut MsgReceiverBuf) -> MsgReceiverRead<'r, 'b> {
        MsgReceiverRead {
            port: buf.port,
            receiver: self,
            buf,
        }
    }
}

/// Buffer for `MsgReceiverRead`
pub struct MsgReceiverBuf {
    port: u8,
    i: usize,
    buf: Vec<u8>,
}

impl MsgReceiverBuf {
    pub fn new(port: Port) -> MsgReceiverBuf {
        MsgReceiverBuf {
            port,
            i: 0,
            buf: Vec::with_capacity(0xFF),
        }
    }
}

/// Wrapper type to implement `Read` for `MsgReceiver`
pub struct MsgReceiverRead<'r, 'b> {
    port: u8,
    receiver: &'r mut MsgReceiver,
    buf: &'b mut MsgReceiverBuf,
}

impl<'r, 'b> std::io::Read for MsgReceiverRead<'r, 'b> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.buf.buf.is_empty() {
            let msg = self
                .receiver
                .recv(self.port)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            let msg_bytes = msg.as_bytes();

            let msg_len = msg_bytes.len();
            let buf_len = buf.len();

            if msg_len <= buf_len {
                buf[..msg_len].copy_from_slice(msg_bytes);
                Ok(msg_len)
            } else {
                buf.copy_from_slice(&msg_bytes[..buf_len]);
                self.buf.buf.resize(msg_len - buf_len, 0);
                self.buf.buf.copy_from_slice(&msg_bytes[buf_len..]);
                self.buf.i = 0;
                Ok(buf_len)
            }
        } else {
            let buf_len = self.buf.buf.len() - self.buf.i;
            let output_buf_len = buf.len();
            if buf_len <= output_buf_len {
                buf[..buf_len].copy_from_slice(&self.buf.buf[self.buf.i..]);
                self.buf.buf.clear();
                Ok(buf_len)
            } else {
                buf.copy_from_slice(&self.buf.buf[self.buf.i..(self.buf.i + output_buf_len)]);
                self.buf.i += output_buf_len;
                Ok(output_buf_len)
            }
        }
    }
}
