use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::time::{Duration, Instant};

use dc_core::{
    ElementBuildable, ElementResult, ElementValue, Error, MsgReceiver, MsgType, Pipeline, Port,
};
use serde::Deserialize;
use serde_with::{DurationMilliSecondsWithFrac, serde_as};

/// Count passed message size and print statistics.
pub struct IdleDetectFilterElement {
    counter: Arc<AtomicUsize>,
}

/// Configuration type for `StatFilterElement`
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdleDetectFilterElementConf {
    /// Timeout duration.
    #[serde_as(as = "DurationMilliSecondsWithFrac<f64>")]
    #[serde(alias = "duration_ms")]
    pub timeout_ms: Duration,
    #[serde(default)]
    pub termination: bool,
}

impl ElementBuildable for IdleDetectFilterElement {
    type Config = IdleDetectFilterElementConf;

    const NAME: &'static str = "idle-detect-filter";
    const DESCRIPTION: &'static str = "Print the statistics of passed messages";
    const CONFIG_DOC: &'static str = r#"
| Field | Type | Description |
| --- | --- | --- |
| timeout_ms | real | Idle timeout in milli seconds |
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
        let counter = Arc::new(AtomicUsize::new(0));

        let c = counter.clone();
        std::thread::spawn(move || {
            detect_task(conf, c);
        });

        Ok(IdleDetectFilterElement { counter })
    }

    fn next(&mut self, _pipeline: &mut Pipeline, receiver: &mut MsgReceiver) -> ElementResult {
        let msg = receiver.recv(0)?;

        self.counter
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        Ok(ElementValue::Msg(0, msg))
    }
}

fn detect_task(conf: IdleDetectFilterElementConf, counter: Arc<AtomicUsize>) {
    let mut prev = 0;
    let mut updated = Instant::now();

    loop {
        std::thread::sleep(conf.timeout_ms / 8);

        let c = counter.load(std::sync::atomic::Ordering::Relaxed);
        let now = Instant::now();

        if c != prev {
            prev = c;
            updated = now;
            continue;
        }

        if now.duration_since(updated) < conf.timeout_ms {
            continue;
        }

        log::info!("idle detected");

        if conf.termination {
            let pid = rustix::process::getpid();
            if let Err(e) = rustix::process::kill_process(pid, rustix::process::Signal::TERM) {
                eprintln!("cannot send sigterm: {}", e);
                std::process::abort();
            }
            break;
        }

        updated = Instant::now();
    }
}
