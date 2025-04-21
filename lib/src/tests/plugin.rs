// #[global_allocator]
// static ALLOC: dhat::Alloc = dhat::Alloc;

use std::ffi::CString;

use crate::{
    conf::Conf,
    log::{dc_log_init, DcLogLevel},
    plugin::*,
    runner::*,
};

use super::test_elements;

#[test]
fn plugin() {
    dc_log_init(DcLogLevel::Trace);

    let _profiler = dhat::Profiler::builder().testing().build();

    run(TEST_CONFIG1);
    run(TEST_CONFIG2);
    run(TEST_CONFIG3);

    let stats = dhat::HeapStats::get();
    eprintln!("stats.curr_bytes = {}", stats.curr_bytes);
    eprintln!("stats.total_bytes = {}", stats.total_bytes);
}

const TEST_CONFIG1: &str = r#"
tasks:
  - id: src1
    element: test-src
    conf:
      repeat: 100

  - id: sink1
    element: test-sink
    from: [ [src1] ]
"#;

const TEST_CONFIG2: &str = r#"
tasks:
  - id: src1
    element: test-src
    conf:
      repeat: 50

  - id: src2
    element: test-src
    conf:
      repeat: 50

  - id: filter1
    element: test-filter
    from: [ [src2] ]

  - id: sink1
    element: test-sink
    from: [ [src1, filter1] ]
"#;

const TEST_CONFIG3: &str = r#"
tasks:
  - id: src1
    element: test-src
    conf:
      repeat: 50

  - id: sink1
    element: test-sink
    from: [ [src1] ]

  - id: sink2
    element: test-sink
    from: [ [src1] ]
"#;

fn run(conf: &str) {
    let conf: Conf = serde_yaml::from_str(conf).unwrap();
    let conf = CString::new(serde_json::to_string(&conf).unwrap()).unwrap();

    unsafe {
        let runner = crate::runner::dc_runner_new();
        dc_runner_set_config(runner, conf.as_ptr());
        dc_runner_append_plugin_init(
            runner,
            "test\0".as_ptr() as _,
            test_elements::dc_plugin_init_test,
        );
        dc_runner_append_plugin_init(
            runner,
            "must-fail\0".as_ptr() as _,
            test_dc_plugin_will_fail,
        );
        dc_runner_run(runner);
    }
}

unsafe extern "C-unwind" fn test_dc_plugin_will_fail(_plugin: *mut DcPlugin) -> bool {
    false
}
