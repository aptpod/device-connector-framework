use crate::element::*;
use crate::error::Error;
use common::{MsgReceiver, MsgType, Pipeline};
use serde::Deserialize;
use serde_with::{serde_as, DurationMilliSecondsWithFrac};
use std::io::Write;
use std::time::{Duration, Instant};

/// Count passed message size and print statistics.
pub struct StatFilterElement {
    conf: StatFilterElementConf,
    count: usize,
    total_msg_size: usize,
    before: Instant,
}

/// Configuration type for `StatFilterElement`
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatFilterElementConf {
    /// Print duration.
    #[serde_as(as = "DurationMilliSecondsWithFrac<f64>")]
    #[serde(alias = "duration_ms")]
    pub interval_ms: Duration,
}

impl ElementBuildable for StatFilterElement {
    type Config = StatFilterElementConf;

    const NAME: &'static str = "stat-filter";
    const RECV_PORTS: Port = 1;
    const SEND_PORTS: Port = 1;

    fn acceptable_msg_types() -> Vec<Vec<MsgType>> {
        vec![vec![MsgType::any()]]
    }

    fn new(conf: Self::Config) -> Result<Self, Error> {
        Ok(StatFilterElement {
            conf,
            count: 0,
            total_msg_size: 0,
            before: Instant::now(),
        })
    }

    fn next(&mut self, pipeline: &mut Pipeline, receiver: &mut MsgReceiver) -> ElementResult {
        let msg = receiver.recv(0)?;
        let msg_size = msg.as_bytes().len();
        self.count += 1;
        self.total_msg_size += msg_size;

        let now = Instant::now();
        let since = now.duration_since(self.before);

        if since > self.conf.interval_ms {
            eprintln!(
                "count = {}, total_msg_size = {}",
                self.count, self.total_msg_size
            );
            self.before = now;
        }

        let mut buf = pipeline.msg_buf(0);
        buf.write_all(msg.as_bytes())?;
        Ok(ElementValue::MsgBuf)
    }
}
