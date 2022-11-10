use crate::element::*;
use crate::error::Error;
use common::{MsgReceiver, MsgType, Pipeline};

/// Null message sink.
pub struct NullSinkElement;

impl ElementBuildable for NullSinkElement {
    type Config = crate::base::EmptyElementConf;

    const NAME: &'static str = "null-sink";

    const RECV_PORTS: Port = 32;

    fn acceptable_msg_types() -> Vec<Vec<MsgType>> {
        vec![vec![MsgType::any(); Self::RECV_PORTS as usize]]
    }

    fn new(_conf: Self::Config) -> Result<Self, Error> {
        Ok(NullSinkElement)
    }

    fn next(&mut self, _pipeline: &mut Pipeline, receiver: &mut MsgReceiver) -> ElementResult {
        loop {
            let _msg = receiver.recv_any_port()?;
        }
    }
}
