use anyhow::bail;
use dc_core::{
    ElementBuildable, ElementResult, ElementValue, Error, MsgReceiver, MsgType, Pipeline, Port,
};
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
    command_string: String,
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
    const DESCRIPTION: &'static str = "Read stdout from process.";
    const CONFIG_DOC: &'static str = r#"
| Field | Type | Description |
| --- | --- | --- |
| program | string | Executable file path. |
| args | [string] | Arguments. |
| command | string | Command line to execute. Used when program is empty. |
| buffer_size | integer | Read buffer size. The defalut value is 255. |
| retry | boolean | Retry the process if it is failed. The defalut value is false. |
| retry_interval_ms | real | Retry interval in milli seconds. The defalut value is 1000. |
"#;

    const SEND_PORTS: Port = 1;

    fn send_msg_types() -> Vec<Vec<MsgType>> {
        vec![vec![MsgType::binary()]]
    }

    fn new(conf: Self::Config) -> Result<Self, Error> {
        if conf.program.is_none() && conf.command.is_empty() {
            bail!("no specified program or command for process-src element");
        }
        let mut command = to_command(&conf.program, &conf.args, &conf.command)?;
        let command_string = format!("{:?}", command);

        let child = if conf.retry {
            None
        } else {
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
            command_string,
        })
    }

    fn next(&mut self, pipeline: &mut Pipeline, _receiver: &mut MsgReceiver) -> ElementResult {
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
                } else {
                    if let Ok(exit_status) = child.try_wait() {
                        if let Some(exit_status) = exit_status {
                            let log_level = if self.conf.retry {
                                log::Level::Warn
                            } else {
                                log::Level::Error
                            };
                            if !exit_status.success() {
                                if let Some(code) = exit_status.code() {
                                    log::log!(
                                        log_level,
                                        "process `{}` exit with code = {}",
                                        self.command_string,
                                        code
                                    );
                                } else {
                                    log::log!(
                                        log_level,
                                        "process `{}` exit with failure",
                                        self.command_string
                                    );
                                }
                            }
                        }

                        if self.conf.retry {
                            self.child = None;
                            std::thread::sleep(self.conf.retry_interval_ms);
                            continue 'read_loop;
                        } else {
                            return Ok(ElementValue::Close);
                        }
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
    const DESCRIPTION: &'static str =
        "Execute an process, and send the whole stdout data as one message.";
    const CONFIG_DOC: &'static str = r#"
| Field | Type | Description |
| --- | --- | --- |
| program | string | Executable file path. |
| args | [string] | Arguments. |
| command | string | Command line to execute. Used when program is empty. |
| interval_ms | real | Repeat interval in milli seconds |
"#;

    const SEND_PORTS: Port = 1;

    fn send_msg_types() -> Vec<Vec<MsgType>> {
        vec![vec![MsgType::any()]]
    }

    fn new(conf: Self::Config) -> Result<Self, Error> {
        Ok(Self { conf, before: None })
    }

    fn next(&mut self, pipeline: &mut Pipeline, _receiver: &mut MsgReceiver) -> ElementResult {
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
        if !args.is_empty() {
            cmd.args(args);
        }
        Ok(cmd)
    } else if !command.is_empty() {
        string_to_command(command)
    } else {
        bail!("no specified program or command for process-src element")
    }
}

#[cfg(unix)]
fn string_to_command(s: &str) -> Result<Command, Error> {
    let mut cmd = Command::new("/bin/sh");
    cmd.arg("-c");
    cmd.arg(s);
    Ok(cmd)
}

#[cfg(windows)]
fn string_to_command(s: &str) -> Result<Command, Error> {
    let mut cmd = Command::new("cmd");
    cmd.arg("/C");
    cmd.arg(s);
    Ok(cmd)
}

#[cfg(not(any(unix, windows)))]
fn string_to_command(s: &str) -> Result<Command, Error> {
    bail!("Process command not supported")
}
