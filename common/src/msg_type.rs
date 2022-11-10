use libc::c_char;
use mime::Mime;
use std::ffi::CStr;
use std::fmt::{Debug, Display};
use std::str::FromStr;
use thiserror::Error;

/// Message type
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum MsgType {
    Mime(Mime),
    Custom(String),
}

impl MsgType {
    /// Any mime
    pub fn any() -> Self {
        MsgType::Mime(mime::STAR_STAR)
    }

    /// Binary mime
    pub fn binary() -> Self {
        MsgType::Mime("application/octet-stream".parse().unwrap())
    }

    /// Create from mime
    pub fn from_mime(mime: &str) -> Result<Self, mime::FromStrError> {
        Ok(MsgType::Mime(mime.parse()?))
    }

    /// Get acceptable or not for other message type
    pub fn acceptable(&self, other: &MsgType) -> bool {
        if *self == *other {
            return true;
        }
        if *self == Self::any() {
            return true;
        }

        match (self, other) {
            (MsgType::Mime(mime), MsgType::Mime(other_mime)) => {
                if mime.type_() == other_mime.type_() {
                    mime.subtype() == mime::STAR
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    /// Convert to ffi type
    pub fn into_ffi(self) -> DcMsgType {
        DcMsgType {
            inner: Box::into_raw(Box::new(self)) as *mut _,
        }
    }

    /// # Safety
    /// `msg_type` must be valid value.
    pub unsafe fn from_ffi(msg_type: DcMsgType) -> Self {
        *Box::from_raw(msg_type.inner as *mut MsgType)
    }
}

impl Display for MsgType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MsgType::Mime(mime) => write!(f, "mime:{}", mime),
            MsgType::Custom(name) => write!(f, "custom:{}", name),
        }
    }
}

/// Conversion error from string to `MsgType`
#[derive(Debug, Error)]
pub enum MsgTypeFromStrErr {
    #[error("invalid prefix")]
    InvalidPrefix,
    #[error("{0}")]
    Mime(#[from] mime::FromStrError),
}

impl FromStr for MsgType {
    type Err = MsgTypeFromStrErr;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(s) = s.strip_prefix("mime:") {
            Ok(MsgType::from_mime(s)?)
        } else if let Some(s) = s.strip_prefix("custom:") {
            Ok(MsgType::Custom(s.to_owned()))
        } else {
            Err(MsgTypeFromStrErr::InvalidPrefix)
        }
    }
}

#[doc(hidden)]
#[repr(C)]
pub struct DcMsgTypeInner;

/// Message type
#[repr(C)]
pub struct DcMsgType {
    inner: *mut DcMsgTypeInner,
}

/// Returns false if `s` is not valid message type text.
///
/// # Safety
/// `s` must be a valid pointer.
#[no_mangle]
pub unsafe extern "C" fn dc_msg_type_new(s: *const c_char, msg_type: *mut DcMsgType) -> bool {
    let s = if let Ok(s) = CStr::from_ptr(s).to_str() {
        s
    } else {
        return false;
    };

    if let Ok(mt) = MsgType::from_str(s) {
        *msg_type = mt.into_ffi();
        true
    } else {
        false
    }
}
