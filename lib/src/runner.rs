use std::{
    ffi::{CStr, CString},
    sync::Arc,
};

use libc::{c_char, c_int, c_void};

use crate::{
    conf::Conf,
    context::Context,
    loader::{DcPluginInitFunc, PluginLoader},
    plugin::DcPort,
};

/// Device connector runner
#[repr(C)]
pub struct DcRunner {
    _data: [u8; 0],
    _marker: core::marker::PhantomData<(*mut u8, core::marker::PhantomPinned)>,
}

pub struct Runner {
    conf: Option<String>,
    loader: PluginLoader,
}

/// Create a runner.
#[no_mangle]
pub unsafe extern "C" fn dc_runner_new() -> *mut DcRunner {
    let runner = Box::new(Runner {
        conf: None,
        loader: PluginLoader::default(),
    });
    Box::into_raw(runner) as *mut _
}

/// Set configuration to a runner.
#[no_mangle]
pub unsafe extern "C" fn dc_runner_set_config(runner: *mut DcRunner, config: *const c_char) {
    let runner = unsafe { &mut *(runner as *mut Runner) };
    let conf = match unsafe { CStr::from_ptr(config) }.to_str() {
        Ok(conf) => conf,
        Err(e) => {
            core_log!(Error, "invalid config string: {}", e);
            return;
        }
    };
    runner.conf = Some(conf.to_owned());
}

/// Append a path to directory that includes plugin files.
#[no_mangle]
pub unsafe extern "C" fn dc_runner_append_dir(runner: *mut DcRunner, path: *const c_char) {
    let runner = unsafe { &mut *(runner as *mut Runner) };
    let path = match unsafe { CStr::from_ptr(path) }.to_str() {
        Ok(path) => path,
        Err(e) => {
            core_log!(Error, "invalid path string: {}", e);
            return;
        }
    };
    runner.loader.append_dir(path);
}

/// Append a path to a plugin file.
#[no_mangle]
pub unsafe extern "C" fn dc_runner_append_file(runner: *mut DcRunner, path: *const c_char) {
    let runner = unsafe { &mut *(runner as *mut Runner) };
    let path = match unsafe { CStr::from_ptr(path) }.to_str() {
        Ok(path) => path,
        Err(e) => {
            core_log!(Error, "invalid path string: {}", e);
            return;
        }
    };
    runner.loader.append_file(path);
}

/// Append a plugin init function.
#[no_mangle]
pub unsafe extern "C" fn dc_runner_append_plugin_init(
    runner: *mut DcRunner,
    name: *const c_char,
    f: DcPluginInitFunc,
) {
    let runner = unsafe { &mut *(runner as *mut Runner) };
    let name = match unsafe { CStr::from_ptr(name) }.to_str() {
        Ok(name) => name,
        Err(e) => {
            core_log!(Error, "invalid name: {}", e);
            return;
        }
    };
    runner.loader.append_fn(name, f);
}

/// Run.
#[no_mangle]
pub unsafe extern "C" fn dc_runner_run(runner: *mut DcRunner) -> c_int {
    let mut runner = unsafe { Box::from_raw(runner as *mut Runner) };

    let Some(conf) = runner.conf else {
        core_log!(Error, "no config for DcRunner");
        return 1;
    };

    let mut conf = match Conf::from_yaml(&conf) {
        Ok(conf) => conf,
        Err(e) => {
            core_log!(Error, "config parse error: {:?}", e);
            return 1;
        }
    };

    for task in &mut conf.tasks {
        task.conf.remove_null_from_map();
    }
    let conf = conf;

    for plugin_file in &conf.plugin.plugin_files {
        runner.loader.append_file(plugin_file);
    }

    let (loaded_elements, _libs) = runner.loader.load();

    let context = Arc::new(Context::new(&conf, &loaded_elements));

    let c = context.clone();
    if let Err(e) = ctrlc::set_handler(move || {
        core_log!(Info, "process received exit signal");
        c.close();
    }) {
        core_log!(Warn, "could not set handler for ctrl-c: {:?}", e);
    }

    let task_groups = match crate::task::create_task_groups(
        &loaded_elements,
        &conf.tasks,
        conf.runner.channel_capacity,
    ) {
        Ok(task_groups) => task_groups,
        Err(e) => {
            core_log!(Error, "could not create tasks: {:?}", e);
            return 1;
        }
    };

    if let Err(e) = crate::process::start_bg_processes(&conf.bg_processes) {
        core_log!(Error, "background process spawn failed: {:?}", e);
        return 1;
    }

    if let Err(e) = crate::process::exec_before_task(&conf.before_task) {
        core_log!(Error, "before task failed: {:?}", e);
        return 1;
    }

    core_log!(Info, "spawning tasks.");

    let join_handles: Vec<_> = task_groups
        .into_iter()
        .map(|task_group| {
            let c = context.clone();
            std::thread::Builder::new()
                .name(format!("dc({})", task_group.id()))
                .spawn(move || {
                    task_group.exec(c);
                })
                .expect("Cannot spawn a thread")
        })
        .collect();

    for jh in join_handles {
        if jh.join().is_err() {
            core_log!(Error, "thread join error");
        }
        context.close();
    }

    context.exec_finalizers(conf.runner.finalizer_timeout);

    0
}

#[repr(C)]
#[non_exhaustive]
pub struct DcElementInfo {
    pub id: *const c_char,
    pub origin: *const c_char,
    pub authors: *const c_char,
    pub description: *const c_char,
    pub config_doc: *const c_char,
    pub recv_ports: DcPort,
    pub send_ports: DcPort,
    pub recv_msg_types: *const *const *const c_char,
    pub send_msg_types: *const *const *const c_char,
    pub metadata_ids: *const *const c_char,
    pub _extension_fields: [u8; 0],
}

pub type DcRunnerIterElementsFunc = unsafe extern "C" fn(*mut c_void, *const DcElementInfo);

/// Iterate elements by callback.
#[no_mangle]
pub unsafe extern "C-unwind" fn dc_runner_iter_elements(
    runner: *mut DcRunner,
    f: DcRunnerIterElementsFunc,
    p: *mut c_void,
) {
    let runner = unsafe { &*(runner as *const Runner) };

    let mut cstr_store: Vec<CString> = Vec::new();
    let mut to_cstr = |s: &str| -> *const c_char {
        let cstr =
            CString::new(s).unwrap_or_else(|_| CString::new("###INVALID STRING###").unwrap());
        let p = cstr.as_ptr();
        cstr_store.push(cstr);
        p
    };

    let (loaded_elements, _libs) = runner.loader.clone().load();

    for (id, loaded_element) in loaded_elements {
        let element = &loaded_element.element;
        let mut metadata_ids: Vec<*const c_char> = element
            .metadata_ids
            .iter()
            .map(|metadata_id| to_cstr(metadata_id))
            .collect();
        metadata_ids.push(std::ptr::null());

        let mut internal_vecs: Vec<Vec<*const c_char>> = Vec::new();
        let mut recv_msg_types: Vec<*const *const c_char> = Vec::new();
        for types in &element.recv_msg_types {
            let mut recv_msg_types_for_port: Vec<*const c_char> = Vec::new();
            for t in types {
                recv_msg_types_for_port.push(to_cstr(&t.to_string()));
            }
            recv_msg_types_for_port.push(std::ptr::null());
            recv_msg_types.push(recv_msg_types_for_port.as_ptr());
            internal_vecs.push(recv_msg_types_for_port);
        }
        recv_msg_types.push(std::ptr::null());

        let mut send_msg_types: Vec<*const *const c_char> = Vec::new();
        for types in &element.send_msg_types {
            let mut send_msg_types_for_port: Vec<*const c_char> = Vec::new();
            for t in types {
                send_msg_types_for_port.push(to_cstr(&t.to_string()));
            }
            send_msg_types_for_port.push(std::ptr::null());
            send_msg_types.push(send_msg_types_for_port.as_ptr());
            internal_vecs.push(send_msg_types_for_port);
        }
        send_msg_types.push(std::ptr::null());

        let authors = if let Some(plugin_info) = &loaded_element.plugin_info {
            plugin_info.authors.as_str()
        } else {
            ""
        };

        let info = DcElementInfo {
            id: to_cstr(&id),
            origin: to_cstr(&loaded_element.origin.to_string()),
            authors: to_cstr(authors),
            description: to_cstr(&element.description),
            config_doc: to_cstr(&element.config_doc),
            recv_ports: element.recv_ports,
            send_ports: element.send_ports,
            recv_msg_types: recv_msg_types.as_ptr(),
            send_msg_types: send_msg_types.as_ptr(),
            metadata_ids: metadata_ids.as_ptr(),
            _extension_fields: [],
        };

        unsafe { f(p, &info) };
    }
}
