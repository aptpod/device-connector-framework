use crate::element::*;
use crate::error::Error;
use crate::MsgType;
use common::{MsgReceiver, Pipeline};
use serde::Deserialize;
use std::io::{BufWriter, Read, Write};
use std::net::{TcpListener, TcpStream};

/// Read from tcp.
pub struct TcpSrcElement {
    listener: TcpListener,
    stream: Option<TcpStream>,
    buf: Vec<u8>,
}

/// Configuration type for `TcpSrcElement`
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TcpSrcElementConf {
    /// Socket.
    pub addr: String,
    pub ttl: Option<u32>,
    pub buf_size: Option<usize>,
}

impl ElementBuildable for TcpSrcElement {
    type Config = TcpSrcElementConf;

    const NAME: &'static str = "tcp-src";

    const SEND_PORTS: Port = 1;

    fn new(conf: Self::Config) -> Result<Self, Error> {
        let listener = TcpListener::bind(&conf.addr)?;
        if let Some(ttl) = conf.ttl {
            listener.set_ttl(ttl)?;
        }
        let buf_size = conf.buf_size.unwrap_or(0xFF);

        Ok(TcpSrcElement {
            listener,
            stream: None,
            buf: vec![0; buf_size],
        })
    }

    fn next(&mut self, pipeline: &mut Pipeline, _receiver: &mut MsgReceiver) -> ElementResult {
        pipeline.check_send_msg_type(0, MsgType::binary)?;

        let stream = if let Some(stream) = &mut self.stream {
            stream
        } else {
            let (stream, addr) = self.listener.accept()?;
            log::trace!("accept tcp stream from {}", addr);
            self.stream = Some(stream);
            self.stream.as_mut().unwrap()
        };

        let mut buf = pipeline.msg_buf(0);

        let n = loop {
            let n = stream.read(&mut self.buf)?;

            if n > 0 {
                break n;
            }
        };
        buf.write_all(&self.buf[0..n])?;
        Ok(ElementValue::MsgBuf)
    }
}

/// Write received message to a tcp stream.
pub struct TcpSinkElement {
    conf: TcpSinkElementConf,
}

/// Configuration type for `TcpSinkElement`
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TcpSinkElementConf {
    /// Socket
    pub addr: String,
    pub ttl: Option<u32>,
    /// Buffer flush size.
    #[serde(default)]
    pub flush_size: usize,
}

impl ElementBuildable for TcpSinkElement {
    type Config = TcpSinkElementConf;

    const NAME: &'static str = "tcp-sink";

    const RECV_PORTS: Port = 1;

    fn acceptable_msg_types() -> Vec<Vec<MsgType>> {
        vec![vec![MsgType::any()]]
    }

    fn new(conf: Self::Config) -> Result<Self, Error> {
        Ok(TcpSinkElement { conf })
    }

    fn next(&mut self, _pipeline: &mut Pipeline, receiver: &mut MsgReceiver) -> ElementResult {
        let stream = TcpStream::connect(&self.conf.addr)?;
        if let Some(ttl) = self.conf.ttl {
            stream.set_ttl(ttl)?;
        }
        let mut stream = BufWriter::new(stream);

        loop {
            let msg = receiver.recv(0)?;
            let bytes = msg.as_bytes();
            stream.write_all(bytes)?;

            if self.conf.flush_size == 0 || stream.buffer().len() > self.conf.flush_size {
                stream.flush()?;
            }
        }
    }
}
