use crate::pytypes::*;

use std::path::PathBuf;
use std::sync::Once;

use crossbeam::atomic::AtomicCell;
use crossbeam::channel::{bounded, Receiver, Sender};
use pyo3::prelude::*;
use pyo3::types::{PyAnyMethods, PyList, PyModule, PyModuleMethods};

static MODULE_INIT: Once = Once::new();
static EXECUTOR_COUNTER: AtomicCell<usize> = AtomicCell::new(1);

pub struct SimpleExecutor {
    tx_msg: Sender<PyMsg>,
    rx_poll_value: Receiver<PollValue>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ExecutorKind {
    Src,
    Filter,
    Sink,
}

impl SimpleExecutor {
    pub fn start(
        script: (String, String),
        function_name: impl Into<String>,
        import_dir: Option<PathBuf>,
        kind: ExecutorKind,
    ) -> Self {
        MODULE_INIT.call_once(|| {
            pyo3::append_to_inittab!(dc_core);
            pyo3::prepare_freethreaded_python();
        });

        let function_name = function_name.into();
        let (tx_poll_value, rx_poll_value) = bounded(1);
        let (tx_msg, rx_msg) = bounded(1);

        std::thread::spawn(move || {
            start(
                script,
                function_name,
                import_dir,
                kind,
                tx_poll_value.clone(),
                rx_msg,
            );
            let _ = tx_poll_value.send(PollValue::Close);
        });

        Self {
            tx_msg,
            rx_poll_value,
        }
    }

    pub fn poll(&mut self) -> Option<PollValue> {
        self.rx_poll_value.recv().ok()
    }

    pub fn send(&mut self, msg: PyMsg) {
        let _ = self.tx_msg.send(msg);
    }
}

pub fn start(
    script: (String, String),
    function_name: String,
    import_dir: Option<PathBuf>,
    kind: ExecutorKind,
    tx_poll_value: Sender<PollValue>,
    rx_msg: Receiver<PyMsg>,
) {
    Python::with_gil(|py| {
        let result = exec_script(
            py,
            script,
            function_name,
            import_dir,
            kind,
            tx_poll_value,
            rx_msg,
        );
        if let Err(e) = result {
            log::error!("python error: {}", e);
            if let Some(et) = e.traceback_bound(py) {
                if let Ok(e) = et.into_gil_ref().format() {
                    log::error!("{}", e);
                }
            }
        }
    });
}

fn exec_script(
    py: Python<'_>,
    (script, file_name): (String, String),
    function_name: String,
    import_dir: Option<PathBuf>,
    kind: ExecutorKind,
    tx_poll_value: Sender<PollValue>,
    rx_msg: Receiver<PyMsg>,
) -> PyResult<()> {
    let module_name = format!(
        "_dc_python_simple_executor_module_{}",
        EXECUTOR_COUNTER.fetch_add(1)
    );
    if let Some(import_dir) = import_dir {
        log::debug!("append {} to python sys.path", import_dir.display());
        let syspath = py
            .import_bound("sys")?
            .getattr("path")?
            .downcast_into::<PyList>()?;
        syspath.insert(0, import_dir)?;
    }
    let pyfunc: Py<PyAny> = PyModule::from_code_bound(py, &script, &file_name, &module_name)?
        .getattr(function_name.as_str())?
        .into();

    loop {
        match kind {
            ExecutorKind::Src => {
                let result = pyfunc.call0(py)?;

                if let Ok(msgs) = result.extract::<'_, '_, Vec<PyMsg>>(py) {
                    for msg in msgs {
                        if py
                            .allow_threads(|| tx_poll_value.send(PollValue::Msg(msg)))
                            .is_err()
                        {
                            break;
                        }
                    }
                } else {
                    let msg: PyMsg = result.extract(py)?;
                    if py
                        .allow_threads(|| tx_poll_value.send(PollValue::Msg(msg)))
                        .is_err()
                    {
                        break;
                    }
                }
            }
            ExecutorKind::Filter => {
                if py
                    .allow_threads(|| tx_poll_value.send(PollValue::Receiving))
                    .is_err()
                {
                    break;
                }
                let Ok(msg) = py.allow_threads(|| rx_msg.recv()) else {
                    break;
                };

                let result = pyfunc.call1(py, (msg,))?;

                if let Ok(msgs) = result.extract::<'_, '_, Vec<PyMsg>>(py) {
                    for msg in msgs {
                        if py
                            .allow_threads(|| tx_poll_value.send(PollValue::Msg(msg)))
                            .is_err()
                        {
                            break;
                        }
                    }
                } else {
                    let msg: PyMsg = result.extract(py)?;
                    if py
                        .allow_threads(|| tx_poll_value.send(PollValue::Msg(msg)))
                        .is_err()
                    {
                        break;
                    }
                }
            }
            ExecutorKind::Sink => {
                if py
                    .allow_threads(|| tx_poll_value.send(PollValue::Receiving))
                    .is_err()
                {
                    break;
                }
                let Ok(msg) = py.allow_threads(|| rx_msg.recv()) else {
                    break;
                };
                pyfunc.call1(py, (msg,))?;
            }
        }
    }

    Ok(())
}

// Simple dc_core module for SimpleExecutor
#[pymodule]
fn dc_core(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyMsg>()?;
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn simple_exexcutor_src() {
        let mut executor = SimpleExecutor::start(
            (
                r#"
import dc_core

def testsrc():
    msg = dc_core.Msg(bytes([0, 1, 2, 3]))
    return msg
            "#
                .into(),
                "".into(),
            ),
            "testsrc",
            None,
            ExecutorKind::Src,
        );

        for _ in 0..100 {
            assert_eq!(
                executor.poll().unwrap(),
                PollValue::Msg(PyMsg::new(vec![0, 1, 2, 3]))
            );
        }
    }

    #[test]
    fn simple_exexcutor_filter() {
        let mut executor = SimpleExecutor::start(
            (
                r#"
import dc_core

def testfilter(msg):
    msg = dc_core.Msg(msg.bytes + bytes([4, 5, 6]))
    return msg
            "#
                .into(),
                "".into(),
            ),
            "testfilter",
            None,
            ExecutorKind::Filter,
        );

        for _ in 0..100 {
            assert_eq!(executor.poll().unwrap(), PollValue::Receiving);
            executor.send(PyMsg::new(vec![0, 1, 2, 3]));
            assert_eq!(
                executor.poll().unwrap(),
                PollValue::Msg(PyMsg::new(vec![0, 1, 2, 3, 4, 5, 6])),
            );
        }
    }

    #[test]
    fn simple_exexcutor_sink() {
        let mut executor = SimpleExecutor::start(
            (
                r#"
import dc_core

def testsink(msg):
    assert msg.bytes == bytes([0, 1, 2, 3])
            "#
                .into(),
                "".into(),
            ),
            "testsink",
            None,
            ExecutorKind::Sink,
        );

        for _ in 0..100 {
            assert_eq!(executor.poll().unwrap(), PollValue::Receiving);
            executor.send(PyMsg::new(vec![0, 1, 2, 3]));
        }
    }
}
