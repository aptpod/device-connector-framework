use std::fmt::{Debug, Display};
use std::str::FromStr;
use thiserror::Error;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Mime(mime::Mime);

/// Message type
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum MsgType {
    Any,
    Mime(Mime),
    Custom(String),
}

impl MsgType {
    /// Any mime
    pub fn any() -> Self {
        MsgType::Any
    }

    /// Binary mime
    pub fn binary() -> Self {
        MsgType::Mime(Mime("application/octet-stream".parse().unwrap()))
    }

    /// Create from mime
    pub fn from_mime(mime: &str) -> Result<Self, MimeParseError> {
        Ok(MsgType::Mime(Mime(mime.parse().map_err(MimeParseError)?)))
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
            (MsgType::Mime(Mime(mime)), MsgType::Mime(Mime(other_mime))) => {
                if mime.type_() == other_mime.type_() {
                    mime.subtype() == mime::STAR
                } else {
                    false
                }
            }
            _ => false,
        }
    }
}

impl Display for MsgType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MsgType::Any => write!(f, "any"),
            MsgType::Mime(mime) => write!(f, "mime:{}", mime.0),
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
    Mime(#[from] MimeParseError),
}

impl FromStr for MsgType {
    type Err = MsgTypeFromStrErr;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "any" {
            Ok(MsgType::Any)
        } else if let Some(s) = s.strip_prefix("mime:") {
            Ok(MsgType::from_mime(s)?)
        } else if let Some(s) = s.strip_prefix("custom:") {
            Ok(MsgType::Custom(s.to_owned()))
        } else {
            Err(MsgTypeFromStrErr::InvalidPrefix)
        }
    }
}

#[derive(Debug, Error)]
#[error("{0}")]
pub struct MimeParseError(mime::FromStrError);
