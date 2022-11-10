use crate::conf::TaskConf;
use crate::element::{ElementBank, Port};
use crate::error::{TypeCheckError, UnknownElementError};
use crate::task::{TaskId, TaskPort};
use common::MsgType;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

#[derive(Clone)]
pub struct TypeChecker(Arc<TypeCheckerInner>);

struct TypeCheckerInner {
    /// Msg destination from one task to multiple tasks.
    dest: HashMap<TaskPort, Vec<TaskPort>>,
    /// Acceptable msg types of each tasks.
    acceptable_msg_types: HashMap<TaskId, Vec<Vec<MsgType>>>,
    /// Checked msg_type
    checked: RwLock<HashMap<TaskPort, MsgType>>,
}

impl TypeChecker {
    pub fn check(&self, from: TaskId, msg_type: MsgType, port: Port) -> Result<(), TypeCheckError> {
        log::info!("task {} emits {}", from, msg_type);

        let tc = &*self.0;
        let taskport = TaskPort(from, port);
        let dest = &tc.dest[&taskport];

        for dest_port in dest {
            let acceptable_msg_types = &tc.acceptable_msg_types[&dest_port.0][port as usize];

            if !acceptable_msg_types.iter().any(|m| m.acceptable(&msg_type)) {
                let err = TypeCheckError(msg_type, *dest_port);
                log::error!("{}", err);
                return Err(err);
            }
        }

        self.0.checked.write().unwrap().insert(taskport, msg_type);

        Ok(())
    }

    pub fn new(bank: &ElementBank, conf: &[TaskConf]) -> Result<Self, UnknownElementError> {
        let mut tc = TypeCheckerInner {
            dest: HashMap::default(),
            acceptable_msg_types: HashMap::default(),
            checked: RwLock::new(HashMap::default()),
        };

        for conf in conf {
            for (port, from) in conf.from.iter().enumerate() {
                for from in from {
                    tc.dest
                        .entry(*from)
                        .or_insert_with(Vec::new)
                        .push(TaskPort(conf.id, port as u8));
                }
            }

            tc.acceptable_msg_types
                .insert(conf.id, bank.acceptable_msg_types(&conf.element)?);
        }
        Ok(TypeChecker(Arc::new(tc)))
    }
}
