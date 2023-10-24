use crate::element::*;
use crate::error::Error;
use anyhow::{anyhow, bail};
use common::{MsgReceiver, Pipeline};
use once_cell::sync::Lazy;
use serde::Deserialize;
use std::collections::HashMap;
use std::io::Write;
use std::sync::{mpsc, Mutex};

#[allow(clippy::type_complexity)]
static CHANNELS: Lazy<Mutex<HashMap<String, Option<mpsc::Receiver<Vec<u8>>>>>> =
    Lazy::new(Mutex::default);

/// Send passed messages to a `tee-src`.
pub struct TeeFilterElement {
    tx: mpsc::SyncSender<Vec<u8>>,
    conf: TeeFilterElementConf,
}

/// Configuration type for `TeeFilterElement`
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TeeFilterElementConf {
    /// Channel name.
    pub name: String,
    /// Channel capacity
    pub channel_capacity: Option<usize>,
}

impl ElementBuildable for TeeFilterElement {
    type Config = TeeFilterElementConf;

    const NAME: &'static str = "tee-filter";

    const SEND_PORTS: Port = 1;
    const RECV_PORTS: Port = 1;

    fn new(conf: Self::Config) -> Result<Self, Error> {
        let (tx, rx) = mpsc::sync_channel(conf.channel_capacity.unwrap_or(16));

        if CHANNELS
            .lock()
            .unwrap()
            .insert(conf.name.clone(), Some(rx))
            .is_some()
        {
            bail!("tee name duplication detected: \"{}\"", conf.name);
        }

        Ok(Self { tx, conf })
    }

    fn next(&mut self, pipeline: &mut Pipeline, receiver: &mut MsgReceiver) -> ElementResult {
        pipeline.check_send_msg_type(0, MsgType::binary)?;

        let mut buf = pipeline.msg_buf(0);

        let msg = receiver.recv(0)?;
        let bytes = msg.as_bytes();

        self.tx
            .send(bytes.to_vec())
            .map_err(|_| anyhow!("cannot send to tee channel \"{}\"", self.conf.name))?;

        buf.write_all(bytes)?;
        Ok(ElementValue::MsgBuf)
    }
}

/// Receive messages from a `tee-filter`.
pub struct TeeSrcElement {
    rx: Option<mpsc::Receiver<Vec<u8>>>,
    conf: TeeSrcElementConf,
}

/// Configuration type for `ChannelFilterElement`
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TeeSrcElementConf {
    /// Channel name.
    pub name: String,
}

impl ElementBuildable for TeeSrcElement {
    type Config = TeeSrcElementConf;

    const NAME: &'static str = "tee-src";

    const SEND_PORTS: Port = 1;

    fn new(conf: Self::Config) -> Result<Self, Error> {
        Ok(Self { rx: None, conf })
    }

    fn next(&mut self, pipeline: &mut Pipeline, _receiver: &mut MsgReceiver) -> ElementResult {
        pipeline.check_send_msg_type(0, MsgType::binary)?;

        if self.rx.is_none() {
            self.rx = Some(
                CHANNELS
                    .lock()
                    .unwrap()
                    .get_mut(&self.conf.name)
                    .ok_or_else(|| {
                        anyhow!(
                            "unknown tee name \"{}\" specified to tee-src",
                            self.conf.name
                        )
                    })?
                    .take()
                    .ok_or_else(|| {
                        anyhow!("tee name duplication detected: \"{}\"", self.conf.name)
                    })?,
            );
        }

        let rx = self.rx.as_mut().unwrap();
        let msg = match rx.recv() {
            Ok(msg) => msg,
            Err(_) => {
                return Ok(ElementValue::Close);
            }
        };

        let mut buf = pipeline.msg_buf(0);
        buf.write_all(&msg[..])?;

        Ok(ElementValue::MsgBuf)
    }
}
