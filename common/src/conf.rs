use anyhow::{anyhow, Error, Result};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

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
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunnerConf {
    #[serde(default = "channel_capacity_default")]
    pub channel_capacity: usize,
    #[serde(with = "serde_with_std_duration")]
    #[serde(default = "termination_timeout_default")]
    pub termination_timeout: Duration,
    #[serde(with = "serde_with_std_duration")]
    #[serde(default = "termination_timeout_default")]
    pub finalizer_timeout: Duration,
}

impl Default for RunnerConf {
    fn default() -> Self {
        RunnerConf {
            channel_capacity: channel_capacity_default(),
            termination_timeout: termination_timeout_default(),
            finalizer_timeout: termination_timeout_default(),
        }
    }
}

fn channel_capacity_default() -> usize {
    16
}

fn termination_timeout_default() -> Duration {
    Duration::from_secs(10)
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
    #[serde(default)]
    pub conf: ElementConf,
}

/// General element configuration type
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ElementConf(serde_json::Value);

impl Default for ElementConf {
    fn default() -> Self {
        Self(serde_json::Value::Object(serde_json::Map::new()))
    }
}

impl ElementConf {
    #[doc(hidden)]
    pub fn remove_null_from_map(&mut self) {
        remove_null_from_map(&mut self.0)
    }
}

fn remove_null_from_map(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Array(array) => {
            for v in array {
                remove_null_from_map(v);
            }
        }
        serde_json::Value::Object(map) => {
            map.retain(|_, value| !value.is_null());
            for (_, v) in map {
                remove_null_from_map(v);
            }
        }
        _ => (),
    }
}

/// Unique id for task.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TaskId(pub String);

/// Port number of elements.
pub type Port = u8;

/// A port of a task.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct TaskPort(pub TaskId, pub Port);

impl std::str::FromStr for TaskPort {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use once_cell::sync::Lazy;
        use regex::Regex;

        static TASK_ID_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new("[a-zA-Z0-9_-]+").unwrap());
        static TASK_PORT_REGEX: Lazy<Regex> =
            Lazy::new(|| Regex::new("([a-zA-Z0-9_-]+):([0-9]+)").unwrap());

        if let Some(cap) = TASK_PORT_REGEX.captures(s) {
            let task_id = cap.get(1).unwrap().as_str();
            let port: u8 = cap.get(2).unwrap().as_str().parse().unwrap();
            return Ok(TaskPort(TaskId(task_id.into()), port));
        }

        if TASK_ID_REGEX.is_match(s) {
            return Ok(TaskPort(TaskId(s.into()), Port::default()));
        }

        Err(anyhow!("Invalid string for task port \"{}\"", s))
    }
}

impl std::fmt::Display for TaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::fmt::Display for TaskPort {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}:{}", self.0, self.1)
    }
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

/// Serialize/deserialize `Duration` as string with units.
pub mod serde_with_std_duration {
    use serde::Deserialize;
    use std::num::ParseIntError;
    use std::time::Duration;

    pub fn serialize<S: serde::Serializer>(t: &Duration, s: S) -> Result<S::Ok, S::Error> {
        let millis = t.as_millis();

        if millis % 1000 == 0 {
            let secs = millis / 1000;
            s.serialize_str(&format!("{}s", secs))
        } else {
            s.serialize_str(&format!("{}ms", millis))
        }
    }

    pub fn deserialize<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Duration, D::Error> {
        let s = String::deserialize(d).map_err(serde::de::Error::custom)?;
        parse(&s).map_err(serde::de::Error::custom)
    }

    fn parse(s: &str) -> Result<Duration, String> {
        if let Some(millis) = s.strip_suffix("ms") {
            let millis: u64 = millis.parse().map_err(|e: ParseIntError| e.to_string())?;
            Ok(Duration::from_millis(millis))
        } else if let Some(secs) = s.strip_suffix('s') {
            let secs: u64 = secs.parse().map_err(|e: ParseIntError| e.to_string())?;
            Ok(Duration::from_secs(secs))
        } else {
            Err(format!("invalid duration config \"{}\"", s))
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn parse_test() {
            assert_eq!(parse("10s").unwrap(), Duration::from_secs(10));
            assert_eq!(parse("100ms").unwrap(), Duration::from_millis(100));
            assert_eq!(parse("1500ms").unwrap(), Duration::from_millis(1500));
            assert!(parse("1500a").is_err());
        }
    }
}
