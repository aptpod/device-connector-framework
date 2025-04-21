use dc_core::{
    ElementBuildable, ElementResult, ElementValue, Error, MsgReceiver, MsgType, Pipeline, Port,
};
use serde::Deserialize;
use serde_with::{serde_as, DurationMilliSecondsWithFrac};
use std::time::{Duration, Instant};

/// Count passed message size and print statistics.
pub struct StatFilterElement {
    conf: StatFilterElementConf,
    count: usize,
    bytes: usize,
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
    const DESCRIPTION: &'static str = "Print the statistics of passed messages";
    const CONFIG_DOC: &'static str = r#"
| Field | Type | Description |
| --- | --- | --- |
| interval_ms | real | Print interval in milli seconds |
"#;

    const RECV_PORTS: Port = 1;
    const SEND_PORTS: Port = 1;

    fn recv_msg_types() -> Vec<Vec<MsgType>> {
        vec![vec![MsgType::any()]]
    }

    fn send_msg_types() -> Vec<Vec<MsgType>> {
        vec![vec![MsgType::any()]]
    }

    fn new(conf: Self::Config) -> Result<Self, Error> {
        Ok(StatFilterElement {
            conf,
            count: 0,
            bytes: 0,
            before: Instant::now(),
        })
    }

    fn next(&mut self, _pipeline: &mut Pipeline, receiver: &mut MsgReceiver) -> ElementResult {
        let msg = receiver.recv(0)?;
        let msg_size = msg.as_bytes().len();
        self.count += 1;
        self.bytes += msg_size;

        let now = Instant::now();
        let since = now.duration_since(self.before);

        if since > self.conf.interval_ms {
            eprintln!("count = {}, bytes = {}", self.count, self.bytes);
            self.count = 0;
            self.bytes = 0;
            self.before = now;
        }

        Ok(ElementValue::Msg(0, msg))
    }
}
