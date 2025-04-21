use std::collections::HashMap;

use anyhow::{anyhow, Error};
use crossbeam::channel::{bounded, Receiver, Select, Sender};
use dc_common::conf::{TaskId, TaskPort};

use crate::{conf::Port, msg::Msg};

pub type MsgTx = Sender<Msg>;
pub type MsgRx = Receiver<Msg>;

#[derive(Debug)]
pub struct MsgSendPorts {
    channels: Vec<Vec<MsgTx>>,
    self_id: TaskId,
    to: Vec<Vec<TaskPort>>,
}

#[derive(Debug)]
pub struct MsgRecvPorts {
    channels: Vec<MsgRx>,
    self_id: TaskId,
}

impl MsgSendPorts {
    pub fn send(&self, port: Port, msg: Msg) -> Result<(), Error> {
        let senders = self.channels.get(port as usize).ok_or_else(|| {
            core_log!(Error, "invalid port #{} to send in {}", port, self.self_id);
            anyhow!("Invalid port #{} to send in {}", port, self.self_id)
        })?;
        let len = senders.len();
        if len == 1 {
            senders[0]
                .send(msg)
                .map_err(|_| anyhow!("Channel to {} disconnected", self.to[port as usize][0]))?;
        } else {
            for (i, sender) in senders.iter().enumerate() {
                if i == len - 1 {
                    sender.send(msg).map_err(|_| {
                        anyhow!("Channel to {} disconnected", self.to[port as usize][i])
                    })?;
                    break;
                } else {
                    sender.send(msg.clone()).map_err(|_| {
                        anyhow!("Channel to {} disconnected", self.to[port as usize][i])
                    })?;
                }
            }
        }
        Ok(())
    }
}

impl MsgRecvPorts {
    pub fn recv(&mut self, port: Port) -> Result<Option<Msg>, Error> {
        let receiver = self
            .channels
            .get(port as usize)
            .ok_or_else(|| anyhow!("Invalid port #{} to receive in {}", port, self.self_id))?;
        Ok(receiver.recv().ok())
    }

    pub fn recv_any(&mut self) -> Result<Option<(Port, Msg)>, Error> {
        let mut select = Select::new();

        for receiver in &self.channels {
            select.recv(receiver);
        }

        let op = select.select();
        let i = op.index();
        Ok(op.recv(&self.channels[i]).ok().map(|msg| (i as _, msg)))
    }
}

#[derive(Debug)]
pub struct TaskGroupWithChannelInfo {
    pub from: Vec<Vec<TaskPort>>,
    pub to: Vec<Vec<TaskPort>>,
    pub last: TaskId,
}

pub fn create_channels(
    task_groups: &HashMap<TaskId, TaskGroupWithChannelInfo>,
    default_cap: usize,
) -> HashMap<TaskId, (Option<MsgSendPorts>, Option<MsgRecvPorts>)> {
    let mut channels: HashMap<TaskPort, (MsgTx, Option<MsgRx>)> = HashMap::default();

    for (id, task_group) in task_groups {
        for i in 0..task_group.from.len() {
            let (tx, rx) = bounded(default_cap);
            channels.insert(TaskPort(id.clone(), i as _), (tx, Some(rx)));
        }
    }

    let mut map = HashMap::default();

    for (id, task_group) in task_groups {
        let mut recvs = Vec::new();
        for i in 0..task_group.from.len() {
            let recv_port = TaskPort(id.clone(), i as _);
            recvs.push(channels.get_mut(&recv_port).unwrap().1.take().unwrap());
        }
        let msg_recv_ports = if recvs.is_empty() {
            None
        } else {
            Some(MsgRecvPorts {
                channels: recvs,
                self_id: task_group.last.clone(),
            })
        };

        let mut sends = Vec::new();
        for to in &task_group.to {
            sends.push(
                to.iter()
                    .map(|task_port| {
                        let root_task_id = task_groups
                            .iter()
                            .find(|(_, info)| info.last == task_port.0)
                            .unwrap()
                            .0;
                        channels
                            .get(&TaskPort(root_task_id.clone(), task_port.1))
                            .unwrap()
                            .0
                            .clone()
                    })
                    .collect::<Vec<_>>(),
            );
        }

        let msg_send_ports = if sends.is_empty() {
            None
        } else {
            Some(MsgSendPorts {
                channels: sends,
                self_id: id.clone(),
                to: task_group.to.clone(),
            })
        };

        map.insert(id.clone(), (msg_send_ports, msg_recv_ports));
    }

    map
}
