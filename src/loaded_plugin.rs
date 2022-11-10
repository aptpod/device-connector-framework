use crate::{conf::PluginConf, ElementBank};
use anyhow::{bail, Result};
use common::DcPlugin;
use libloading::Library;
use std::path::{Path, PathBuf};

/// Load plugin libraries.
pub struct LoadedPlugin {
    library_list: Vec<(PathBuf, Library)>,
}

impl LoadedPlugin {
    /// Load libraries given as list.
    pub fn new<P: AsRef<Path>>(list: &[P]) -> Result<Self> {
        let mut library_list = Vec::new();

        for path in list {
            let path = path.as_ref();
            let lib = unsafe { Library::new(path)? };
            library_list.push((path.to_owned(), lib));
        }

        Ok(LoadedPlugin { library_list })
    }

    /// Load libraries from conf.
    pub fn from_conf(conf: &PluginConf) -> Result<Self> {
        let list = conf.plugin_files.clone();
        Self::new(&list)
    }

    /// Load plugin from the list to ElementBank.
    pub fn load_plugins(&self, bank: &mut ElementBank) -> Result<()> {
        for (path, lib) in &self.library_list {
            log::trace!("loading plugin from \"{}\"", path.display());

            let plugin = unsafe {
                let dc_load: libloading::Symbol<
                    unsafe extern "C" fn(plugin: *mut DcPlugin) -> bool,
                > = lib.get(b"dc_load")?;
                let mut plugin = std::mem::zeroed();
                if !dc_load(&mut plugin) {
                    bail!("dc_load() failed for \"{}\"", path.display());
                }
                plugin
            };

            for i in 0..plugin.n_element {
                let element = unsafe { *plugin.elements.add(i) };
                bank.append_plugin(element)?;
            }
        }
        Ok(())
    }
}
