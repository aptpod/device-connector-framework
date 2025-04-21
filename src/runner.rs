use std::{
    ffi::{c_void, CStr, CString},
    path::Path,
};

use sys::{DcElementInfo, DcPlugin};

use crate::{
    element::{register_element_to_plugin, ElementBuildable},
    MsgType, Plugin, Port,
};

/// Runner builder.
pub struct RunnerBuilder {
    runner: *mut sys::DcRunner,
}

impl Default for RunnerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl RunnerBuilder {
    pub fn new() -> Self {
        Self {
            runner: unsafe { sys::dc_runner_new() },
        }
    }

    /// Append a path to directory that includes plugin files.
    pub fn append_dir<P: AsRef<Path>>(self, path: P) -> Result<Self, RunnerError> {
        let path = path_to_cstring(path.as_ref())?;

        unsafe { sys::dc_runner_append_dir(self.runner, path.as_ptr()) }

        Ok(self)
    }

    /// Append a path to a plugin file.
    pub fn append_file<P: AsRef<Path>>(self, path: P) -> Result<Self, RunnerError> {
        let path = path_to_cstring(path.as_ref())?;

        unsafe { sys::dc_runner_append_file(self.runner, path.as_ptr()) }

        Ok(self)
    }

    /// Append an element to runner.
    ///
    /// # Panics
    /// This function may panic if given element has invalid name.
    pub fn append_element<E: ElementBuildable>(self) -> Self {
        let name = format!("append_element:{}", E::NAME);

        unsafe extern "C-unwind" fn plugin_fn<E: ElementBuildable>(
            dc_plugin: *mut DcPlugin,
        ) -> bool {
            unsafe { register_element_to_plugin::<E>(dc_plugin) };
            true
        }

        unsafe { self.append_plugin_init(&name, plugin_fn::<E>) }
    }

    /// Append a plugin to runner.
    ///
    /// # Panics
    /// This function may panic if given element has invalid name.
    pub fn append_plugin<P: Plugin>(self) -> Self {
        unsafe extern "C-unwind" fn plugin_fn<P: Plugin>(dc_plugin: *mut DcPlugin) -> bool {
            unsafe { P::init(dc_plugin) };
            true
        }

        unsafe { self.append_plugin_init(P::NAME, plugin_fn::<P>) }
    }

    /// Append an plugin init function.
    ///
    /// # Safety
    /// Given function must be correctly implemented.
    ///
    /// # Panics
    /// This function may panic if name is invalid.
    pub unsafe fn append_plugin_init(
        self,
        name: &str,
        f: unsafe extern "C-unwind" fn(dc_plugin: *mut DcPlugin) -> bool,
    ) -> Self {
        let name = CString::new(name).unwrap();

        unsafe { sys::dc_runner_append_plugin_init(self.runner, name.as_ptr(), Some(f)) }

        self
    }

    pub fn config<T: Into<String>>(self, config: T) -> Result<Self, RunnerError> {
        let config = config.into();
        let config = CString::new(config).map_err(|e| RunnerError::InvalidConfig(e.to_string()))?;

        unsafe { sys::dc_runner_set_config(self.runner, config.as_ptr()) };

        Ok(self)
    }

    pub fn run(self) -> Result<(), RunnerError> {
        let result = unsafe { sys::dc_runner_run(self.runner) };

        if result == 0 {
            Ok(())
        } else {
            Err(RunnerError::ExecutionFailed)
        }
    }

    pub fn element_info_list(&self) -> Vec<ElementInfo> {
        let list: Vec<ElementInfo> = vec![];
        let list = Box::into_raw(Box::new(list)) as *mut _;

        unsafe {
            sys::dc_runner_iter_elements(self.runner, Some(element_info_callback), list);
        }

        let mut list: Box<Vec<ElementInfo>> = unsafe { Box::from_raw(list as *mut _) };
        list.sort_by(|a, b| a.id.cmp(&b.id));
        *list
    }
}

unsafe extern "C-unwind" fn element_info_callback(p: *mut c_void, info: *const DcElementInfo) {
    unsafe {
        let list = &mut *(p as *mut Vec<ElementInfo>);
        let info = &*info;

        let element_id = CStr::from_ptr(info.id).to_string_lossy();

        let cptr_to_string = |ptr, field| match CStr::from_ptr(ptr).to_str() {
            Ok(s) => s.to_owned(),
            Err(_) => {
                log::warn!(
                    "invalid string detected for {} in element \"{}\"",
                    field,
                    element_id
                );
                CStr::from_ptr(ptr).to_string_lossy().to_string()
            }
        };

        let mut recv_msg_types = Vec::new();
        let mut p = info.recv_msg_types;
        while !(*p).is_null() {
            let mut q = *p;
            let mut msg_types_for_port = Vec::new();
            while !(*q).is_null() {
                let msg_type = CStr::from_ptr(*q)
                    .to_str()
                    .unwrap_or_else(|_| {
                        panic!("invalid msg type in element info of \"{}\"", element_id)
                    })
                    .parse::<MsgType>()
                    .unwrap_or_else(|_| {
                        panic!("invalid msg type in element info of \"{}\"", element_id)
                    });
                q = q.add(1);
                msg_types_for_port.push(msg_type);
            }
            p = p.add(1);
            recv_msg_types.push(msg_types_for_port);
        }

        let mut send_msg_types = Vec::new();
        let mut p = info.send_msg_types;
        while !(*p).is_null() {
            let mut q = *p;
            let mut msg_types_for_port = Vec::new();
            while !(*q).is_null() {
                let msg_type = CStr::from_ptr(*q)
                    .to_str()
                    .unwrap_or_else(|_| {
                        panic!("invalid msg type in element info of \"{}\"", element_id)
                    })
                    .parse::<MsgType>()
                    .unwrap_or_else(|_| {
                        panic!("invalid msg type in element info of \"{}\"", element_id)
                    });
                q = q.add(1);
                msg_types_for_port.push(msg_type);
            }
            p = p.add(1);
            send_msg_types.push(msg_types_for_port);
        }

        let mut metadata_ids = Vec::new();
        let mut p = info.metadata_ids;
        while !(*p).is_null() {
            let metadata_id = cptr_to_string(*p, "metadata_ids");
            p = p.add(1);
            metadata_ids.push(metadata_id);
        }

        let element_info = ElementInfo {
            id: cptr_to_string(info.id, "id"),
            origin: cptr_to_string(info.origin, "origin"),
            authors: cptr_to_string(info.authors, "authors"),
            description: cptr_to_string(info.description, "description"),
            config_doc: cptr_to_string(info.config_doc, "config_doc"),
            recv_ports: info.recv_ports,
            send_ports: info.send_ports,
            recv_msg_types,
            send_msg_types,
            metadata_ids,
        };

        list.push(element_info);
    }
}

#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct ElementInfo {
    pub id: String,
    pub origin: String,
    pub authors: String,
    pub description: String,
    pub config_doc: String,
    pub recv_ports: Port,
    pub send_ports: Port,
    pub recv_msg_types: Vec<Vec<MsgType>>,
    pub send_msg_types: Vec<Vec<MsgType>>,
    pub metadata_ids: Vec<String>,
}

#[cfg(unix)]
fn path_to_cstring(path: &Path) -> Result<CString, RunnerError> {
    use std::os::unix::ffi::OsStrExt;

    CString::new(path.as_os_str().as_bytes()).map_err(|e| RunnerError::InvalidPath(e.to_string()))
}

#[cfg(not(unix))]
fn path_to_cstring(path: &Path) -> Result<CString, RunnerError> {
    let path = path
        .to_str()
        .ok_or_else(|| RunnerError::InvalidPath("invalid utf-8".into()))?;

    CString::new(path).map_err(|e| RunnerError::InvalidPath(e.to_string()))
}

/// Errors of runner.
#[derive(Clone, Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RunnerError {
    #[error("invalid path: {0}")]
    InvalidPath(String),
    #[error("invalid config: {0}")]
    InvalidConfig(String),
    #[error("execution failed")]
    ExecutionFailed,
}
