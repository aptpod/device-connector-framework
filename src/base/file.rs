use crate::element::*;
use crate::error::Error;
use crate::MsgType;
use common::{MsgReceiver, Pipeline};
use serde::Deserialize;
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Read, Write};
use std::path::PathBuf;

/// Read from file.
pub struct FileSrcElement {
    file: File,
}

/// Configuration type for `FileSrcElement`
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileSrcElementConf {
    /// File path.
    pub path: PathBuf,
    /// Add write flag when opening a file.
    #[serde(default)]
    pub write_flag: bool,
}

impl ElementBuildable for FileSrcElement {
    type Config = FileSrcElementConf;

    const NAME: &'static str = "file-src";

    const SEND_PORTS: Port = 1;

    fn new(conf: Self::Config) -> Result<Self, Error> {
        let file = if conf.write_flag {
            OpenOptions::new().read(true).write(true).open(&conf.path)?
        } else {
            File::open(&conf.path)?
        };
        Ok(FileSrcElement { file })
    }

    fn next(&mut self, pipeline: &mut Pipeline, _receiver: &mut MsgReceiver) -> ElementResult {
        pipeline.check_send_msg_type(0, MsgType::binary)?;

        let mut buf = pipeline.msg_buf(0);
        let mut read_buf = [0; 0xFF];

        let n = loop {
            let n = self.file.read(&mut read_buf)?;

            if n > 0 {
                break n;
            }
        };
        buf.write_all(&read_buf[0..n])?;
        Ok(ElementValue::MsgBuf)
    }
}

/// Emits received message to a file.
pub struct FileSinkElement {
    conf: FileSinkElementConf,
}

/// Configuration type for `FileSinkElement`
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileSinkElementConf {
    /// File path.
    pub path: PathBuf,
    /// Create new file or not.
    #[serde(default)]
    pub create: bool,
    /// Separator text.
    pub separator: Option<String>,
    /// Buffer flush size.
    #[serde(default)]
    pub flush_size: usize,
}

impl ElementBuildable for FileSinkElement {
    type Config = FileSinkElementConf;

    const NAME: &'static str = "file-sink";

    const RECV_PORTS: Port = 1;

    fn acceptable_msg_types() -> Vec<Vec<MsgType>> {
        vec![vec![MsgType::any()]]
    }

    fn new(conf: Self::Config) -> Result<Self, Error> {
        Ok(FileSinkElement { conf })
    }

    fn next(&mut self, _pipeline: &mut Pipeline, receiver: &mut MsgReceiver) -> ElementResult {
        let file = OpenOptions::new()
            .read(false)
            .write(true)
            .create(self.conf.create)
            .open(&self.conf.path)?;
        let mut file = BufWriter::new(file);

        loop {
            let msg = receiver.recv(0)?;
            let bytes = msg.as_bytes();
            file.write_all(bytes)?;

            if let Some(separator) = self.conf.separator.as_ref() {
                file.write_all(separator.as_bytes())?;
            }

            if self.conf.flush_size == 0 || file.buffer().len() > self.conf.flush_size {
                file.flush()?;
            }
        }
    }
}
