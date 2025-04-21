use std::cell::OnceCell;
use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{anyhow, bail, Context as _, Result};
use dc_common::conf::{TaskId, TaskPort};

use crate::channel::{create_channels, MsgRecvPorts, MsgSendPorts, TaskGroupWithChannelInfo};
use crate::conf::{Port, TaskConf};
use crate::context::Context;
use crate::element::{ElementExecutable, ElementPreBuilt};
use crate::loader::{LoadedElement, LoadedElements};
use crate::metadata::MetadataIdList;
use crate::msg::Msg;
use crate::msg_receiver::MsgReceiver;
use crate::pipeline::Pipeline;
use crate::plugin::DcElementResult;
use crate::utils::debug_without_newline;

/// Grouped task that can be executed in one thread
#[derive(Debug)]
pub struct TaskGroup {
    pub elements: Vec<ElementPreBuilt>,
    ids: Vec<TaskId>,
    msg_send_ports: Option<MsgSendPorts>,
    msg_recv_ports: Option<MsgRecvPorts>,
}

impl TaskGroup {
    pub fn exec(mut self, context: Arc<Context>) {
        let task_context = TaskContext::new(&context);
        TASK_CONTEXT.with(|c| {
            assert!(c.set(task_context).is_ok());
        });

        let mut task = match to_task_chain(
            0,
            &mut self.elements,
            &self.ids,
            &context,
            &mut self.msg_recv_ports,
        ) {
            Ok(task) => task,
            Err(e) => {
                core_log!(Error, "{}", debug_without_newline(e));
                return;
            }
        };

        loop {
            if task.pipeline.context.closing() {
                core_log!(Info, "close {}", task.id);
                break;
            }

            match task
                .element
                .next(&mut task.pipeline, &mut task.msg_receiver)
            {
                DcElementResult::Close => {
                    core_log!(Info, "task {} closed normally", task.id);
                }
                DcElementResult::Err => {
                    core_log!(
                        Error,
                        "task {} closed with error: {}",
                        task.id,
                        task.pipeline.err_msg
                    );
                }
                DcElementResult::Msg => {
                    if let Some(msg_send_ports) = &mut self.msg_send_ports {
                        if let Some((port, msg)) = task.pipeline.msg.take() {
                            if let Err(e) = msg_send_ports.send(port, msg) {
                                core_log!(Info, "{}", debug_without_newline(e));
                                break;
                            }
                            continue;
                        } else {
                            core_log!(
                                Error,
                                "element returns Msg but any no message set in task {}",
                                task.id
                            );
                        }
                    } else {
                        core_log!(
                            Error,
                            "element returns Msg that don't have send ports in task {}",
                            task.id,
                        );
                    }
                }
                DcElementResult::MsgBuf => {
                    if let Some(msg_send_ports) = &mut self.msg_send_ports {
                        for (port, msg) in task.pipeline.take_msgs() {
                            if let Err(e) = msg_send_ports.send(port, msg) {
                                core_log!(Info, "{}", debug_without_newline(e));
                                break;
                            }
                        }
                        continue;
                    } else {
                        core_log!(
                            Error,
                            "Element returns MsgBuf that don't have send ports in task {}",
                            task.id,
                        );
                    }
                }
            }
            break;
        }
        task.pipeline.context.close();
    }

    pub fn id(&self) -> String {
        self.ids[0].to_string()
    }
}

fn to_task_chain(
    i: usize,
    elements: &mut Vec<ElementPreBuilt>,
    ids: &[TaskId],
    context: &Arc<Context>,
    msg_recv_ports: &mut Option<MsgRecvPorts>,
) -> Result<Task> {
    let id = ids[i].clone();
    let msg_receiver = if i < elements.len() - 1 {
        let child = to_task_chain(i + 1, elements, ids, context, msg_recv_ports)?;
        MsgReceiver::Child(Box::new(child))
    } else {
        // if this is the tail
        if let Some(msg_recv_ports) = msg_recv_ports.take() {
            MsgReceiver::MsgRecvPorts(msg_recv_ports)
        } else {
            MsgReceiver::Empty
        }
    };

    let element = elements.pop().unwrap();

    let pipeline = Pipeline::new(context.clone(), element.element.send_ports);
    let (element, finalizer) = element
        .build()
        .with_context(|| format!("Creating task {}", id))?;

    if finalizer.f.is_some() {
        context.push_finalizer(id.clone(), finalizer);
    }

    Ok(Task {
        id,
        element,
        pipeline,
        msg_receiver,
    })
}

pub struct Task {
    id: TaskId,
    element: ElementExecutable,
    msg_receiver: MsgReceiver,
    pipeline: Pipeline,
}

impl Task {
    pub fn exec_as_child(&mut self) -> Option<Msg> {
        if self.pipeline.context.closing() {
            core_log!(Info, "close task {}", self.id);
            return None;
        }

        match self
            .element
            .next(&mut self.pipeline, &mut self.msg_receiver)
        {
            DcElementResult::Close => {
                core_log!(Info, "task {} closed normally", self.id);
                None
            }
            DcElementResult::Err => {
                core_log!(
                    Error,
                    "task {} closed with error: {}",
                    self.id,
                    self.pipeline.err_msg
                );
                None
            }
            DcElementResult::Msg => match self.pipeline.msg.take() {
                Some((0, msg)) => Some(msg),
                _ => {
                    core_log!(Error, "task {} returns Msg, but no Msg in port 0", self.id);
                    None
                }
            },
            DcElementResult::MsgBuf => Some(self.pipeline.take_msg_port_0()),
        }
    }
}

struct TaskPreparing<'a> {
    conf: &'a TaskConf,
    element_conf: String,
    loaded_element: &'a LoadedElement,
    to: Vec<Vec<TaskPort>>,
}

impl<'a> TaskPreparing<'a> {
    fn childable(&self) -> bool {
        self.to.len() == 1 && self.to[0].len() == 1
    }
}

pub fn create_task_groups(
    loaded_elements: &LoadedElements,
    task_conf: &[TaskConf],
    channel_capacity: usize,
) -> Result<Vec<TaskGroup>> {
    let mut tasks: HashMap<TaskId, TaskPreparing> = HashMap::default();

    for task_conf in task_conf {
        let loaded_element = loaded_elements
            .get(&task_conf.element)
            .ok_or_else(|| anyhow!("Unknown element: {}", task_conf.element))?;

        let task = TaskPreparing {
            conf: task_conf,
            element_conf: serde_json::to_string(&task_conf.conf)?,
            to: vec![Vec::new(); loaded_element.element.send_ports as usize],
            loaded_element,
        };
        if tasks.insert(task_conf.id.clone(), task).is_some() {
            bail!("Task id duplication detected: {}", task_conf.id);
        }
    }

    // Set TaskPreparing::to
    for task_conf in task_conf {
        for (i_recv_port, from) in task_conf.from.iter().enumerate() {
            for send_port in from {
                let task = tasks
                    .get_mut(&send_port.0)
                    .ok_or_else(|| anyhow!("Undefined task specified: {}", send_port.0))?;
                task.to
                    .get_mut(send_port.1 as usize)
                    .ok_or_else(|| anyhow!("Invalid port specified: {}", send_port))?
                    .push(TaskPort(task_conf.id.clone(), i_recv_port as Port));
            }
        }
    }

    // Check port requirement
    for (id, task) in &tasks {
        if task.conf.from.len() != task.loaded_element.element.recv_ports as usize {
            bail!("Invalid port specified to {}", id);
        }
        for port in 0..task.conf.from.len() {
            if task.conf.from[port].is_empty() {
                bail!("No port specified to {}", TaskPort(id.clone(), port as _));
            }
        }

        for port in 0..task.to.len() {
            if task.to[port].is_empty() {
                bail!("No receiver for {}", TaskPort(id.clone(), port as _));
            }
        }
    }

    // List all root tasks that are start point of execution
    // Root tasks don't have parent
    let mut task_groups: HashMap<TaskId, Vec<TaskId>> = tasks
        .iter()
        .filter_map(|(id, task)| {
            if task.childable() {
                let send_to = &tasks[&task.to[0][0].0];
                if send_to.conf.from.len() == 1 && send_to.conf.from[0].len() == 1 {
                    // This task is child
                    None
                } else {
                    // This task is root
                    Some((id.clone(), vec![id.clone()]))
                }
            } else {
                // This task is root
                Some((id.clone(), vec![id.clone()]))
            }
        })
        .collect();

    // Push child
    for ids in &mut task_groups.values_mut() {
        push_childs(&tasks, ids);
    }

    // Check unused task
    let mut unused = Vec::new();
    'target_id_loop: for target_id in task_conf.iter().map(|task_conf| &task_conf.id) {
        for id in task_groups.values().flatten() {
            if target_id == id {
                continue 'target_id_loop;
            }
        }
        unused.push(target_id);
    }
    if !unused.is_empty() {
        bail!("Some tasks are not executable, maybe circular reference");
    }

    // Channel
    let task_groups_with_channel_info: HashMap<_, _> = task_groups
        .iter()
        .map(|(root_id, ids)| {
            let last = ids.last().unwrap();

            let info = TaskGroupWithChannelInfo {
                to: tasks[root_id].to.clone(),
                from: tasks[last].conf.from.clone(),
                last: last.clone(),
            };
            (root_id.clone(), info)
        })
        .collect();

    let mut channels = create_channels(&task_groups_with_channel_info, channel_capacity);

    let task_groups = task_groups
        .into_iter()
        .map(|(root_id, ids)| {
            let elements = ids
                .iter()
                .map(|id| {
                    ElementPreBuilt::new(tasks[id].loaded_element, tasks[id].element_conf.clone())
                })
                .collect();
            let channels = channels.get_mut(&root_id).unwrap();
            TaskGroup {
                elements,
                ids: ids.clone(),
                msg_send_ports: channels.0.take(),
                msg_recv_ports: channels.1.take(),
            }
        })
        .collect();

    Ok(task_groups)
}

fn push_childs(tasks: &HashMap<TaskId, TaskPreparing>, ids: &mut Vec<TaskId>) {
    let parent_id = ids.last().unwrap();
    let from = &tasks.get(parent_id).unwrap().conf.from;

    let child_id = if from.len() == 1 && from[0].len() == 1 {
        &from[0][0].0
    } else {
        return;
    };

    if !tasks.get(child_id).unwrap().childable() {
        return;
    }

    ids.push(child_id.clone());
    push_childs(tasks, ids);
}

thread_local! {
    static TASK_CONTEXT: OnceCell<TaskContext> = const { OnceCell::new() };
}

/// Task(thread) local context
pub struct TaskContext {
    pub(crate) metadata_id_list: Arc<MetadataIdList>,
}

impl TaskContext {
    fn new(context: &Context) -> Self {
        Self {
            metadata_id_list: context.metadata_id_list.clone(),
        }
    }
}

pub fn with_task_context<F: FnMut(&TaskContext) -> R, R>(mut f: F) -> Option<R> {
    TASK_CONTEXT.with(|task_context| {
        let task_context = task_context.get()?;
        Some(f(task_context))
    })
}
