//! Element definitions

use crate::error::{ElementAppendError, ElementBuildError, UnknownElementError};
use crate::ElementConf;
use common::{DcElement, DcMsgReceiver, DcPipeline, MsgReceiver, Pipeline};
pub use common::{ElementResult, ElementValue, MsgType, Port};
use serde::de::DeserializeOwned;
use std::collections::HashMap;

pub(crate) type ElementNextBoxed =
    Box<dyn FnMut(*mut DcPipeline, *mut DcMsgReceiver) -> ElementResult + Send>;
pub type ElementFinalizer = Box<dyn FnOnce() -> Result<(), crate::error::Error> + Send>;

/// Prepared element to execute
pub struct ElementExecutable {
    pub(crate) next_boxed: ElementNextBoxed,
    pub(crate) finalizer: Option<ElementFinalizer>,
}

/// Opaque type to build element
#[derive(Clone)]
pub enum ElementBuilder {
    Native(fn(ElementConf) -> Result<ElementExecutable, crate::error::Error>),
    Plugin(DcElement),
}

impl ElementBuilder {
    pub(crate) fn build(
        &self,
        conf: ElementConf,
    ) -> Result<ElementExecutable, crate::error::Error> {
        match self {
            ElementBuilder::Native(build_executable) => build_executable(conf),
            ElementBuilder::Plugin(element) => crate::plugin::build_plugin_element(*element, &conf),
        }
    }
}

/// Unit of task execution.
pub struct ElementPreBuild {
    conf: ElementConf,
    builder: ElementBuilder,
}

impl ElementPreBuild {
    pub(crate) fn build(&mut self) -> Result<ElementExecutable, crate::error::Error> {
        self.builder.build(self.conf.clone())
    }
}

/// Buildable element from config.
pub trait ElementBuildable: Sized + Send + 'static {
    /// Configuration type for this element
    type Config: DeserializeOwned;

    /// Name of this element. Must be unique in elements
    const NAME: &'static str;

    /// The number of receiving ports
    const RECV_PORTS: Port = 0;

    /// The number of sending ports
    const SEND_PORTS: Port = 0;

    /// Returns acceptable message type of this element
    fn acceptable_msg_types() -> Vec<Vec<MsgType>> {
        Vec::new()
    }

    /// Create element from config
    fn new(conf: Self::Config) -> Result<Self, crate::error::Error>;

    /// Get message from `receiver` and returns the result of this element.
    fn next(&mut self, pipeline: &mut Pipeline, receiver: &mut MsgReceiver) -> ElementResult;

    /// Returns the finalizer of this element
    fn finalizer(&mut self) -> Result<Option<ElementFinalizer>, crate::error::Error> {
        Ok(None)
    }

    #[doc(hidden)]
    fn element_conf_to_executable(
        conf: ElementConf,
    ) -> Result<ElementExecutable, crate::error::Error> {
        let conf: Self::Config = conf.to_conf()?;
        let mut element = Self::new(conf)?;
        let finalizer = element.finalizer()?;
        let next_boxed: ElementNextBoxed = Box::new(move |pipeline, recv| {
            let mut pipeline = unsafe { Pipeline::new(pipeline) };
            let mut recv = unsafe { MsgReceiver::new(recv) };
            element.next(&mut pipeline, &mut recv)
        });
        Ok(ElementExecutable {
            next_boxed,
            finalizer,
        })
    }

    #[doc(hidden)]
    fn builder() -> ElementBuilder {
        ElementBuilder::Native(Self::element_conf_to_executable)
    }
}

type RecvMsgTypeGetter = Box<dyn Fn() -> Vec<Vec<MsgType>>>;

/// Holds buildable element list.
pub struct ElementBank {
    builders: HashMap<String, ElementBuilder>,
    acceptable_msg_types: HashMap<String, RecvMsgTypeGetter>,
    ports: HashMap<String, (Port, Port)>,
}

impl Default for ElementBank {
    fn default() -> Self {
        ElementBank::new()
    }
}

impl ElementBank {
    /// New empty ElementBank.
    pub fn empty() -> Self {
        ElementBank {
            builders: HashMap::default(),
            acceptable_msg_types: HashMap::default(),
            ports: HashMap::default(),
        }
    }

    /// Append element.
    pub(crate) fn append(
        &mut self,
        name: &str,
        builder: ElementBuilder,
        acceptable_msg_types: RecvMsgTypeGetter,
        ports: (Port, Port),
    ) -> Result<(), ElementAppendError> {
        if self.builders.contains_key(name) {
            return Err(ElementAppendError(format!(
                "element name duplication detected for \"{}\"",
                name
            )));
        }

        self.builders.insert(name.to_owned(), builder);
        self.acceptable_msg_types
            .insert(name.to_owned(), acceptable_msg_types);
        self.ports.insert(name.to_owned(), ports);

        log::trace!("append \"{}\" to element bank", name);
        Ok(())
    }

    /// Register buildable element.
    pub fn append_from_buildable<E: ElementBuildable + 'static>(
        &mut self,
    ) -> Result<(), ElementAppendError> {
        self.append(
            E::NAME,
            ElementBuilder::Native(E::element_conf_to_executable),
            Box::new(E::acceptable_msg_types),
            (E::RECV_PORTS, E::SEND_PORTS),
        )
    }

    /// get pre build element by name and config.
    pub fn pre_build(
        &self,
        name: &str,
        conf: ElementConf,
    ) -> Result<ElementPreBuild, ElementBuildError> {
        if let Some(builder) = self.builders.get(name) {
            Ok(ElementPreBuild {
                conf,
                builder: builder.clone(),
            })
        } else {
            Err(ElementBuildError::UnknownElement(UnknownElementError(
                name.into(),
            )))
        }
    }

    /// Get acceptable message types.
    pub fn acceptable_msg_types(
        &self,
        name: &str,
    ) -> Result<Vec<Vec<MsgType>>, UnknownElementError> {
        self.acceptable_msg_types
            .get(name)
            .map(|f| f())
            .ok_or_else(|| UnknownElementError(name.into()))
    }

    /// Get the number of ports of element.
    pub fn ports(&self, name: &str) -> Result<(Port, Port), UnknownElementError> {
        self.ports
            .get(name)
            .copied()
            .ok_or_else(|| UnknownElementError(name.into()))
    }

    /// New ElementBank with core elements.
    pub fn new() -> Self {
        let mut bank = Self::empty();

        crate::base::append_to_bank(&mut bank);

        bank
    }
}
