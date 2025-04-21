use anyhow::{anyhow, bail, Context, Result};
use dc_core::{
    ElementBuildable, ElementResult, ElementValue, Error, MsgReceiver, MsgType, Pipeline, Port,
};
use serde::Deserialize;
use std::io::Write;
use std::path::PathBuf;

use crate::{
    pytypes::{PollValue, PyMsg},
    simple_executor::{ExecutorKind, SimpleExecutor},
};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PythonElementConf {
    pub function_name: String,
    pub script: Option<String>,
    pub script_path: Option<PathBuf>,
    #[serde(default)]
    pub import_script_dir: bool,
}

pub struct PythonSrcElement {
    executor: SimpleExecutor,
}

impl ElementBuildable for PythonSrcElement {
    type Config = PythonElementConf;

    const NAME: &'static str = "python-src";
    const DESCRIPTION: &'static str = "Call python function as a device connector src.";
    const CONFIG_DOC: &'static str = r#"
| Field | Type | Description |
| --- | --- | --- |
| function_name | string | Python function name to call |
| script | string | Script. |
| script_path | string | File path to a python script. |
| import_script_dir | bool | Append the directory of `script_path` to sys.path |
"#;

    const SEND_PORTS: Port = 1;

    fn send_msg_types() -> Vec<Vec<MsgType>> {
        vec![vec![MsgType::any()]]
    }

    fn new(conf: Self::Config) -> Result<Self, Error> {
        let import_dir = if conf.import_script_dir {
            conf.script_path
                .as_ref()
                .and_then(|path| path.parent().map(|path| path.to_owned()))
        } else {
            None
        };
        let script = load_script(conf.script, conf.script_path)?;
        let executor =
            SimpleExecutor::start(script, conf.function_name, import_dir, ExecutorKind::Src);
        Ok(Self { executor })
    }

    fn next(&mut self, pipeline: &mut Pipeline, _receiver: &mut MsgReceiver) -> ElementResult {
        while let Some(poll_value) = self.executor.poll() {
            match poll_value {
                PollValue::Msg(result_msg) => {
                    let mut buf = pipeline.msg_buf(0);
                    buf.write(&result_msg.bytes)?;
                    return Ok(ElementValue::MsgBuf);
                }
                PollValue::Close => {
                    break;
                }
                _ => unreachable!(),
            }
        }

        Ok(ElementValue::Close)
    }
}

pub struct PythonFilterElement {
    executor: SimpleExecutor,
}

impl ElementBuildable for PythonFilterElement {
    type Config = PythonElementConf;

    const NAME: &'static str = "python-filter";
    const DESCRIPTION: &'static str = "Call python function as a device connector filter.";
    const CONFIG_DOC: &'static str = r#"
| Field | Type | Description |
| --- | --- | --- |
| function_name | string | Python function name to call |
| script | string | Script. |
| script_path | string | File path to a python script. |
| import_script_dir | bool | Append the directory of `script_path` to sys.path |
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
        let import_dir = if conf.import_script_dir {
            conf.script_path
                .as_ref()
                .and_then(|path| path.parent().map(|path| path.to_owned()))
        } else {
            None
        };
        let script = load_script(conf.script, conf.script_path)?;
        let executor =
            SimpleExecutor::start(script, conf.function_name, import_dir, ExecutorKind::Filter);
        Ok(Self { executor })
    }

    fn next(&mut self, pipeline: &mut Pipeline, receiver: &mut MsgReceiver) -> ElementResult {
        while let Some(poll_value) = self.executor.poll() {
            match poll_value {
                PollValue::Receiving => {
                    let msg = receiver.recv(0)?;
                    let pymsg = PyMsg::new(msg.as_bytes().to_vec());
                    self.executor.send(pymsg);
                }
                PollValue::Msg(result_msg) => {
                    let mut buf = pipeline.msg_buf(0);
                    buf.write(&result_msg.bytes)?;
                    return Ok(ElementValue::MsgBuf);
                }
                PollValue::Close => {
                    break;
                }
            }
        }

        Ok(ElementValue::Close)
    }
}

pub struct PythonSinkElement {
    executor: SimpleExecutor,
}

impl ElementBuildable for PythonSinkElement {
    type Config = PythonElementConf;

    const NAME: &'static str = "python-sink";
    const DESCRIPTION: &'static str = "Call python function as a device connector sink.";
    const CONFIG_DOC: &'static str = r#"
| Field | Type | Description |
| --- | --- | --- |
| function_name | string | Python function name to call |
| script | string | Script. |
| script_path | string | File path to a python script. |
| import_script_dir | bool | Append the directory of `script_path` to sys.path |
"#;

    const RECV_PORTS: Port = 1;

    fn recv_msg_types() -> Vec<Vec<MsgType>> {
        vec![vec![MsgType::any()]]
    }

    fn new(conf: Self::Config) -> Result<Self, Error> {
        let import_dir = if conf.import_script_dir {
            conf.script_path
                .as_ref()
                .and_then(|path| path.parent().map(|path| path.to_owned()))
        } else {
            None
        };
        let script = load_script(conf.script, conf.script_path)?;
        let executor =
            SimpleExecutor::start(script, conf.function_name, import_dir, ExecutorKind::Sink);
        Ok(Self { executor })
    }

    fn next(&mut self, _pipeline: &mut Pipeline, receiver: &mut MsgReceiver) -> ElementResult {
        while let Some(poll_value) = self.executor.poll() {
            match poll_value {
                PollValue::Receiving => {
                    let msg = receiver.recv(0)?;
                    let pymsg = PyMsg::new(msg.as_bytes().to_vec());
                    self.executor.send(pymsg);
                }
                PollValue::Close => {
                    break;
                }
                _ => unreachable!(),
            }
        }

        Ok(ElementValue::Close)
    }
}

fn load_script(script: Option<String>, script_path: Option<PathBuf>) -> Result<(String, String)> {
    match (script, script_path) {
        (Some(script), None) => Ok((script, "".into())),
        (None, Some(path)) => {
            let s = std::fs::read_to_string(&path)
                .with_context(|| format!("reading {}", path.display()))?;
            let filename = path
                .file_name()
                .ok_or_else(|| anyhow!("cannot get python script file name"))?
                .to_string_lossy()
                .into_owned();
            Ok((s, filename))
        }
        _ => {
            bail!("either `script` or `script_path` must be specified");
        }
    }
}
