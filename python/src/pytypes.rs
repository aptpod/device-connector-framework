use std::sync::mpsc;

use dc_core::Port;
use pyo3::{exceptions::PyTypeError, prelude::*};

#[derive(PartialEq, Eq, Debug)]
pub enum PollValue {
    Msg(PyMsg),
    Receiving,
    Close,
}

#[pyclass(name = "MsgReceiver")]
pub struct PyMsgReceiver {
    tx_poll_value: mpsc::SyncSender<PollValue>,
    rx_msg: mpsc::Receiver<PyMsg>,
}

#[pymethods]
impl PyMsgReceiver {
    #[pyo3(signature = (port=0))]
    fn recv(&mut self, port: Port) -> PyResult<Option<PyMsg>> {
        if port != 0 {
            return Err(PyErr::new::<PyTypeError, _>(
                "Ports other than 0 are not supported",
            ));
        }
        let _ = self.tx_poll_value.send(PollValue::Receiving);
        Ok(self.rx_msg.recv().ok())
    }
}

#[pyclass(name = "Pipeline")]
pub struct PyPipeline {
    tx_poll_value: mpsc::SyncSender<PollValue>,
}

#[pymethods]
impl PyPipeline {
    #[pyo3(signature = (msg, port=0))]
    fn send_msg(&mut self, msg: &PyMsg, port: Port) -> PyResult<()> {
        if port != 0 {
            return Err(PyErr::new::<PyTypeError, _>(
                "Ports other than 0 are not supported",
            ));
        }
        let _ = self.tx_poll_value.send(PollValue::Msg(msg.clone()));
        Ok(())
    }
}

#[pyclass(name = "Msg")]
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PyMsg {
    pub(crate) bytes: Vec<u8>,
}

#[pymethods]
impl PyMsg {
    #[new]
    pub fn new(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }

    #[getter]
    pub fn get_bytes(&self) -> &[u8] {
        &self.bytes
    }

    #[setter]
    pub fn set_bytes(&mut self, bytes: &[u8]) {
        self.bytes = bytes.to_vec();
    }
}
