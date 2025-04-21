use std::{marker::PhantomData, mem::MaybeUninit};

use crate::Port;
use sys::{DcMsg, DcMsgReceiver};

use crate::Msg;

/// Receive messages from ports.
pub struct MsgReceiver {
    _marker: PhantomData<*mut ()>,
}

impl MsgReceiver {
    #[inline]
    pub fn recv(&mut self, port: Port) -> Result<Msg, ReceiveError> {
        let msg_receiver = self as *mut _ as *mut DcMsgReceiver;

        unsafe {
            let mut msg: MaybeUninit<DcMsg> = MaybeUninit::uninit();
            if sys::dc_msg_receiver_recv(msg_receiver, port, msg.as_mut_ptr()) {
                Ok(Msg::new(msg.assume_init()))
            } else {
                Err(ReceiveError)
            }
        }
    }

    #[inline]
    pub fn recv_any_port(&mut self) -> Result<(Port, Msg), ReceiveError> {
        let msg_receiver = self as *mut _ as *mut DcMsgReceiver;

        unsafe {
            let mut port: MaybeUninit<Port> = MaybeUninit::uninit();
            let mut msg: MaybeUninit<DcMsg> = MaybeUninit::uninit();
            if sys::dc_msg_receiver_recv_any_port(msg_receiver, port.as_mut_ptr(), msg.as_mut_ptr())
            {
                Ok((port.assume_init(), Msg::new(msg.assume_init())))
            } else {
                Err(ReceiveError)
            }
        }
    }

    #[inline]
    pub fn reader<'r, 'b>(&'r mut self, buf: &'b mut MsgReceiverBuf) -> MsgReceiverReader<'r, 'b> {
        MsgReceiverReader {
            port: buf.port,
            receiver: self,
            buf,
        }
    }
}

/// Buffer for `MsgReceiverReader`
pub struct MsgReceiverBuf {
    port: Port,
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
pub struct MsgReceiverReader<'r, 'b> {
    port: u8,
    receiver: &'r mut MsgReceiver,
    buf: &'b mut MsgReceiverBuf,
}

impl std::io::Read for MsgReceiverReader<'_, '_> {
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

/// Receive error from `MsgReceiver`.
#[derive(Clone, Copy, Debug, thiserror::Error)]
#[error("receive error")]
pub struct ReceiveError;
