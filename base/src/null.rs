use dc_core::{
    ElementBuildable, ElementResult, EmptyElementConf, Error, MsgReceiver, MsgType, Pipeline, Port,
};

/// Null message sink.
pub struct NullSinkElement;

impl ElementBuildable for NullSinkElement {
    type Config = EmptyElementConf;

    const NAME: &'static str = "null-sink";
    const DESCRIPTION: &'static str = "Null message sink.";

    const RECV_PORTS: Port = 1;

    fn recv_msg_types() -> Vec<Vec<MsgType>> {
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
