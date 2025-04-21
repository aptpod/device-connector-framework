#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

use crate::metadata::*;
use crate::msg::*;
use crate::msg_buf::*;

struct Case {
    data: &'static [u8],
    metadata: &'static [DcMetadata],
    empty_metadata: usize,
}

impl Case {
    unsafe fn test(&self) {
        unsafe {
            let msg_buf = dc_msg_buf_new();

            dc_msg_buf_write(msg_buf, self.data.as_ptr(), self.data.len());
            for metadata in self.metadata {
                dc_msg_buf_set_metadata(msg_buf, *metadata);
            }
            set_metadata_padding(self.empty_metadata);
            let mut msg = dc_msg_buf_take_msg(msg_buf);

            let msg_cloned = dc_msg_clone(&msg);
            assert_eq!(msg, msg_cloned);

            let check_for_dc_msg = |msg: &DcMsg| {
                let mut data = std::ptr::null();
                let mut len = 0;
                dc_msg_get_data(msg, &mut data, &mut len);
                let data_slice = std::slice::from_raw_parts(data, len);
                assert_eq!(data_slice, self.data);

                for want in self.metadata {
                    assert_metadata_eq(*want, dc_msg_get_metadata(msg, want.id));
                }

                let invalid_metadata = dc_msg_get_metadata(msg, 0xFFFFFFFF);
                assert_eq!(invalid_metadata.r#type, DcMetadataType::Empty);
            };

            check_for_dc_msg(&msg);
            check_for_dc_msg(&msg_cloned);

            let want = DcMetadata {
                id: 0xFF,
                r#type: DcMetadataType::Int64,
                value: DcMetadataValue { int64: 42 },
            };
            dc_msg_set_metadata(&mut msg, want);
            let metadata = dc_msg_get_metadata(&msg, want.id);
            assert_metadata_eq(want, metadata);

            assert_ne!(msg, msg_cloned);

            check_for_dc_msg(&msg);
            check_for_dc_msg(&msg_cloned);

            dc_msg_free(msg);
            dc_msg_free(msg_cloned);

            dc_msg_buf_write(msg_buf, self.data.as_ptr(), self.data.len());
            for metadata in self.metadata {
                dc_msg_buf_set_metadata(msg_buf, *metadata);
            }
            set_metadata_padding(self.empty_metadata);
            let mut msg = dc_msg_buf_take_msg(msg_buf);

            let want = DcMetadata {
                id: 0xFF,
                r#type: DcMetadataType::Int64,
                value: DcMetadataValue { int64: 42 },
            };
            let old_msg = msg;
            dc_msg_set_metadata(&mut msg, want);
            let metadata = dc_msg_get_metadata(&msg, want.id);
            assert_metadata_eq(want, metadata);

            if self.empty_metadata == 0 {
                assert_ne!(msg, old_msg);
            } else {
                assert_eq!(msg, old_msg);
            }

            dc_msg_free(msg);

            dc_msg_buf_free(msg_buf);
        }
    }
}

fn create_cases() -> Vec<Case> {
    let case_data: &[&[u8]] = &[&[], &[1; 10], &[1; 256]];
    let case_metadata: &[&[DcMetadata]] = &[
        &[],
        &[DcMetadata {
            id: 1,
            r#type: DcMetadataType::Int64,
            value: DcMetadataValue { int64: 10 },
        }],
        &[
            DcMetadata {
                id: 1,
                r#type: DcMetadataType::Int64,
                value: DcMetadataValue { int64: 10 },
            },
            DcMetadata {
                id: 2,
                r#type: DcMetadataType::Float64,
                value: DcMetadataValue { float64: 10.0 },
            },
        ],
    ];
    let case_empty_metadata: &[usize] = &[0, 1, 4];

    let mut cases = Vec::new();
    for i in 0..case_data.len() {
        for j in 0..case_metadata.len() {
            for k in 0..case_empty_metadata.len() {
                cases.push(Case {
                    data: case_data[i],
                    metadata: case_metadata[j],
                    empty_metadata: case_empty_metadata[k],
                });
            }
        }
    }
    cases
}

#[test]
fn msg_memory() {
    let cases = create_cases();

    let _profiler = dhat::Profiler::builder().testing().build();

    for case in cases {
        unsafe {
            case.test();
        }
    }

    // memory leak check
    let stats = dhat::HeapStats::get();
    eprintln!("stats.total_bytes = {}", stats.total_bytes);
    dhat::assert_eq!(stats.curr_blocks, 0);
    dhat::assert_eq!(stats.curr_bytes, 0);
}

fn assert_metadata_eq(a: DcMetadata, b: DcMetadata) {
    assert_eq!(a.id, b.id);
    assert_eq!(a.r#type, b.r#type);

    unsafe {
        match a.r#type {
            DcMetadataType::Empty => (),
            DcMetadataType::Int64 => assert_eq!(a.value.int64, b.value.int64),
            DcMetadataType::Float64 => assert_eq!(a.value.float64, b.value.float64),
            DcMetadataType::Duration => assert_eq!(a.value.duration, b.value.duration),
        }
    }
}
