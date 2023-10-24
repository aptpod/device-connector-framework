use crate::element::*;
use crate::error::Error;
use crate::MsgType;
use anyhow::bail;
use common::{MsgReceiver, Pipeline};
use serde::Deserialize;
use serde_with::{serde_as, DurationMilliSecondsWithFrac};
use std::io::{Read, Write};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};

/// Captures process stdout.
pub struct ProcessSrcElement {
    child: Option<(Child, ChildStdout)>,
    read_buf: Vec<u8>,
    conf: ProcessSrcElementConf,
}

/// Configuration type for `ProcessSrcElement`
#[serde_as]
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
    /// Buffer size.
    #[serde(default)]
    pub buffer_size: Option<usize>,
    /// Retry
    #[serde(default)]
    pub retry: bool,
    /// Retry interval
    #[serde_as(as = "DurationMilliSecondsWithFrac<f64>")]
    #[serde(default = "default_retry_interval_ms")]
    pub retry_interval_ms: Duration,
}

fn default_retry_interval_ms() -> Duration {
    Duration::from_secs(1)
}

impl ElementBuildable for ProcessSrcElement {
    type Config = ProcessSrcElementConf;

    const NAME: &'static str = "process-src";

    const SEND_PORTS: Port = 1;

    fn new(conf: Self::Config) -> Result<Self, Error> {
        if conf.program.is_none() && conf.command.is_empty() {
            bail!("no specified program or command for process-src element");
        }
        let child = if conf.retry {
            None
        } else {
            let mut command = to_command(&conf.program, &conf.args, &conf.command)?;
            let mut child = command.stdout(Stdio::piped()).spawn()?;
            let output = child.stdout.take().unwrap();
            Some((child, output))
        };
        let buffer_size = conf.buffer_size.unwrap_or(0xFF);
        let read_buf = vec![0; buffer_size];

        Ok(ProcessSrcElement {
            child,
            read_buf,
            conf,
        })
    }

    fn next(&mut self, pipeline: &mut Pipeline, _receiver: &mut MsgReceiver) -> ElementResult {
        pipeline.check_send_msg_type(0, MsgType::binary)?;

        let mut buf = pipeline.msg_buf(0);

        'read_loop: loop {
            let (child, output) = loop {
                if let Some((child, output)) = &mut self.child {
                    break (child, output);
                } else if self.conf.retry {
                    let mut command =
                        to_command(&self.conf.program, &self.conf.args, &self.conf.command)?;
                    let mut child = match command.stdout(Stdio::piped()).spawn() {
                        Ok(child) => child,
                        Err(e) => {
                            log::warn!("cannot spawn child process: {:?}", e);
                            std::thread::sleep(self.conf.retry_interval_ms);
                            continue;
                        }
                    };
                    let output = child.stdout.take().unwrap();
                    self.child = Some((child, output));
                } else {
                    return Ok(ElementValue::Close);
                }
            };

            let n = loop {
                let n = match output.read(&mut self.read_buf) {
                    Ok(n) => n,
                    Err(e) => {
                        if self.conf.retry {
                            log::warn!("read error from process: {:?}", e);
                            self.child = None;
                            std::thread::sleep(self.conf.retry_interval_ms);
                            continue 'read_loop;
                        } else {
                            return Err(e.into());
                        }
                    }
                };

                if n > 0 {
                    break n;
                } else if child.try_wait().is_ok() {
                    if self.conf.retry {
                        self.child = None;
                        std::thread::sleep(self.conf.retry_interval_ms);
                        continue 'read_loop;
                    } else {
                        return Ok(ElementValue::Close);
                    }
                }
            };
            buf.write_all(&self.read_buf[0..n])?;
            return Ok(ElementValue::MsgBuf);
        }
    }
}

/// Captures process stdout.
pub struct RepeatProcessSrcElement {
    conf: RepeatProcessSrcElementConf,
    before: Option<Instant>,
}

/// Configuration type for `ProcessSrcElement`
#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepeatProcessSrcElementConf {
    /// Executable path.
    #[serde(default)]
    pub program: Option<String>,
    /// Arguments.
    #[serde(default)]
    pub args: Vec<String>,
    /// Use if program is empty.
    #[serde(default)]
    pub command: String,
    #[serde_as(as = "DurationMilliSecondsWithFrac<f64>")]
    pub interval_ms: Duration,
}

impl ElementBuildable for RepeatProcessSrcElement {
    type Config = RepeatProcessSrcElementConf;

    const NAME: &'static str = "repeat-process-src";

    const SEND_PORTS: Port = 1;

    fn new(conf: Self::Config) -> Result<Self, Error> {
        Ok(Self { conf, before: None })
    }

    fn next(&mut self, pipeline: &mut Pipeline, _receiver: &mut MsgReceiver) -> ElementResult {
        pipeline.check_send_msg_type(0, MsgType::binary)?;

        if let Some(before) = self.before {
            let now = Instant::now();
            if before + self.conf.interval_ms > now {
                std::thread::sleep(before + self.conf.interval_ms - now);
            }
        }
        self.before = Some(Instant::now());

        let mut command = to_command(&self.conf.program, &self.conf.args, &self.conf.command)?;

        let output = command.stdout(Stdio::piped()).output()?;

        let mut buf = pipeline.msg_buf(0);
        buf.write_all(&output.stdout)?;

        Ok(ElementValue::MsgBuf)
    }
}

fn to_command(program: &Option<String>, args: &[String], command: &str) -> Result<Command, Error> {
    if let Some(program) = program {
        let mut cmd = Command::new(program);
        if args.is_empty() {
            cmd.args(args);
        }
        Ok(cmd)
    } else if !command.is_empty() {
        let mut cmd = Command::new("/bin/sh");
        cmd.arg("-c");
        cmd.arg(command);
        Ok(cmd)
    } else {
        bail!("no specified program or command for process-src element")
    }
}
