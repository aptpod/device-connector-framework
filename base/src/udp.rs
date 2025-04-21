use dc_core::{
    ElementBuildable, ElementResult, ElementValue, Error, MsgReceiver, MsgType, Pipeline, Port,
};
use serde::Deserialize;
use serde_with::{serde_as, DurationMilliSecondsWithFrac};
use std::io::Write;
use std::net::UdpSocket;
use std::time::Duration;

pub struct UdpSrcElement {
    socket: UdpSocket,
    buf: Vec<u8>,
    conf: UdpSrcElementConf,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UdpSrcElementConf {
    bind_addr: String,
    buf_size: Option<usize>,
    #[serde(default)]
    retry: bool,
    #[serde_as(as = "Option<DurationMilliSecondsWithFrac<f64>>")]
    retry_interval_ms: Option<Duration>,
}

impl ElementBuildable for UdpSrcElement {
    type Config = UdpSrcElementConf;

    const NAME: &'static str = "udp-src";

    const SEND_PORTS: Port = 1;

    fn send_msg_types() -> Vec<Vec<MsgType>> {
        vec![vec![MsgType::any()]]
    }

    fn new(conf: Self::Config) -> Result<Self, Error> {
        let socket = UdpSocket::bind(&conf.bind_addr)?;
        let buf_size = conf.buf_size.unwrap_or(0xFFFF);
        Ok(UdpSrcElement {
            socket,
            buf: vec![0; buf_size],
            conf,
        })
    }

    fn next(&mut self, pipeline: &mut Pipeline, _receiver: &mut MsgReceiver) -> ElementResult {
        let data = loop {
            match self.recv() {
                Ok(data) => break data,
                Err(e) => {
                    if !self.conf.retry {
                        return Err(e);
                    }
                    log::warn!("io error in udp-src, retrying: {}", e);
                    if let Some(retry_interval) = self.conf.retry_interval_ms {
                        std::thread::sleep(retry_interval);
                    }
                    loop {
                        match UdpSocket::bind(&self.conf.bind_addr) {
                            Ok(socket) => {
                                self.socket = socket;
                                break;
                            }
                            Err(e) => {
                                log::warn!("io error in udp-src, retrying: {}", e);
                                if let Some(retry_interval) = self.conf.retry_interval_ms {
                                    std::thread::sleep(retry_interval);
                                }
                            }
                        }
                    }
                }
            }
        };

        let mut buf = pipeline.msg_buf(0);
        buf.write_all(data)?;

        Ok(ElementValue::MsgBuf)
    }
}

impl UdpSrcElement {
    fn recv(&mut self) -> Result<&[u8], Error> {
        let size = self.socket.recv(&mut self.buf)?;
        Ok(&self.buf[0..size])
    }
}

pub struct UdpSinkElement {
    socket: UdpSocket,
}

#[derive(Debug, Deserialize)]
#[serde_as]
#[serde(deny_unknown_fields)]
pub struct UdpSinkElementConf {
    bind_addr: Option<String>,
    remote_addr: String,
    broadcast: Option<bool>,
    multicast_loop_v4: Option<bool>,
    multicast_loop_v6: Option<bool>,
    multicast_ttl_v4: Option<u32>,
    ttl: Option<u32>,
    #[serde_as(as = "Option<DurationMilliSecondsWithFrac<f64>>")]
    write_timeout_ms: Option<Duration>,
}

impl ElementBuildable for UdpSinkElement {
    type Config = UdpSinkElementConf;

    const NAME: &'static str = "udp-sink";

    const RECV_PORTS: Port = 1;

    fn recv_msg_types() -> Vec<Vec<MsgType>> {
        vec![vec![MsgType::any()]]
    }

    fn new(conf: Self::Config) -> Result<Self, Error> {
        let bind_addr = conf
            .bind_addr
            .as_ref()
            .map(|s| s.as_ref())
            .unwrap_or("0.0.0.0:0");
        let socket = UdpSocket::bind(bind_addr)?;

        if let Some(broadcast) = conf.broadcast {
            socket.set_broadcast(broadcast)?;
        }
        if let Some(multicast_loop_v4) = conf.multicast_loop_v4 {
            socket.set_multicast_loop_v4(multicast_loop_v4)?;
        }
        if let Some(multicast_loop_v6) = conf.multicast_loop_v6 {
            socket.set_multicast_loop_v4(multicast_loop_v6)?;
        }
        if let Some(multicast_ttl_v4) = conf.multicast_ttl_v4 {
            socket.set_multicast_ttl_v4(multicast_ttl_v4)?;
        }
        if let Some(ttl) = conf.ttl {
            socket.set_ttl(ttl)?;
        }
        socket.set_write_timeout(conf.write_timeout_ms)?;

        socket.connect(&conf.remote_addr)?;

        Ok(UdpSinkElement { socket })
    }

    fn next(&mut self, _pipeline: &mut Pipeline, receiver: &mut MsgReceiver) -> ElementResult {
        loop {
            let msg = receiver.recv(0)?;
            let bytes = msg.as_bytes();
            self.socket.send(bytes)?;
        }
    }
}
