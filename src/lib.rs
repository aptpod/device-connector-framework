/// Configuration type definitions.
#[path = "../common/src/conf.rs"]
pub mod conf;

#[path = "../common/src/msg_type.rs"]
pub mod msg_type;

/// Element definitions.
pub mod element;

mod log;
mod metadata;
mod msg;
mod msg_buf;
mod msg_receiver;
mod pipeline;
mod runner;

pub use conf::Port;
pub use element::*;
pub use log::*;
pub use metadata::*;
pub use msg::*;
pub use msg_buf::*;
pub use msg_receiver::*;
pub use msg_type::*;
pub use pipeline::*;
pub use runner::*;

pub use sys;

#[doc(hidden)]
pub use ::log as _log;

#[allow(clippy::missing_safety_doc)]
/// A trait to represent a plugin.
pub unsafe trait Plugin {
    /// Plugin name.
    const NAME: &'static str;
    /// Plugin authors.
    const AUTHORS: &'static str;

    /// Initialize the plugin.
    ///
    /// # Safety
    /// Given `plugin` must be valid.
    ///
    /// # Panics
    /// This function may panic if the plugin includes invalid elements.
    unsafe fn init(plugin: *mut sys::DcPlugin);
}

/// Define a plugin from elements.
#[macro_export]
macro_rules! define_plugin {
    ($($element:path),* $(,)?) => {
        $crate::define_plugin!(env!("CARGO_CRATE_NAME"); $($element),*);
    };
    ($name:expr; $($element:path),* $(,)?) => {
        pub struct Plugin;

        unsafe impl $crate::Plugin for Plugin {
            const NAME: &'static str = $name;
            const AUTHORS: &'static str = env!("CARGO_PKG_AUTHORS");

            unsafe fn init(plugin: *mut $crate::sys::DcPlugin) {
                $crate::sys::dc_plugin_set_version(plugin, "3.0.0\0".as_ptr() as *const _);

                let name = ::std::ffi::CString::new(Self::NAME).unwrap();
                $crate::sys::dc_plugin_set_name(plugin, name.as_ptr());

                let authors = ::std::ffi::CString::new(Self::AUTHORS).unwrap();
                $crate::sys::dc_plugin_set_authors(plugin, authors.as_ptr());

                $crate::init_log($name, None);

                $(
                    $crate::element::register_element_to_plugin::<$element>(plugin);
                )*
            }
        }

        #[no_mangle]
        pub unsafe extern "C" fn dc_plugin_init(dc_plugin: *mut $crate::sys::DcPlugin) -> bool {
            let result = std::panic::catch_unwind(|| {
                <Plugin as $crate::Plugin>::init(dc_plugin);
            });
            if let Err(e) = result {
                let e = e.downcast_ref::<&str>().unwrap_or(&"panic");
                $crate::_log::error!("plugin {} initialization failed: {:?}", <Plugin as $crate::Plugin>::NAME, e);
                false
            } else {
                true
            }
        }
    }
}
