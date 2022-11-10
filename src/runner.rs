use crate::channel::ChannelBuilder;
use crate::conf::*;
use crate::element::*;
use crate::finalizer::FinalizerHolder;
use crate::loaded_plugin::LoadedPlugin;
use crate::pipeline::PipelineInner;
use crate::task::*;
use crate::type_check::TypeChecker;
use anyhow::{bail, Result};
use std::collections::HashMap;

/// Run built tasks.
pub struct Runner<'b, 'p> {
    tasks: Vec<Task>,
    _conf: Conf,
    _bank: &'b ElementBank,
    _loaded_plugin: &'p LoadedPlugin,
    fh: FinalizerHolder,
}

/// Runner builder.
pub struct RunnerBuilder<'b, 'p> {
    tasks: Vec<Task>,
    channels: HashMap<TaskId, Vec<Vec<TaskPort>>>,
    ports: HashMap<TaskId, (Port, Port)>,
    conf: Conf,
    bank: &'b ElementBank,
    loaded_plugin: &'p LoadedPlugin,
    pipeline: Option<PipelineInner>,
    fh: FinalizerHolder,
}

impl<'b, 'p> RunnerBuilder<'b, 'p> {
    pub fn new(bank: &'b ElementBank, loaded_plugin: &'p LoadedPlugin, conf: &Conf) -> Self {
        RunnerBuilder {
            tasks: Vec::new(),
            channels: HashMap::new(),
            ports: HashMap::new(),
            conf: conf.clone(),
            bank,
            loaded_plugin,
            pipeline: None,
            fh: FinalizerHolder::default(),
        }
    }

    fn append_task(
        &mut self,
        id: TaskId,
        element: ElementPreBuild,
        receive_from: &[Vec<TaskPort>],
        ports: (Port, Port),
    ) {
        let task = Task::new(
            id,
            element,
            self.pipeline
                .as_ref()
                .map(|pipeline| pipeline.clone())
                .unwrap(),
        );
        self.tasks.push(task);
        self.channels.insert(id, receive_from.to_vec());
        self.ports.insert(id, ports);
    }

    pub fn append_from_conf(&mut self, conf: &[TaskConf]) -> Result<()> {
        let tc = TypeChecker::new(self.bank, conf)?;
        let pipeline = PipelineInner::new(tc);
        self.pipeline = Some(pipeline);

        for task_conf in conf {
            let element = self.conf_to_element(task_conf)?;
            // TODO: Port handling for plugin elements
            let ports = self.bank.ports(&task_conf.element)?;
            self.append_task(task_conf.id, element, &task_conf.from, ports);
        }

        Ok(())
    }

    pub fn build(mut self) -> Result<Runner<'b, 'p>> {
        self.set_channel()?;

        Ok(Runner {
            tasks: self.tasks,
            _conf: self.conf,
            _bank: self.bank,
            _loaded_plugin: self.loaded_plugin,
            fh: self.fh,
        })
    }

    fn conf_to_element(&self, conf: &TaskConf) -> Result<ElementPreBuild> {
        Ok(self
            .bank
            .pre_build(conf.element.as_ref(), conf.conf.clone().unwrap_or_default())?)
    }

    fn set_channel(&mut self) -> Result<()> {
        let task_ids: Vec<TaskId> = self.tasks.iter().map(|task| task.id()).collect();
        let mut channels: HashMap<TaskId, ChannelBuilder> = HashMap::new();

        for task_id in &task_ids {
            if channels.get(task_id).is_some() {
                bail!("task id duplication detected (id: {})", task_id);
            }
            let ports = self.ports[task_id];
            channels.insert(*task_id, ChannelBuilder::new(ports.0, ports.1));
        }

        // Set channels
        for task_id in &task_ids {
            // Receive from these ports
            if let Some(origins) = self.channels.get(task_id) {
                if origins.len() == 1 && origins[0].len() == 1 {
                    let child_task_port = origins[0][0];
                    let child_task_id = child_task_port.0;
                    let i = self.tasks.iter().enumerate().find_map(|(i, task)| {
                        if task.id() == child_task_id {
                            Some(i)
                        } else {
                            None
                        }
                    });
                    let i = if let Some(i) = i { i } else { todo!() };
                    let mut child_task = self.tasks.remove(i);
                    if let Some(channel) = channels.remove(&child_task_id) {
                        child_task.set_channel(channel.build());
                    } else {
                        bail!(
                            "task {} not found that is referenced by task {}",
                            child_task_id,
                            task_id
                        )
                    }
                    channels
                        .get_mut(task_id)
                        .unwrap()
                        .set_child(child_task.child(&mut self.fh)?);
                } else {
                    for (recv_port, origins) in origins.iter().enumerate() {
                        for origin in origins {
                            // Set channel origin -> task_id:recv_port
                            let sender = channels
                                .get_mut(task_id)
                                .unwrap()
                                .get_sender(recv_port as Port);
                            if let Some(origin_channel) = channels.get_mut(&origin.0) {
                                origin_channel.set_sender(sender, origin.1);
                            } else {
                                bail!(
                                    "task {} not found that is referenced by task {}",
                                    origin,
                                    task_id
                                )
                            }
                        }
                    }
                }
            }
        }

        // Set built channel for tasks
        for task in &mut self.tasks {
            let task_id = task.id();
            if let Some(channel) = channels.remove(&task_id) {
                task.set_channel(channel.build());
            } else {
                todo!()
            }
        }

        Ok(())
    }
}

impl<'b, 'p> Runner<'b, 'p> {
    /// Runs and wait for closing.
    pub fn run(self) -> Result<()> {
        // Start tasks
        let mut fh = self.fh;
        let mut tasks = Vec::new();
        for task in self.tasks.into_iter() {
            log::info!("spawn task {}", task.id());
            let jh = task.spawn(&mut fh)?;
            tasks.push(jh);
        }

        crate::finalizer::register(fh);

        ctrlc::set_handler(|| {
            log::info!("process received exit signal");
            crate::close::close();
        })?;

        while let Some(jh) = tasks.pop() {
            let _result = jh.join();
        }

        crate::finalizer::finalize();

        Ok(())
    }
}
