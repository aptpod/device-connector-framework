use crate::element::ElementFinalizer;
use once_cell::sync::Lazy;
use std::sync::Mutex;

pub(crate) static AFTER_SCRIPT: Lazy<Mutex<Vec<String>>> = Lazy::new(|| Mutex::new(Vec::new()));
static FINALIZER_HOLDER: Lazy<Mutex<Option<FinalizerHolder>>> = Lazy::new(|| Mutex::new(None));

#[derive(Default)]
pub struct FinalizerHolder {
    finalizers: Vec<ElementFinalizer>,
}

impl FinalizerHolder {
    pub fn append(&mut self, finalizer: ElementFinalizer) {
        self.finalizers.push(finalizer);
    }
}

pub(crate) fn register(fh: FinalizerHolder) {
    *FINALIZER_HOLDER.lock().unwrap() = Some(fh);
}

pub(crate) fn finalize() {
    let mut lock = FINALIZER_HOLDER.lock().unwrap();

    if let Some(fh) = lock.take() {
        log::info!("execute finalizers");
        for finalizer in fh.finalizers.into_iter() {
            if let Err(e) = finalizer() {
                log::error!("error in finalizer\n{:?}", e);
            }
        }

        log::info!("execute after script");
        let after_script = &*AFTER_SCRIPT.lock().unwrap();
        if let Err(e) = crate::process::exec_script_lines(after_script) {
            log::error!("{:?}", e);
        }
    }
}
