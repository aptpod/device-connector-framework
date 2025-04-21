use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use semver::Version;

use crate::plugin::{DcPlugin, Element, Plugin};

pub type DcPluginInitFunc = unsafe extern "C-unwind" fn(dc_plugin: *mut DcPlugin) -> bool;

#[derive(Clone, Default)]
pub struct PluginLoader {
    files: Vec<PathBuf>,
    fns: HashMap<String, DcPluginInitFunc>,
}

pub type LoadedElements = HashMap<String, LoadedElement>;

#[derive(Clone)]
pub struct LoadedElement {
    pub origin: ElementOrigin,
    pub framework_version: Version,
    pub element: &'static Element,
    pub plugin_info: Option<Arc<PluginInfo>>,
}

#[derive(Clone, Debug)]
pub enum ElementOrigin {
    File(String),
    PluginInitFn(String),
}

#[derive(Clone, Debug)]
pub struct PluginInfo {
    pub path: String,
    pub name: String,
    pub authors: String,
}

impl PluginLoader {
    pub fn append_dir<P: AsRef<Path>>(&mut self, path: P) {
        let path = path.as_ref();

        let read_dir = match std::fs::read_dir(path) {
            Ok(read_dir) => read_dir,
            Err(e) => {
                core_log!(Warn, "cannot open directory {}: {}", path.display(), e);
                return;
            }
        };

        for entry in read_dir {
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => {
                    core_log!(Warn, "error occured when reading {}: {}", path.display(), e);
                    continue;
                }
            };
            let path = entry.path();
            if path.is_dir() {
                self.append_dir(&path);
            } else if let (Some(file_name), Some(extension)) = (
                path.file_name().and_then(|file_name| file_name.to_str()),
                path.extension().and_then(|extension| extension.to_str()),
            ) {
                if file_name.starts_with("libdc")
                    && extension == plugin_extension()
                    && !file_name.starts_with("libdc_core")
                {
                    self.files.push(path.to_owned());
                }
            }
        }
    }

    pub fn append_file<P: AsRef<Path>>(&mut self, path: P) {
        let path = path.as_ref();
        self.files.push(path.to_owned());
    }

    pub fn append_fn(&mut self, name: &str, f: DcPluginInitFunc) {
        if self.fns.insert(name.into(), f).is_some() {
            core_log!(Error, "init function name duplication detected: {}", name);
        }
    }

    pub fn load(mut self) -> (LoadedElements, Vec<libloader::Library>) {
        let mut libs = Vec::new();

        self.files.dedup();
        let mut loaded_elements = LoadedElements::default();

        for file in &self.files {
            core_log!(Debug, "loading plugin {}", file.display());
            unsafe {
                let lib = match libloader::load(file) {
                    Ok(lib) => lib,
                    Err(e) => {
                        core_log!(Warn, "cannot open plugin {}: {}", file.display(), e);
                        continue;
                    }
                };
                let dc_plugin_init: libloader::Symbol<DcPluginInitFunc> =
                    match lib.get(b"dc_plugin_init") {
                        Ok(dc_plugin_fn) => dc_plugin_fn,
                        Err(e) => {
                            core_log!(
                                Warn,
                                "cannot get dc_plugin_init from {}: {}",
                                file.display(),
                                e
                            );
                            continue;
                        }
                    };

                let mut plugin = Plugin::default();
                if !dc_plugin_init(&mut plugin as *mut _ as *mut DcPlugin) {
                    core_log!(Warn, "dc_plugin_init from {} failed", file.display());
                    continue;
                }
                libs.push(lib);

                let plugin_info = Arc::new(PluginInfo {
                    path: file.display().to_string(),
                    name: plugin.name.clone(),
                    authors: plugin.authors.clone(),
                });

                for element in &plugin.elements {
                    let loaded_element = LoadedElement {
                        element,
                        origin: ElementOrigin::File(plugin_info.path.clone()),
                        framework_version: plugin.version.clone(),
                        plugin_info: Some(plugin_info.clone()),
                    };
                    if let Some(duplicate_element) =
                        loaded_elements.insert(element.name.clone(), loaded_element)
                    {
                        core_log!(
                            Warn,
                            "element duplication detected {}, replace {} by {}",
                            element.name,
                            duplicate_element.origin,
                            ElementOrigin::File(file.display().to_string()),
                        );
                    }
                }
            }
        }

        for (name, dc_plugin_init) in self.fns.iter() {
            core_log!(Debug, "calling init function {}", name);
            let mut plugin = Plugin::default();
            if !unsafe { dc_plugin_init(&mut plugin as *mut _ as *mut DcPlugin) } {
                core_log!(Warn, "init function ({}) failed", name);
                continue;
            }

            for element in &plugin.elements {
                let origin = ElementOrigin::PluginInitFn(name.into());
                let loaded_element = LoadedElement {
                    element,
                    origin: origin.clone(),
                    framework_version: plugin.version.clone(),
                    plugin_info: None,
                };
                if let Some(duplicate_element) =
                    loaded_elements.insert(element.name.clone(), loaded_element)
                {
                    core_log!(
                        Warn,
                        "element duplication detected {}, replace {} by {}",
                        element.name,
                        duplicate_element.origin,
                        origin,
                    );
                }
            }
        }

        (loaded_elements, libs)
    }
}

impl std::fmt::Display for ElementOrigin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ElementOrigin::File(plugin) => {
                write!(f, "plugin {}", plugin)?;
            }
            ElementOrigin::PluginInitFn(name) => {
                write!(f, "dc_plugin_init {}", name)?;
            }
        }
        Ok(())
    }
}

#[cfg(unix)]
fn plugin_extension() -> &'static str {
    "so"
}

#[cfg(windows)]
fn plugin_extension() -> &'static str {
    "dll"
}

#[cfg(not(any(unix, windows)))]
fn plugin_extension() -> &'static str {
    unimplemented!("Plugin not supported")
}

#[cfg(unix)]
mod libloader {
    pub use libloading::os::unix::{Library, Symbol};
    pub use libloading::Error;

    pub unsafe fn load(path: &std::path::Path) -> Result<Library, Error> {
        unsafe {
            libloading::os::unix::Library::open(
                Some(path),
                libloading::os::unix::RTLD_NOW | libloading::os::unix::RTLD_GLOBAL,
            )
        }
    }
}

#[cfg(not(unix))]
mod libloader {
    pub use libloading::{Error, Library, Symbol};

    pub unsafe fn load(path: &std::path::Path) -> Result<Library, Error> {
        unsafe { Library::new(path) }
    }
}
