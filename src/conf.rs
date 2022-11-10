use crate::task::{TaskId, TaskPort};
use anyhow::Result;
use serde_derive::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use std::path::{Path, PathBuf};

use common::ElementConf;

/// Device connector configuration
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Conf {
    #[serde(default)]
    pub runner: RunnerConf,
    #[serde(default)]
    pub plugin: PluginConf,
    #[serde(alias = "task")]
    pub tasks: Vec<TaskConf>,
    #[serde(default)]
    pub bg_processes: Vec<BgProcessConf>,
    #[serde(default, alias = "before_script")]
    pub before_task: Vec<String>,
    #[serde(default, alias = "after_script")]
    pub after_task: Vec<String>,
}

impl Conf {
    pub fn read_from_file(path: &Path) -> Result<Self> {
        let s = std::fs::read_to_string(path)?;
        Ok(serde_yaml::from_str(&s)?)
    }

    pub fn from_yaml(s: &str) -> Result<Self> {
        Ok(serde_yaml::from_str(s)?)
    }
}

/// Runner configuration
#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunnerConf {
    pub channel_capacity: Option<usize>,
}

/// Plugin configuration
#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginConf {
    pub plugin_files: Vec<PathBuf>,
}

/// Configuration of tasks to execute
#[serde_as]
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskConf {
    pub id: TaskId,
    pub element: String,
    #[serde(default)]
    #[serde_as(as = "Vec<Vec<DisplayFromStr>>")]
    pub from: Vec<Vec<TaskPort>>,
    pub conf_file: Option<PathBuf>,
    pub conf: Option<ElementConf>,
}

/// Background process configuration
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[serde(deny_unknown_fields)]
pub struct BgProcessConf {
    pub command: String,
    pub wait_signal: Option<BgProcessWaitSignal>,
}

/// Signal type
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BgProcessWaitSignal {
    Sigusr1,
    Sigusr2,
}
