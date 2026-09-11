use anyhow::Context;
use dc_core::{
    ElementBuildable, ElementResult, ElementValue, Error, MsgReceiver, MsgType, Pipeline, Port,
};
use serde::Deserialize;
use serde_with::{DurationMilliSecondsWithFrac, serde_as};
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Read, Write};
use std::path::PathBuf;
use std::time::Duration;

fn retry_interval_default() -> Duration {
    Duration::from_secs(5)
}

/// Read from file.
pub struct FileSrcElement {
    conf: FileSrcElementConf,
    file: Option<File>,
}

/// Configuration type for `FileSrcElement`
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileSrcElementConf {
    /// File path.
    pub path: PathBuf,
    /// Add write flag when opening a file.
    #[serde(default)]
    pub write_flag: bool,
    /// Retry opening/reading file on failure.
    #[serde(default)]
    pub retry: bool,
    /// Interval in milliseconds before retrying.
    #[serde_as(as = "DurationMilliSecondsWithFrac<f64>")]
    #[serde(default = "retry_interval_default")]
    pub retry_interval_ms: Duration,
}

impl ElementBuildable for FileSrcElement {
    type Config = FileSrcElementConf;

    const NAME: &'static str = "file-src";
    const DESCRIPTION: &'static str = "Read binary data from given file path.";
    const CONFIG_DOC: &'static str = r#"
| Field | Type | Description |
| --- | --- | --- |
| path | string | Path to a file. |
| write_flag | bool | Add write flag when opening a file. |
| retry | bool | Retry opening/reading file on failure. |
| retry_interval_ms | number | Interval in milliseconds before retrying. |
"#;

    const SEND_PORTS: Port = 1;

    fn send_msg_types() -> Vec<Vec<MsgType>> {
        vec![vec![MsgType::binary()]]
    }

    fn new(conf: Self::Config) -> Result<Self, Error> {
        let file_res = if conf.write_flag {
            OpenOptions::new().read(true).write(true).open(&conf.path)
        } else {
            File::open(&conf.path)
        };

        let file = match file_res {
            Ok(f) => Some(f),
            Err(e) => {
                if conf.retry {
                    log::warn!(
                        "initial open failed for {}, will retry: {}",
                        conf.path.display(),
                        e
                    );
                    None
                } else {
                    return Err(e)
                        .with_context(|| format!("opening {} failed", conf.path.display()));
                }
            }
        };

        Ok(FileSrcElement { conf, file })
    }

    fn next(&mut self, pipeline: &mut Pipeline, _receiver: &mut MsgReceiver) -> ElementResult {
        let mut buf = pipeline.msg_buf(0);
        let mut read_buf = [0; 0xFF];

        let n = loop {
            if self.file.is_none() {
                let file_res = if self.conf.write_flag {
                    OpenOptions::new()
                        .read(true)
                        .write(true)
                        .open(&self.conf.path)
                } else {
                    File::open(&self.conf.path)
                };

                match file_res {
                    Ok(f) => {
                        self.file = Some(f);
                        log::info!("successfully opened file: {}", self.conf.path.display());
                    }
                    Err(e) => {
                        if self.conf.retry {
                            log::warn!(
                                "failed to open {}, retrying in {:?}: {}",
                                self.conf.path.display(),
                                self.conf.retry_interval_ms,
                                e
                            );
                            std::thread::sleep(self.conf.retry_interval_ms);
                            continue;
                        } else {
                            return Err(e).with_context(|| {
                                format!("opening {} failed", self.conf.path.display())
                            });
                        }
                    }
                }
            }

            let file = self.file.as_mut().unwrap();
            match file.read(&mut read_buf) {
                Ok(n) => {
                    if n > 0 {
                        break n;
                    } else if self.conf.retry {
                        log::warn!(
                            "eof detected in {}, reopening in {:?}",
                            self.conf.path.display(),
                            self.conf.retry_interval_ms,
                        );
                        self.file = None;
                        std::thread::sleep(self.conf.retry_interval_ms);
                        continue;
                    } else {
                        return Ok(ElementValue::Close);
                    }
                }
                Err(e) => {
                    if self.conf.retry {
                        log::warn!(
                            "failed to read {}, reopening in {:?}: {}",
                            self.conf.path.display(),
                            self.conf.retry_interval_ms,
                            e
                        );
                        self.file = None;
                        std::thread::sleep(self.conf.retry_interval_ms);
                        continue;
                    } else {
                        return Err(e.into());
                    }
                }
            }
        };

        buf.write_all(&read_buf[0..n])?;
        Ok(ElementValue::MsgBuf)
    }
}

/// Emits received message to a file.
pub struct FileSinkElement {
    conf: FileSinkElementConf,
}

/// Configuration type for `FileSinkElement`
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileSinkElementConf {
    /// File path.
    pub path: PathBuf,
    /// Create new file or not.
    #[serde(default)]
    pub create: bool,
    /// Buffer flush size.
    #[serde(default)]
    pub flush_size: usize,
    /// Separator text.
    pub separator: Option<String>,
    /// Retry opening/writing file on failure.
    #[serde(default)]
    pub retry: bool,
    /// Interval in milliseconds before retrying.
    #[serde_as(as = "DurationMilliSecondsWithFrac<f64>")]
    #[serde(default = "retry_interval_default")]
    pub retry_interval_ms: Duration,
}

impl ElementBuildable for FileSinkElement {
    type Config = FileSinkElementConf;

    const NAME: &'static str = "file-sink";
    const DESCRIPTION: &'static str = "Write data to a specified file.";
    const CONFIG_DOC: &'static str = r#"
| Field | Type | Description |
| --- | --- | --- |
| path | string | Path to a file. |
| create | bool | Create new file or not. |
| flush_size | integer | Buffer flush size. |
| separator | string | Optional string to separate received messages. |
| retry | bool | Retry opening/writing file on failure. |
| retry_interval_ms | number | Interval in milliseconds before retrying. |
"#;

    const RECV_PORTS: Port = 1;

    fn recv_msg_types() -> Vec<Vec<MsgType>> {
        vec![vec![MsgType::any()]]
    }

    fn new(conf: Self::Config) -> Result<Self, Error> {
        Ok(FileSinkElement { conf })
    }

    fn next(&mut self, _pipeline: &mut Pipeline, receiver: &mut MsgReceiver) -> ElementResult {
        let mut file_opt: Option<BufWriter<File>> = None;

        loop {
            let msg = receiver.recv(0)?;
            let bytes = msg.as_bytes();

            loop {
                if file_opt.is_none() {
                    let file_res = OpenOptions::new()
                        .read(false)
                        .write(true)
                        .create(self.conf.create)
                        .open(&self.conf.path);

                    match file_res {
                        Ok(f) => {
                            file_opt = Some(BufWriter::new(f));
                            log::info!("successfully opened file: {}", self.conf.path.display());
                        }
                        Err(e) => {
                            if self.conf.retry {
                                log::warn!(
                                    "failed to open {}, retrying in {:?}: {}",
                                    self.conf.path.display(),
                                    self.conf.retry_interval_ms,
                                    e
                                );
                                std::thread::sleep(self.conf.retry_interval_ms);
                                continue;
                            } else {
                                return Err(e).with_context(|| {
                                    format!("opening {} failed", self.conf.path.display())
                                });
                            }
                        }
                    }
                }

                let file = file_opt.as_mut().unwrap();
                let write_res = (|| -> std::io::Result<()> {
                    file.write_all(bytes)?;

                    if let Some(separator) = self.conf.separator.as_ref() {
                        file.write_all(separator.as_bytes())?;
                    }

                    if self.conf.flush_size == 0 || file.buffer().len() > self.conf.flush_size {
                        file.flush()?;
                    }
                    Ok(())
                })();

                match write_res {
                    Ok(_) => break,
                    Err(e) => {
                        if self.conf.retry {
                            log::warn!(
                                "failed to write to {}, reopening in {:?}: {}",
                                self.conf.path.display(),
                                self.conf.retry_interval_ms,
                                e
                            );
                            file_opt = None;
                            std::thread::sleep(self.conf.retry_interval_ms);
                            continue;
                        } else {
                            return Err(e.into());
                        }
                    }
                }
            }
        }
    }
}
