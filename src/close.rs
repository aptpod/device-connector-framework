use once_cell::sync::Lazy;
use std::sync::atomic::Ordering;
use std::sync::Mutex;
use std::time::Duration;

static CLOSE_HANDLER: Lazy<Mutex<CloseHandler>> = Lazy::new(|| Mutex::new(CloseHandler(false)));

struct CloseHandler(bool);

impl CloseHandler {
    fn close(&mut self) {
        if !self.0 {
            self.0 = true;

            std::thread::spawn(|| {
                crate::task::CLOSING.store(true, Ordering::Relaxed);
                std::thread::sleep(Duration::from_millis(1000));
                log::info!("process will exit before closing tasks");
                crate::finalizer::finalize();
                std::process::exit(0);
            });
        }
    }
}

pub fn close() {
    CLOSE_HANDLER.lock().unwrap().close();
}
