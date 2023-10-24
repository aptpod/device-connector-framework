use std::io::Write;

use crate::element::*;
use crate::error::Error;
use anyhow::bail;
use common::{MsgReceiver, MsgType, Pipeline};
use serde::Deserialize;

/// Split data by fixed size
pub struct SplitByFixedSizeFilterElement {
    conf: SplitByFixedSizeFilterElementConfig,
    buf: Vec<u8>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
/// Configuration type for `FixedSizeFilterElement`
pub struct SplitByFixedSizeFilterElementConfig {
    /// Byte size to split
    pub size: usize,
}

impl ElementBuildable for SplitByFixedSizeFilterElement {
    type Config = SplitByFixedSizeFilterElementConfig;

    const NAME: &'static str = "split-by-fixed-size-filter";
    const RECV_PORTS: Port = 1;
    const SEND_PORTS: Port = 1;

    fn acceptable_msg_types() -> Vec<Vec<MsgType>> {
        vec![vec![MsgType::any()]]
    }

    fn new(conf: Self::Config) -> Result<Self, Error> {
        if conf.size == 0 {
            bail!("specified size must not be zero");
        }

        let buf = Vec::with_capacity(conf.size * 2);
        Ok(Self { conf, buf })
    }

    fn next(&mut self, pipeline: &mut Pipeline, receiver: &mut MsgReceiver) -> ElementResult {
        pipeline.check_send_msg_type(0, MsgType::binary)?;

        loop {
            if self.buf.len() < self.conf.size {
                let msg = receiver.recv(0)?;
                let msg_bytes = msg.as_bytes();

                if self.buf.len() + msg_bytes.len() >= self.conf.size {
                    let mut buf = pipeline.msg_buf(0);
                    let i = self.conf.size - self.buf.len();

                    buf.write_all(&self.buf)?;
                    buf.write_all(&msg_bytes[0..i])?;

                    self.buf.clear();
                    if i > 0 {
                        self.buf.extend_from_slice(&msg_bytes[i..]);
                    }
                    return Ok(ElementValue::MsgBuf);
                }
                self.buf.extend_from_slice(msg_bytes);
            } else {
                let mut buf = pipeline.msg_buf(0);
                buf.write_all(&self.buf[0..self.conf.size])?;

                let remaining = self.buf.len() - self.conf.size;
                if remaining > 0 {
                    self.buf.copy_within(self.conf.size.., 0);
                }
                self.buf.resize_with(remaining, || unreachable!());
                return Ok(ElementValue::MsgBuf);
            }
        }
    }
}
