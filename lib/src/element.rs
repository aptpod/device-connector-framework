use anyhow::{bail, Error};
use libc::c_void;
use semver::Version;
use std::ffi::CString;

use crate::{
    loader::{ElementOrigin, LoadedElement},
    msg_receiver::{DcMsgReceiver, MsgReceiver},
    pipeline::Pipeline,
    plugin::{DcElementResult, DcFinalizer, DcPipeline, Element},
};

#[derive(Debug)]
pub struct ElementExecutable {
    element: &'static Element,
    p: *mut c_void,
    origin: ElementOrigin,
}

impl ElementExecutable {
    pub fn next(
        &mut self,
        pipeline: &mut Pipeline,
        msg_receiver: &mut MsgReceiver,
    ) -> DcElementResult {
        let pipeline = pipeline as *mut _ as *mut DcPipeline;
        let msg_receiver = msg_receiver as *mut _ as *mut DcMsgReceiver;

        unsafe { (self.element.next)(self.p, pipeline, msg_receiver) }
    }
}

impl Drop for ElementExecutable {
    fn drop(&mut self) {
        unsafe { (self.element.free)(self.p) };
    }
}

impl std::fmt::Display for ElementExecutable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.element.name, self.origin)?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct ElementPreBuilt {
    pub element: &'static Element,
    pub framework_version: Version,
    pub origin: ElementOrigin,
    pub conf: String,
}

impl ElementPreBuilt {
    pub fn new(loaded_element: &LoadedElement, conf: String) -> Self {
        ElementPreBuilt {
            element: loaded_element.element,
            framework_version: loaded_element.framework_version.clone(),
            origin: loaded_element.origin.clone(),
            conf,
        }
    }

    pub fn build(self) -> Result<(ElementExecutable, DcFinalizer), Error> {
        let conf = CString::new(self.conf)?;

        let p = unsafe { (self.element.new)(conf.as_ptr()) };

        let finalizer = if let Some(f) = self.element.finalizer_creator {
            let mut finalizer = DcFinalizer::default();
            if !unsafe { f(p, &mut finalizer) } {
                bail!("Finalizer creator failed.");
            }
            finalizer
        } else {
            DcFinalizer::default()
        };

        if p.is_null() {
            bail!("{} element new() failed", self.element.name);
        }

        Ok((
            ElementExecutable {
                element: self.element,
                p,
                origin: self.origin,
            },
            finalizer,
        ))
    }
}
