use std::{ffi::CString, num::NonZeroU32, time::Duration};

use sys::{DcMetadata, DcMetadataId, DcMetadataType, DcMetadataValue};

/// Metadata id.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct MetadataId(NonZeroU32);

/// Metadata.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Metadata {
    pub id: MetadataId,
    pub value: MetadataValue,
}

/// Metadata value.
#[derive(Clone, Copy, PartialEq, Debug)]
#[non_exhaustive]
pub enum MetadataValue {
    Empty,
    Int64(i64),
    Float64(f64),
    Duration(Duration),
}

impl MetadataId {
    /// Get a metadata id from given string.
    /// Return `None` if `string_id` is invalid or unknown. If this function is called from out of task threads, returns `None` also.
    pub fn new(string_id: &str) -> Option<MetadataId> {
        let cstr = CString::new(string_id).ok()?;

        let id = unsafe { sys::dc_metadata_get_id(cstr.as_ptr()) };

        NonZeroU32::try_from(id).ok().map(MetadataId)
    }

    /// Create `MetadataId` from `DcMetadataId`. Return `None` if `id` is zero.
    pub fn from_raw(id: DcMetadataId) -> Option<MetadataId> {
        NonZeroU32::try_from(id).ok().map(MetadataId)
    }

    /// Convert to `DcMetadataId`.
    pub fn into_raw(self) -> DcMetadataId {
        self.0.get()
    }
}

impl Metadata {
    /// Create `Metadata` from `DcMetadata`
    ///
    /// # Safety
    /// Given `metadata` must be valid.
    pub unsafe fn from_raw(metadata: DcMetadata) -> Metadata {
        Metadata {
            id: MetadataId::from_raw(metadata.id).expect("Invalid DcMetadataId"),
            value: unsafe { MetadataValue::from_raw(metadata.type_, metadata.value) },
        }
    }

    /// Convert to `DcMetadata`
    pub fn into_raw(self) -> DcMetadata {
        let (type_, value) = self.value.into_raw();
        DcMetadata {
            id: self.id.0.get(),
            type_,
            value,
        }
    }
}

impl MetadataValue {
    /// Create `MetadataValue` from `DcMetadataType` and `DcMetadataValue`
    ///
    /// # Safety
    /// Given values must be valid.
    pub unsafe fn from_raw(type_: DcMetadataType, value: DcMetadataValue) -> MetadataValue {
        unsafe {
            match type_ {
                sys::DcMetadataType_Empty => MetadataValue::Empty,
                sys::DcMetadataType_Int64 => MetadataValue::Int64(value.int64),
                sys::DcMetadataType_Float64 => MetadataValue::Float64(value.float64),
                sys::DcMetadataType_Duration => {
                    let duration = Duration::new(value.duration.secs, value.duration.nsecs);
                    MetadataValue::Duration(duration)
                }
                _ => panic!("Invalid DcMetadataType"),
            }
        }
    }

    /// Convert to `DcMetadataType` and `DcMetadataValue`
    pub fn into_raw(self) -> (DcMetadataType, DcMetadataValue) {
        unsafe {
            let mut value: DcMetadataValue = std::mem::zeroed();
            match self {
                MetadataValue::Empty => (sys::DcMetadataType_Empty, value),
                MetadataValue::Int64(i) => {
                    value.int64 = i;
                    (sys::DcMetadataType_Int64, value)
                }
                MetadataValue::Float64(f) => {
                    value.float64 = f;
                    (sys::DcMetadataType_Float64, value)
                }
                MetadataValue::Duration(d) => {
                    value.duration.secs = d.as_secs();
                    value.duration.nsecs = d.subsec_nanos();
                    (sys::DcMetadataType_Duration, value)
                }
            }
        }
    }
}
