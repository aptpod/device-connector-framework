use std::{
    collections::HashMap,
    ffi::{c_char, CStr},
};

use crate::{loader::LoadedElements, task::with_task_context};

pub type DcMetadataId = u32;

pub const META_DATA_SIZE: usize = std::mem::size_of::<DcMetadata>();

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum DcMetadataType {
    Empty = 0,
    Int64 = 1,
    Float64 = 2,
    Duration = 3,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct DcMetadata {
    pub id: DcMetadataId,
    pub r#type: DcMetadataType,
    pub value: DcMetadataValue,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union DcMetadataValue {
    pub int64: i64,
    pub float64: f64,
    pub duration: DcDuration,
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DcDuration {
    pub secs: u64,
    pub nsecs: u32,
}

impl DcMetadata {
    pub fn as_array(&self) -> [u8; META_DATA_SIZE] {
        unsafe { std::mem::transmute(*self) }
    }
}

pub(crate) struct MetadataIdList {
    string_ids: Vec<String>,
    map: HashMap<String, DcMetadataId>,
}

impl Default for MetadataIdList {
    fn default() -> Self {
        Self {
            string_ids: vec!["!RESERVED!".into()],
            map: HashMap::default(),
        }
    }
}

impl MetadataIdList {
    pub fn new(loaded_elements: &LoadedElements) -> Self {
        let mut list = Self::default();

        for e in loaded_elements.values() {
            for string_id in &e.element.metadata_ids {
                list.append(string_id);
            }
        }

        list
    }

    fn append(&mut self, string_id: &str) {
        if self.map.contains_key(string_id) {
            return;
        }

        let id: DcMetadataId = self
            .string_ids
            .len()
            .try_into()
            .expect("MetadataId overflow");
        self.string_ids.push(string_id.into());
        self.map.insert(string_id.into(), id);
    }

    pub fn id(&self, string_id: &str) -> Option<DcMetadataId> {
        self.map.get(string_id).copied()
    }
}

/// Get a metadata id from given string.
/// Return zero if given string is invalid or unknown. If this function is called from out of task threads, returns zero also.
#[no_mangle]
pub unsafe extern "C" fn dc_metadata_get_id(string_id: *const c_char) -> DcMetadataId {
    let id = match unsafe { CStr::from_ptr(string_id) }.to_str() {
        Ok(id) => id,
        Err(e) => {
            core_log!(Error, "invalid metadata id: {}", e);
            return 0;
        }
    };

    if let Some(Some(id)) = with_task_context(|task_context| task_context.metadata_id_list.id(id)) {
        id
    } else {
        0
    }
}
