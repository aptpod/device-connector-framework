use crate::element::*;
use crate::error::Error;
use crate::MsgType;
use anyhow::bail;
use common::{MsgReceiver, Pipeline};
use serde_derive::Deserialize;
use std::io::{Read, Write};
use std::process::{Child, ChildStdout, Command, Stdio};

/// Captures process stdout.
pub struct ProcessSrcElement {
    child: Child,
    output: ChildStdout,
}

/// Configuration type for `ProcessSrcElement`
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProcessSrcElementConf {
    /// Executable path.
    #[serde(default)]
    pub program: Option<String>,
    /// Arguments.
    #[serde(default)]
    pub args: Vec<String>,
    /// Use if program is empty.
    #[serde(default)]
    pub command: String,
}

impl ElementBuildable for ProcessSrcElement {
    type Config = ProcessSrcElementConf;

    const NAME: &'static str = "process-src";

    const SEND_PORTS: Port = 1;

    fn new(conf: Self::Config) -> Result<Self, Error> {
        let mut command = if let Some(program) = conf.program {
            let mut command = Command::new(&program);
            if !conf.args.is_empty() {
                command.args(&conf.args);
            }
            command
        } else if !conf.command.is_empty() {
            let mut command = Command::new("/bin/sh");
            command.arg("-c");
            command.arg(&conf.command);
            command
        } else {
            bail!("no specified program or command for process-src element");
        };

        let mut child = command.stdout(Stdio::piped()).spawn()?;
        let output = child.stdout.take().unwrap();

        Ok(ProcessSrcElement { child, output })
    }

    fn next(&mut self, pipeline: &mut Pipeline, _receiver: &mut MsgReceiver) -> ElementResult {
        pipeline.check_send_msg_type(0, MsgType::binary)?;

        let mut buf = pipeline.msg_buf(0);
        let mut read_buf = [0; 0xFF];

        let n = loop {
            let n = self.output.read(&mut read_buf)?;

            if n > 0 {
                break n;
            } else if self.child.try_wait().is_ok() {
                return Ok(ElementValue::Close);
            }
        };
        buf.write_all(&read_buf[0..n])?;
        Ok(ElementValue::MsgBuf)
    }
}
