use crate::element::*;
use crate::error::Error;
use common::{MsgReceiver, MsgType, Pipeline};
use serde_derive::Deserialize;
use serde_with::{serde_as, DurationMilliSecondsWithFrac};
use std::io::Write;
use std::thread::sleep;
use std::time::Duration;

/// Generate text message.
pub struct TextSrcElement {
    conf: TextSrcElementConf,
    count: usize,
}

/// Configuration type for `TextSrcElement`
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextSrcElementConf {
    /// Text to send.
    pub text: String,
    /// Duration of sending message.
    #[serde_as(as = "DurationMilliSecondsWithFrac<f64>")]
    #[serde(alias = "duration_ms")]
    pub interval_ms: Duration,
    /// The number of message repeatation until next sleep.
    #[serde(default)]
    #[serde(alias = "repeat_until_sleep")]
    pub repeat: usize,
}

impl ElementBuildable for TextSrcElement {
    type Config = TextSrcElementConf;

    const NAME: &'static str = "text-src";

    const SEND_PORTS: Port = 1;

    fn new(mut conf: Self::Config) -> Result<Self, Error> {
        if conf.repeat == 0 {
            conf.repeat = 1;
        }
        Ok(TextSrcElement { conf, count: 0 })
    }

    fn next(&mut self, pipeline: &mut Pipeline, _receiver: &mut MsgReceiver) -> ElementResult {
        if !pipeline.send_msg_type_checked() {
            pipeline.check_send_msg_type(0, MsgType::binary)?;
        }

        let mut buf = pipeline.msg_buf(0);

        self.count += 1;
        if self.count == self.conf.repeat {
            sleep(self.conf.interval_ms);
            self.count = 0;
        }
        buf.write_all(self.conf.text.as_bytes())?;

        Ok(ElementValue::MsgBuf)
    }
}
