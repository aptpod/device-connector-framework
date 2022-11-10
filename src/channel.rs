use crate::element::Port;
use crate::error::ReceiveError;
// use crate::message::{ReceivedMsg, ReceivedMsgInner, SendableMsg};
use crate::msg_buf::msg_clone;
use crate::task::ChildTask;
use common::{DcMsg, DcMsgReceiver, DcMsgReceiverInner, Msg, SendableMsg};
use crossbeam_channel::{bounded, Receiver, Select, SendError, Sender};

pub struct Channel {
    pub(crate) sender: MsgSender,
    pub(crate) receiver: MsgReceiverInner,
}

impl Channel {
    pub(crate) fn split(self) -> (MsgSender, MsgReceiverInner) {
        (self.sender, self.receiver)
    }
}

pub(crate) struct MsgSender {
    mpsc_channel: Vec<Vec<Sender<SendableMsg>>>,
}

impl MsgSender {
    pub fn send(&self, msg: SendableMsg, port: Port) -> Result<(), SendError<SendableMsg>> {
        let sender = &self.mpsc_channel[port as usize];

        for (i, sender) in sender.iter().enumerate() {
            if i == self.mpsc_channel.len() - 1 {
                sender.send(msg)?;
                return Ok(());
            }
            sender.send(msg_clone(&msg.0))?;
        }
        Ok(())
    }
}

#[derive(Default)]
pub struct MsgReceiverInner {
    child: Option<Box<ChildTask>>,
    recvs: Vec<Receiver<SendableMsg>>,
}

impl MsgReceiverInner {
    /// Receive message from specified port.
    #[allow(clippy::needless_lifetimes)]
    pub fn recv<'a>(&'a mut self, port: Port) -> Result<Msg<'a>, ReceiveError> {
        if let Some(child) = self.child.as_mut() {
            return child.next();
        }

        self.recvs[port as usize]
            .recv()
            .map(|msg| msg.0)
            .map_err(|_| ReceiveError)
    }

    /// Receive message from any port.
    #[allow(clippy::needless_lifetimes)]
    pub fn recv_any_port<'a>(&'a mut self) -> Result<(Port, Msg<'a>), ReceiveError> {
        if let Some(child) = self.child.as_mut() {
            return child.next().map(|result| (0, result));
        }

        let mut sel = Select::new();
        for r in &self.recvs {
            sel.recv(r);
        }
        let oper = sel.select();
        let index = oper.index();
        let msg = oper.recv(&self.recvs[index]).map_err(|_| ReceiveError)?;
        Ok((index as Port, msg.0))
    }

    pub fn into_ffi(self) -> DcMsgReceiver {
        let msg_receiver = Box::new(self);
        DcMsgReceiver {
            inner: Box::into_raw(msg_receiver) as *mut DcMsgReceiverInner,
            recv,
            recv_any,
        }
    }
}

unsafe extern "C" fn recv(inner: *mut DcMsgReceiverInner, port: Port, msg: *mut DcMsg) -> bool {
    let inner: &mut MsgReceiverInner = &mut *(inner as *mut MsgReceiverInner);

    if let Ok(received_msg) = inner.recv(port) {
        *msg = received_msg.into_ffi();
        true
    } else {
        false
    }
}

unsafe extern "C" fn recv_any(
    inner: *mut DcMsgReceiverInner,
    port: *mut Port,
    msg: *mut DcMsg,
) -> bool {
    let inner: &mut MsgReceiverInner = &mut *(inner as *mut MsgReceiverInner);

    if let Ok((p, received_msg)) = inner.recv_any_port() {
        *port = p;
        *msg = received_msg.into_ffi();
        true
    } else {
        false
    }
}

pub struct ChannelBuilder {
    sender: MsgSender,
    self_mpsc: Vec<Option<(Sender<SendableMsg>, Receiver<SendableMsg>)>>,
    pub(crate) child_task: Option<Box<ChildTask>>,
}

impl ChannelBuilder {
    pub fn new(recv_port: Port, send_port: Port) -> ChannelBuilder {
        let recv_port: usize = recv_port.into();
        let send_port: usize = send_port.into();

        ChannelBuilder {
            sender: MsgSender {
                mpsc_channel: vec![vec![]; send_port],
            },
            self_mpsc: std::iter::from_fn(|| Some(None)).take(recv_port).collect(),
            child_task: None,
        }
    }

    pub fn set_sender(&mut self, sender: Sender<SendableMsg>, port: Port) {
        self.sender.mpsc_channel[port as usize].push(sender);
    }

    pub fn get_sender(&mut self, port: Port) -> Sender<SendableMsg> {
        let self_mpsc = &mut self.self_mpsc[port as usize];
        if self_mpsc.is_none() {
            *self_mpsc = Some(bounded(16));
        }
        self_mpsc.as_ref().unwrap().0.clone()
    }

    pub(crate) fn set_child(&mut self, child_task: ChildTask) {
        self.child_task = Some(Box::new(child_task));
    }

    pub fn build(self) -> Channel {
        let recvs = self
            .self_mpsc
            .into_iter()
            .filter_map(|mpsc| mpsc.map(|(_sender, receiver)| receiver))
            .collect();

        Channel {
            sender: self.sender,
            receiver: MsgReceiverInner {
                child: self.child_task,
                recvs,
            },
        }
    }
}
