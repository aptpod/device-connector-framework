use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, Once,
    },
    time::Duration,
};

use dc_common::conf::{Conf, TaskId};

use crate::{loader::LoadedElements, metadata::MetadataIdList, plugin::DcFinalizer};

pub struct Context {
    closing: AtomicBool,
    closer_spawned: Once,
    termination_timeout: Duration,
    finalizer_timeout: Duration,
    finalizers: Arc<Mutex<Option<Finalizers>>>,
    pub(crate) metadata_id_list: Arc<MetadataIdList>,
}

struct Finalizers {
    finalizers: Vec<(TaskId, DcFinalizer)>,
    after_task: Vec<String>,
}

impl Context {
    pub fn new(conf: &Conf, loaded_elements: &LoadedElements) -> Self {
        let metadata_id_list = Arc::new(MetadataIdList::new(loaded_elements));
        let finalizers = Finalizers {
            finalizers: Vec::new(),
            after_task: conf.after_task.clone(),
        };
        Self {
            closing: AtomicBool::new(false),
            closer_spawned: Once::new(),
            finalizers: Arc::new(Mutex::new(Some(finalizers))),
            termination_timeout: conf.runner.termination_timeout,
            finalizer_timeout: conf.runner.finalizer_timeout,
            metadata_id_list,
        }
    }

    pub fn close(&self) {
        self.closer_spawned.call_once(|| {
            core_log!(Info, "closing..");
            self.closing.store(true, Ordering::Relaxed);
            let timeout = self.termination_timeout;
            let finalizer_timeout = self.finalizer_timeout;
            let finalizers = self.finalizers.clone();
            std::thread::spawn(move || closer(timeout, finalizer_timeout, finalizers));
        });
    }

    pub fn closing(&self) -> bool {
        self.closing.load(Ordering::Relaxed)
    }

    pub fn push_finalizer(&self, id: TaskId, finalizer: DcFinalizer) {
        self.finalizers
            .lock()
            .unwrap() // Mutex should not be poisoned because panics cause abort
            .as_mut()
            .unwrap()
            .finalizers
            .push((id, finalizer));
    }

    pub fn exec_finalizers(&self, timeout: Duration) {
        exec_finalizers(&self.finalizers, timeout);
    }
}

fn exec_finalizers(finalizers: &Mutex<Option<Finalizers>>, timeout: Duration) {
    let mut finalizers = finalizers.lock().unwrap();

    let Some(finalizers) = finalizers.take() else {
        return;
    };

    std::thread::spawn(move || {
        std::thread::sleep(timeout);
        core_log!(
            Error,
            "finalizer or after task execution takes too long, quitting.."
        );
        std::process::exit(1);
    });

    for (id, finalizer) in finalizers.finalizers.into_iter() {
        if let Some(f) = finalizer.f {
            if !unsafe { f(finalizer.context) } {
                core_log!(Warn, "a finalizer for task {} execution failed", id);
            }
        }
    }

    if let Err(e) = crate::process::exec_script_lines(&finalizers.after_task) {
        core_log!(Error, "{}", e);
        core_log!(Error, "after task failed");
    }
}

fn closer(
    timeout: Duration,
    finalizer_timeout: Duration,
    finalizers: Arc<Mutex<Option<Finalizers>>>,
) {
    std::thread::sleep(timeout);
    core_log!(Info, "process will exit before closing tasks");

    exec_finalizers(&finalizers, finalizer_timeout);
    std::process::exit(0);
}
