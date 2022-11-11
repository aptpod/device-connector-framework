use crate::element::*;
use crate::error::Error;
use common::{MsgReceiver, MsgType, Pipeline};
use serde_derive::Deserialize;
use serde_with::{serde_as, DurationMilliSecondsWithFrac};
use std::io::Write;
use std::time::{Duration, Instant};

/// Count passed message size and print statistics.
pub struct PrintLogFilterElement {
    conf: PrintLogFilterElementConf,
    count: usize,
    bytes: usize,
    before: Instant,
}

/// Configuration type for `PrintLogFilterElement`
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrintLogFilterElementConf {
    /// Print duration.
    #[serde_as(as = "DurationMilliSecondsWithFrac<f64>")]
    #[serde(alias = "duration_ms")]
    pub interval_ms: Duration,
    pub tag: String,
    pub output: PrintLogFilterElementConfOutput,
}

/// Output for `PrintLogFilterElemen`
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PrintLogFilterElementConfOutput {
    LogTrace,
    Stderr,
}

impl ElementBuildable for PrintLogFilterElement {
    type Config = PrintLogFilterElementConf;

    const NAME: &'static str = "print-log-filter";
    const RECV_PORTS: Port = 1;
    const SEND_PORTS: Port = 1;

    fn acceptable_msg_types() -> Vec<Vec<MsgType>> {
        vec![vec![MsgType::any()]]
    }

    fn new(conf: Self::Config) -> Result<Self, Error> {
        Ok(PrintLogFilterElement {
            conf,
            count: 0,
            bytes: 0,
            before: Instant::now(),
        })
    }

    fn next(&mut self, pipeline: &mut Pipeline, receiver: &mut MsgReceiver) -> ElementResult {
        let msg = receiver.recv(0)?;
        let msg_size = msg.as_bytes().len();
        self.count += 1;
        self.bytes += msg_size;

        let now = Instant::now();
        let since = now.duration_since(self.before);

        if since > self.conf.interval_ms {
            let s = format!(
                "[{}] {} msgs, {} bytes",
                self.conf.tag, self.count, self.bytes
            );
            match self.conf.output {
                PrintLogFilterElementConfOutput::LogTrace => {
                    log::trace!("{}", s);
                }
                PrintLogFilterElementConfOutput::Stderr => {
                    eprintln!("{}", s);
                }
            }
            self.before = now;
            self.count = 0;
            self.bytes = 0;
        }

        let mut buf = pipeline.msg_buf(0);
        buf.write_all(msg.as_bytes())?;
        Ok(ElementValue::MsgBuf)
    }
}
