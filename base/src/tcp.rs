use crate::loopback::warn_if_binds_beyond_loopback;
use dc_core::{
    ElementBuildable, ElementResult, ElementValue, Error, MsgReceiver, MsgType, Pipeline, Port,
};
use serde::Deserialize;
use serde_with::{DurationMilliSecondsWithFrac, serde_as};
use std::collections::HashMap;
use std::io::{BufWriter, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::Duration;

/// Read from tcp.
pub struct TcpClientSrcElement {
    conf: TcpClientSrcElementConf,
    connector: Arc<TcpConnector>,
    stream: Option<TcpStream>,
    count: usize,
    buf: Vec<u8>,
}

/// Configuration type for `TcpClientSrcElement`
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TcpClientSrcElementConf {
    /// Socket.
    pub addr: String,
    #[serde(default = "buf_size_default")]
    pub buf_size: usize,
    #[serde(default)]
    pub retry: bool,
    #[serde_as(as = "DurationMilliSecondsWithFrac<f64>")]
    #[serde(default = "retry_interval_default")]
    pub retry_interval_ms: Duration,
    pub ttl: Option<u32>,
}

impl ElementBuildable for TcpClientSrcElement {
    type Config = TcpClientSrcElementConf;

    const NAME: &'static str = "tcp-client-src";
    const DESCRIPTION: &'static str = "Read binary data from a TCP server.";
    const CONFIG_DOC: &'static str = r#"
| Field | Type | Description |
| --- | --- | --- |
| addr | string | Address of the TCP server (e.g., "127.0.0.1:8080"). |
| buf_size | integer | Buffer size for reading data. Default is 255. |
| retry | bool | Retry connection on error. Default is false. |
| retry_interval_ms | number | Retry interval in milliseconds. Default is 5000. |
| ttl | integer | Optional Time-To-Live for the socket. |
"#;

    const SEND_PORTS: Port = 1;

    fn send_msg_types() -> Vec<Vec<MsgType>> {
        vec![vec![MsgType::binary()]]
    }

    fn new(conf: Self::Config) -> Result<Self, Error> {
        let connector = TcpConnector::new(&conf.addr, false)?;
        let buf = vec![0; conf.buf_size];

        Ok(Self {
            conf,
            connector,
            stream: None,
            count: 0,
            buf,
        })
    }

    fn next(&mut self, pipeline: &mut Pipeline, _receiver: &mut MsgReceiver) -> ElementResult {
        let mut need_sleep = false;

        'reconnect: loop {
            let stream = if let Some(stream) = &mut self.stream {
                stream
            } else {
                if need_sleep {
                    std::thread::sleep(self.conf.retry_interval_ms);
                } else {
                    need_sleep = true;
                }
                let (new_count, stream) = match self.connector.connect(self.count) {
                    Ok(result) => result,
                    Err(e) => {
                        if self.conf.retry {
                            log::warn!("io error in tcp-client-src, retrying: {}", e);
                            continue 'reconnect;
                        } else {
                            return Err(e);
                        }
                    }
                };
                self.count = new_count;
                if let Some(ttl) = self.conf.ttl
                    && let Err(e) = stream.set_ttl(ttl)
                {
                    if self.conf.retry {
                        log::warn!("io error in tcp-client-src, retrying: {}", e);
                        continue 'reconnect;
                    } else {
                        return Err(e.into());
                    }
                }
                self.stream = Some(stream);
                self.stream.as_mut().unwrap()
            };

            let mut buf = pipeline.msg_buf(0);

            let n = match stream.read(&mut self.buf) {
                Ok(n) => n,
                Err(e) => {
                    if self.conf.retry {
                        log::warn!("io error in tcp-client-src, retrying: {}", e);
                        self.stream = None;
                        continue 'reconnect;
                    } else {
                        return Err(e.into());
                    }
                }
            };

            if n == 0 {
                let _ = stream.shutdown(Shutdown::Both);
                if self.conf.retry {
                    self.stream = None;
                    continue 'reconnect;
                } else {
                    return Ok(ElementValue::Close);
                }
            }

            buf.write_all(&self.buf[0..n])?;
            return Ok(ElementValue::MsgBuf);
        }
    }
}

/// Write received message to a tcp stream.
pub struct TcpClientSinkElement {
    conf: TcpClientSinkElementConf,
    connector: Arc<TcpConnector>,
}

/// Configuration type for `TcpClientSinkElement`
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde_as]
pub struct TcpClientSinkElementConf {
    /// Socket
    pub addr: String,
    /// Buffer flush size.
    #[serde(default)]
    pub flush_size: usize,
    #[serde(default)]
    pub retry: bool,
    #[serde_as(as = "DurationMilliSecondsWithFrac<f64>")]
    #[serde(default = "retry_interval_default")]
    pub retry_interval_ms: Duration,
    pub ttl: Option<u32>,
}

impl ElementBuildable for TcpClientSinkElement {
    type Config = TcpClientSinkElementConf;

    const NAME: &'static str = "tcp-client-sink";
    const DESCRIPTION: &'static str = "Write data to a TCP server.";
    const CONFIG_DOC: &'static str = r#"
| Field | Type | Description |
| --- | --- | --- |
| addr | string | Address of the TCP server (e.g., "127.0.0.1:8080"). |
| flush_size | integer | Buffer flush size. Flushes automatically if 0. Default is 0. |
| retry | bool | Retry connection on error. Default is false. |
| retry_interval_ms | number | Retry interval in milliseconds. Default is 5000. |
| ttl | integer | Optional Time-To-Live for the socket. |
"#;

    const RECV_PORTS: Port = 1;

    fn recv_msg_types() -> Vec<Vec<MsgType>> {
        vec![vec![MsgType::any()]]
    }

    fn new(conf: Self::Config) -> Result<Self, Error> {
        let connector = TcpConnector::new(&conf.addr, false)?;
        Ok(Self { conf, connector })
    }

    fn next(&mut self, _pipeline: &mut Pipeline, receiver: &mut MsgReceiver) -> ElementResult {
        let mut count = 0;
        let mut need_sleep = false;

        'reconnect: loop {
            if need_sleep {
                std::thread::sleep(self.conf.retry_interval_ms);
            } else {
                need_sleep = true;
            }

            let (new_count, stream) = match self.connector.connect(count) {
                Ok(result) => result,
                Err(e) => {
                    if self.conf.retry {
                        log::warn!("io error in tcp-client-sink, retrying: {}", e);
                        continue 'reconnect;
                    } else {
                        return Err(e);
                    }
                }
            };
            count = new_count;
            if let Some(ttl) = self.conf.ttl
                && let Err(e) = stream.set_ttl(ttl)
            {
                if self.conf.retry {
                    log::warn!("io error in tcp-client-sink, retrying: {}", e);
                    continue 'reconnect;
                } else {
                    return Err(e.into());
                }
            }
            let mut stream = BufWriter::new(stream);

            loop {
                let msg = receiver.recv(0)?;
                let bytes = msg.as_bytes();

                if let Err(e) = stream.write_all(bytes) {
                    if self.conf.retry {
                        log::warn!("io error in tcp-client-sink, retrying: {}", e);
                        continue 'reconnect;
                    } else {
                        return Err(e.into());
                    }
                }

                if (self.conf.flush_size == 0 || stream.buffer().len() > self.conf.flush_size)
                    && let Err(e) = stream.flush()
                {
                    if self.conf.retry {
                        log::warn!("io error in tcp-client-sink, retrying: {}", e);
                        continue 'reconnect;
                    } else {
                        return Err(e.into());
                    }
                }
            }
        }
    }
}

/// Read from tcp as a server.
pub struct TcpServerSrcElement {
    conf: TcpServerSrcElementConf,
    connector: Arc<TcpConnector>,
    stream: Option<TcpStream>,
    count: usize,
    buf: Vec<u8>,
}

/// Configuration type for `TcpServerSrcElement`
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TcpServerSrcElementConf {
    pub bind_addr: String,
    #[serde(default = "buf_size_default")]
    pub buf_size: usize,
    #[serde(default)]
    pub retry: bool,
    #[serde_as(as = "DurationMilliSecondsWithFrac<f64>")]
    #[serde(default = "retry_interval_default")]
    pub retry_interval_ms: Duration,
    pub ttl: Option<u32>,
    #[serde(default)]
    pub suppress_non_loopback_bind_warning: bool,
}

impl ElementBuildable for TcpServerSrcElement {
    type Config = TcpServerSrcElementConf;

    const NAME: &'static str = "tcp-server-src";
    const DESCRIPTION: &'static str = "Read binary data as a TCP server.";
    const CONFIG_DOC: &'static str = r#"
| Field | Type | Description |
| --- | --- | --- |
| bind_addr | string | Address to bind the TCP server (e.g., "127.0.0.1:8080"). |
| buf_size | integer | Buffer size for reading data. Default is 255. |
| retry | bool | Retry connection on error. Default is false. |
| retry_interval_ms | number | Retry interval in milliseconds. Default is 5000. |
| ttl | integer | Optional Time-To-Live for the socket. |
| suppress_non_loopback_bind_warning | bool | Suppress the SECURITY warning emitted when bound to a non-loopback address. Default is false. |
"#;

    const SEND_PORTS: Port = 1;

    fn send_msg_types() -> Vec<Vec<MsgType>> {
        vec![vec![MsgType::binary()]]
    }

    fn new(conf: Self::Config) -> Result<Self, Error> {
        let connector = TcpConnector::new(&conf.bind_addr, true)?;
        warn_if_binds_beyond_loopback(
            Self::NAME,
            connector.local_addr()?,
            conf.suppress_non_loopback_bind_warning,
        );
        let buf = vec![0; conf.buf_size];

        Ok(Self {
            conf,
            connector,
            stream: None,
            count: 0,
            buf,
        })
    }

    fn next(&mut self, pipeline: &mut Pipeline, _receiver: &mut MsgReceiver) -> ElementResult {
        let mut need_sleep = false;

        'reconnect: loop {
            let stream = if let Some(stream) = &mut self.stream {
                stream
            } else {
                if need_sleep {
                    std::thread::sleep(self.conf.retry_interval_ms);
                } else {
                    need_sleep = true;
                }
                let (new_count, stream) = match self.connector.connect(self.count) {
                    Ok(result) => result,
                    Err(e) => {
                        if self.conf.retry {
                            log::warn!("io error in tcp-server-src, retrying: {}", e);
                            continue 'reconnect;
                        } else {
                            return Err(e);
                        }
                    }
                };
                self.count = new_count;
                if let Some(ttl) = self.conf.ttl
                    && let Err(e) = stream.set_ttl(ttl)
                {
                    if self.conf.retry {
                        log::warn!("io error in tcp-server-src, retrying: {}", e);
                        continue 'reconnect;
                    } else {
                        return Err(e.into());
                    }
                }
                self.stream = Some(stream);
                self.stream.as_mut().unwrap()
            };

            let mut buf = pipeline.msg_buf(0);

            let n = match stream.read(&mut self.buf) {
                Ok(n) => n,
                Err(e) => {
                    if self.conf.retry {
                        log::warn!("io error in tcp-server-src, retrying: {}", e);
                        self.stream = None;
                        continue 'reconnect;
                    } else {
                        return Err(e.into());
                    }
                }
            };

            if n == 0 {
                let _ = stream.shutdown(Shutdown::Both);
                if self.conf.retry {
                    self.stream = None;
                    log::warn!("stream closed in tcp-server-src, retrying");
                    continue 'reconnect;
                } else {
                    return Ok(ElementValue::Close);
                }
            }

            buf.write_all(&self.buf[0..n])?;
            return Ok(ElementValue::MsgBuf);
        }
    }
}

/// Write received message to a tcp stream as a server.
pub struct TcpServerSinkElement {
    conf: TcpServerSinkElementConf,
    connector: Arc<TcpConnector>,
}

/// Configuration type for `TcpServerSinkElement`
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde_as]
pub struct TcpServerSinkElementConf {
    pub bind_addr: String,
    pub ttl: Option<u32>,
    /// Buffer flush size.
    #[serde(default)]
    pub flush_size: usize,
    #[serde(default)]
    pub retry: bool,
    #[serde_as(as = "DurationMilliSecondsWithFrac<f64>")]
    #[serde(default = "retry_interval_default")]
    pub retry_interval_ms: Duration,
    #[serde(default)]
    pub suppress_non_loopback_bind_warning: bool,
}

impl ElementBuildable for TcpServerSinkElement {
    type Config = TcpServerSinkElementConf;

    const NAME: &'static str = "tcp-server-sink";
    const DESCRIPTION: &'static str = "Write data as a TCP server.";
    const CONFIG_DOC: &'static str = r#"
| Field | Type | Description |
| --- | --- | --- |
| bind_addr | string | Address to bind the TCP server (e.g., "127.0.0.1:8080"). |
| flush_size | integer | Buffer flush size. Flushes automatically if 0. Default is 0. |
| retry | bool | Retry connection on error. Default is false. |
| retry_interval_ms | number | Retry interval in milliseconds. Default is 5000. |
| ttl | integer | Optional Time-To-Live for the socket. |
| suppress_non_loopback_bind_warning | bool | Suppress the SECURITY warning emitted when bound to a non-loopback address. Default is false. |
"#;

    const RECV_PORTS: Port = 1;

    fn recv_msg_types() -> Vec<Vec<MsgType>> {
        vec![vec![MsgType::any()]]
    }

    fn new(conf: Self::Config) -> Result<Self, Error> {
        let connector = TcpConnector::new(&conf.bind_addr, true)?;
        warn_if_binds_beyond_loopback(
            Self::NAME,
            connector.local_addr()?,
            conf.suppress_non_loopback_bind_warning,
        );
        Ok(Self { conf, connector })
    }

    fn next(&mut self, _pipeline: &mut Pipeline, receiver: &mut MsgReceiver) -> ElementResult {
        let mut count = 0;
        let mut need_sleep = false;

        'reconnect: loop {
            if need_sleep {
                std::thread::sleep(self.conf.retry_interval_ms);
            } else {
                need_sleep = true;
            }

            let (new_count, stream) = match self.connector.connect(count) {
                Ok(result) => result,
                Err(e) => {
                    if self.conf.retry {
                        log::warn!("io error in tcp-server-sink, retrying: {}", e);
                        continue 'reconnect;
                    } else {
                        return Err(e);
                    }
                }
            };
            count = new_count;
            if let Some(ttl) = self.conf.ttl
                && let Err(e) = stream.set_ttl(ttl)
            {
                if self.conf.retry {
                    log::warn!("io error in tcp-server-sink, retrying: {}", e);
                    continue 'reconnect;
                } else {
                    return Err(e.into());
                }
            }
            let mut stream = BufWriter::new(stream);

            loop {
                let msg = receiver.recv(0)?;
                let bytes = msg.as_bytes();

                if let Err(e) = stream.write_all(bytes) {
                    if self.conf.retry {
                        log::warn!("io error in tcp-server-sink, retrying: {}", e);
                        continue 'reconnect;
                    } else {
                        return Err(e.into());
                    }
                }

                if (self.conf.flush_size == 0 || stream.buffer().len() > self.conf.flush_size)
                    && let Err(e) = stream.flush()
                {
                    if self.conf.retry {
                        log::warn!("io error in tcp-server-sink, retrying: {}", e);
                        continue 'reconnect;
                    } else {
                        return Err(e.into());
                    }
                }
            }
        }
    }
}

#[derive(Default)]
struct TcpConnector {
    listener: Option<TcpListener>,
    addr: String,
    stream: Mutex<(usize, Option<TcpStream>)>,
}

static CONNECTOR_MAP: LazyLock<Mutex<HashMap<(String, bool), Arc<TcpConnector>>>> =
    LazyLock::new(Mutex::default);

impl TcpConnector {
    fn new(addr: &str, listen: bool) -> Result<Arc<Self>, Error> {
        let mut map = CONNECTOR_MAP.lock().expect("tcp map lock");

        if let Some(connector) = map.get(&(addr.to_owned(), listen)) {
            return Ok(connector.clone());
        }

        let listener = if listen {
            Some(TcpListener::bind(addr)?)
        } else {
            None
        };
        let connector = Arc::new(TcpConnector {
            listener,
            addr: addr.to_owned(),
            stream: Mutex::default(),
        });
        map.insert((addr.to_owned(), listen), connector.clone());

        Ok(connector)
    }

    fn connect(&self, old_counter: usize) -> Result<(usize, TcpStream), Error> {
        let mut lock = self.stream.lock().expect("stream lock");
        let (counter, stream) = &mut *lock;

        if *counter <= old_counter {
            *stream = None;
        }

        if stream.is_none() {
            if let Some(listener) = &self.listener {
                let (new_stream, addr) = listener.accept()?;
                log::debug!("accept tcp stream from {}", addr);
                *stream = Some(new_stream);
            } else {
                let new_stream = TcpStream::connect(&self.addr)?;
                *stream = Some(new_stream);
            }
            *counter += 1;
        }

        Ok((*counter, stream.as_ref().unwrap().try_clone()?))
    }

    fn local_addr(&self) -> Result<SocketAddr, Error> {
        Ok(self
            .listener
            .as_ref()
            .expect("local_addr called on a non-listening TcpConnector")
            .local_addr()?)
    }
}

fn buf_size_default() -> usize {
    0xFF
}

fn retry_interval_default() -> Duration {
    Duration::from_secs(5)
}

/// Read from tcp.
/// Deprecated
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
    #[serde(default)]
    pub suppress_non_loopback_bind_warning: bool,
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
        warn_if_binds_beyond_loopback(
            Self::NAME,
            listener.local_addr()?,
            conf.suppress_non_loopback_bind_warning,
        );
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

            #[allow(clippy::never_loop)]
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
/// Deprecated
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
