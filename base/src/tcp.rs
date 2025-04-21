use dc_core::{
    ElementBuildable, ElementResult, ElementValue, Error, MsgReceiver, MsgType, Pipeline, Port,
};
use serde::Deserialize;
use serde_with::{serde_as, DurationMilliSecondsWithFrac};
use std::io::{BufWriter, Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::time::Duration;

/// Read from tcp.
pub struct TcpSrcElement {
    conf: TcpSrcElementConf,
    listener: TcpListener,
    stream: Option<TcpStream>,
    buf: Vec<u8>,
}

/// Configuration type for `TcpSrcElement`
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TcpSrcElementConf {
    /// Socket.
    pub addr: String,
    pub ttl: Option<u32>,
    pub buf_size: Option<usize>,
    #[serde(default)]
    pub retry: bool,
    #[serde_as(as = "Option<DurationMilliSecondsWithFrac<f64>>")]
    pub retry_interval_ms: Option<Duration>,
}

impl ElementBuildable for TcpSrcElement {
    type Config = TcpSrcElementConf;

    const NAME: &'static str = "tcp-src";

    const SEND_PORTS: Port = 1;

    fn send_msg_types() -> Vec<Vec<MsgType>> {
        vec![vec![MsgType::binary()]]
    }

    fn new(conf: Self::Config) -> Result<Self, Error> {
        let listener = TcpListener::bind(&conf.addr)?;
        if let Some(ttl) = conf.ttl {
            listener.set_ttl(ttl)?;
        }
        let buf_size = conf.buf_size.unwrap_or(0xFF);

        Ok(TcpSrcElement {
            conf,
            listener,
            stream: None,
            buf: vec![0; buf_size],
        })
    }

    fn next(&mut self, pipeline: &mut Pipeline, _receiver: &mut MsgReceiver) -> ElementResult {
        'reconnect: loop {
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
                let n = match stream.read(&mut self.buf) {
                    Ok(n) => n,
                    Err(e) => {
                        if !self.conf.retry {
                            return Err(e.into());
                        }
                        log::warn!("io error in tcp-src, retrying: {}", e);
                        if let Some(retry_interval) = self.conf.retry_interval_ms {
                            std::thread::sleep(retry_interval);
                        }
                        self.stream = None;
                        continue 'reconnect;
                    }
                };

                if n > 0 {
                    break n;
                } else {
                    let _ = stream.shutdown(Shutdown::Both);
                    self.stream = None;
                    continue 'reconnect;
                }
            };

            buf.write_all(&self.buf[0..n])?;
            return Ok(ElementValue::MsgBuf);
        }
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

    fn recv_msg_types() -> Vec<Vec<MsgType>> {
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
