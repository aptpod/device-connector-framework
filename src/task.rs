use crate::channel::{Channel, MsgReceiverInner};
use crate::element::*;
use crate::error::{Error, ReceiveError};
use crate::finalizer::FinalizerHolder;
use crate::msg_buf::MsgBufInner;
use crate::pipeline::PipelineInner;
use common::{DcMsgReceiver, DcPipeline, Msg, SendableMsg};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{spawn, JoinHandle};

const CHANNEL_NONE_ERR_MSG: &str = "task channel is not set";

pub(crate) static CLOSING: AtomicBool = AtomicBool::new(false);

/// Get tasks are closing or not
pub fn task_closing() -> bool {
    CLOSING.load(Ordering::Relaxed)
}

/// Unique id for task.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TaskId(pub u64);

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct TaskPort(pub TaskId, pub Port);

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

/// Runnable task that includes element and task id.
pub struct Task {
    id: TaskId,
    element: ElementPreBuild,
    channel: Option<Channel>,
    pipeline: Option<PipelineInner>,
}

pub(crate) struct ChildTask {
    id: TaskId,
    next_boxed: ElementNextBoxed,
    pipeline: DcPipeline,
    msg_receiver: DcMsgReceiverWrapped,
}

/// Wrapped DcMsgReceiver for drop.
struct DcMsgReceiverWrapped(DcMsgReceiver);

impl Drop for DcMsgReceiverWrapped {
    fn drop(&mut self) {
        let _inner: Box<MsgReceiverInner> = unsafe { Box::from_raw(self.0.inner as *mut _) };
    }
}

/// Values that are sent between threads when spawn new task.
struct MoveValue {
    pipeline: DcPipeline,
    msg_receiver: DcMsgReceiverWrapped,
}

impl Task {
    pub fn new(id: TaskId, element: ElementPreBuild, mut pipeline: PipelineInner) -> Self {
        pipeline.self_taskid = Some(id);

        Task {
            id,
            element,
            channel: None,
            pipeline: Some(pipeline),
        }
    }

    pub fn id(&self) -> TaskId {
        self.id
    }

    pub fn set_channel(&mut self, channel: Channel) {
        self.channel = Some(channel);
    }

    /// Spawn task under tokio runtime.
    pub fn spawn(mut self, fh: &mut FinalizerHolder) -> Result<JoinHandle<ElementResult>, Error> {
        let (sender, receiver) = self.channel.take().expect(CHANNEL_NONE_ERR_MSG).split();
        let mut move_value = MoveValue {
            pipeline: self.pipeline.take().unwrap().into_ffi(),
            msg_receiver: DcMsgReceiverWrapped(receiver.into_ffi()),
        };

        let ElementExecutable {
            mut next_boxed,
            finalizer,
        } = self.element.build()?;
        let id = self.id;
        if let Some(finalizer) = finalizer {
            fh.append(finalizer);
        }

        Ok(spawn(move || loop {
            if CLOSING.load(Ordering::Relaxed) {
                log::info!("closing task {}", id);
                return Ok(ElementValue::Close);
            }

            let result = next_boxed(&mut move_value.pipeline, &mut move_value.msg_receiver.0);

            match result {
                Ok(ElementValue::Close) => {
                    log::info!("task {} is closed normally", id);
                    crate::close::close();
                    return Ok(ElementValue::Close);
                }
                Ok(ElementValue::MsgBuf) => {
                    let msg = unsafe {
                        let pipeline_inner =
                            &mut *(move_value.pipeline.inner as *mut PipelineInner);
                        let msg_buf = &mut *(pipeline_inner.msg_buf.inner as *mut MsgBufInner);
                        // Use cloned message to pass msg between threads safely.
                        Msg::new(msg_buf.get_msg_cloned())
                    };
                    if let Err(e) = sender.send(SendableMsg(msg), 0) {
                        log::error!("task {} occured sending error\n{}", id, e);
                    }
                }
                Err(e) => {
                    log::error!("task {} is closed with error\n{}", id, e);
                    crate::close::close();
                    return Err(e);
                }
            }
        }))
    }

    pub(crate) fn child(mut self, fh: &mut FinalizerHolder) -> Result<ChildTask, Error> {
        let (_sender, msg_receiver) = self.channel.take().expect(CHANNEL_NONE_ERR_MSG).split();
        let ElementExecutable {
            next_boxed,
            finalizer,
        } = self.element.build()?;
        if let Some(finalizer) = finalizer {
            fh.append(finalizer);
        }

        Ok(ChildTask {
            id: self.id,
            pipeline: self.pipeline.unwrap().into_ffi(),
            next_boxed,
            msg_receiver: DcMsgReceiverWrapped(msg_receiver.into_ffi()),
        })
    }
}

impl ChildTask {
    /// Get next value from task.
    #[allow(clippy::needless_lifetimes)]
    pub fn next<'a>(&'a mut self) -> Result<Msg<'a>, ReceiveError> {
        if CLOSING.load(Ordering::Relaxed) {
            return Err(ReceiveError);
        }

        let result = (self.next_boxed)(&mut self.pipeline, &mut self.msg_receiver.0);
        match result {
            Ok(ElementValue::Close) => {
                log::info!("task {} is closed normally", self.id);
                Err(ReceiveError)
            }
            Ok(ElementValue::MsgBuf) => {
                let msg = unsafe {
                    let pipeline_inner = &mut *(self.pipeline.inner as *mut PipelineInner);
                    let msg_buf = &mut *(pipeline_inner.msg_buf.inner as *mut MsgBufInner);
                    // Use cloned message to pass msg between threads safely.
                    Msg::new(msg_buf.get_msg())
                };
                Ok(msg)
            }
            Err(e) => {
                log::error!("task {}: {}", self.id, e);
                Err(ReceiveError)
            }
        }
    }
}

impl std::str::FromStr for TaskPort {
    type Err = crate::error::TaskPortParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use once_cell::sync::Lazy;
        use regex::Regex;

        static TASK_ID_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new("[0-9]+").unwrap());
        static TASK_PORT_REGEX: Lazy<Regex> =
            Lazy::new(|| Regex::new("([0-9]+):([0-9]+)").unwrap());

        if let Some(cap) = TASK_PORT_REGEX.captures(s) {
            let task_id: u64 = cap.get(1).unwrap().as_str().parse().unwrap();
            let port: u8 = cap.get(2).unwrap().as_str().parse().unwrap();
            return Ok(TaskPort(TaskId(task_id), port));
        }

        if TASK_ID_REGEX.is_match(s) {
            let task_id: u64 = s.parse().unwrap();
            return Ok(TaskPort(TaskId(task_id), Port::default()));
        }

        Err(crate::error::TaskPortParseError(format!(
            "invalid string for task port \"{}\"",
            s
        )))
    }
}
