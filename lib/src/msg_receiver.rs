use crate::{channel::MsgRecvPorts, msg::DcMsg, plugin::DcPort, task::Task};

/// Message receiver
#[repr(C)]
pub struct DcMsgReceiver {
    _data: [u8; 0],
    _marker: core::marker::PhantomData<(*mut u8, core::marker::PhantomPinned)>,
}

pub enum MsgReceiver {
    Empty,
    Child(Box<Task>),
    MsgRecvPorts(MsgRecvPorts),
}

/// Receive a message from specified port. Return false if sender task closed or an error occured.
#[no_mangle]
pub unsafe extern "C" fn dc_msg_receiver_recv(
    msg_receiver: *mut DcMsgReceiver,
    port: DcPort,
    msg: *mut DcMsg,
) -> bool {
    let msg_receiver: &mut MsgReceiver = unsafe { &mut *(msg_receiver as *mut MsgReceiver) };

    match msg_receiver {
        MsgReceiver::Empty => {
            core_log!(
                Error,
                "Tried to receive a message from an empty DcMsgReceiver"
            );
            false
        }
        MsgReceiver::Child(child) => {
            if let Some(received_msg) = child.exec_as_child() {
                unsafe { msg.write(received_msg.into_raw()) };
                true
            } else {
                false
            }
        }
        MsgReceiver::MsgRecvPorts(msg_recv_ports) => match msg_recv_ports.recv(port) {
            Ok(Some(received_msg)) => {
                unsafe { msg.write(received_msg.into_raw()) };
                true
            }
            Ok(None) => false,
            Err(e) => {
                core_log!(Error, "{}", e);
                false
            }
        },
    }
}

/// Receive a message. Return false if sender task closed or an error occured.
#[no_mangle]
pub unsafe extern "C" fn dc_msg_receiver_recv_any_port(
    msg_receiver: *mut DcMsgReceiver,
    port: *mut DcPort,
    msg: *mut DcMsg,
) -> bool {
    let msg_receiver: &mut MsgReceiver = unsafe { &mut *(msg_receiver as *mut MsgReceiver) };
    match msg_receiver {
        MsgReceiver::Empty => {
            core_log!(
                Error,
                "Tried to receive a message from an empty DcMsgReceiver"
            );
            false
        }
        MsgReceiver::Child(child) => {
            if let Some(received_msg) = child.exec_as_child() {
                unsafe {
                    port.write(0);
                    msg.write(received_msg.into_raw());
                }
                true
            } else {
                false
            }
        }
        MsgReceiver::MsgRecvPorts(msg_recv_ports) => match msg_recv_ports.recv_any() {
            Ok(Some(result)) => {
                unsafe {
                    port.write(result.0);
                    msg.write(result.1.into_raw());
                }
                true
            }
            Ok(None) => false,
            Err(e) => {
                core_log!(Error, "{}", e);
                false
            }
        },
    }
}
