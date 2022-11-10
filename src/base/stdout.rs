use crate::element::*;
use crate::error::Error;
use common::{MsgReceiver, MsgType, Pipeline};
use serde_derive::Deserialize;
use std::io::{BufWriter, Write};

/// Emits received message to stdout.
pub struct StdoutSinkElement {
    stdout: BufWriter<std::io::Stdout>,
    conf: StdoutSinkElementConf,
}

/// Configuration type for `StdoutSinkElement`
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StdoutSinkElementConf {
    /// Separator text.
    pub separator: Option<String>,
}

impl ElementBuildable for StdoutSinkElement {
    type Config = StdoutSinkElementConf;

    const NAME: &'static str = "stdout-sink";

    const RECV_PORTS: Port = 1;

    fn acceptable_msg_types() -> Vec<Vec<MsgType>> {
        vec![vec![MsgType::any()]]
    }

    fn new(conf: Self::Config) -> Result<Self, Error> {
        Ok(StdoutSinkElement {
            conf,
            stdout: BufWriter::new(std::io::stdout()),
        })
    }

    fn next(&mut self, _pipeline: &mut Pipeline, receiver: &mut MsgReceiver) -> ElementResult {
        loop {
            let msg = receiver.recv(0)?;
            let bytes = msg.as_bytes();
            self.stdout.write_all(bytes)?;

            if let Some(separator) = self.conf.separator.as_ref() {
                self.stdout.write_all(separator.as_bytes())?;
            }
            self.stdout.flush()?;
        }
    }
}
