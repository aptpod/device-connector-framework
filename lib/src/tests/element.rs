// #[global_allocator]
// static ALLOC: dhat::Alloc = dhat::Alloc;

use super::test_elements;
use crate::{
    element::ElementPreBuilt,
    loader::PluginLoader,
    log::{dc_log_init, DcLogLevel},
};

#[test]
fn element() {
    dc_log_init(DcLogLevel::Trace);

    let mut plugin_loader = PluginLoader::default();
    plugin_loader.append_fn("test", test_elements::dc_plugin_init_test);
    let (loaded_elements, _libs) = plugin_loader.load();

    // let _profiler = dhat::Profiler::builder().testing().build();
    {
        let pre_build = ElementPreBuilt::new(
            loaded_elements.get("test-src").unwrap(),
            "{\"repeat\": 100}".into(),
        );
        let _element_built = pre_build.build().unwrap();
    }

    // memory leak check
    // let stats = dhat::HeapStats::get();
    // eprintln!("stats.total_bytes = {}", stats.total_bytes);
    // eprintln!("stats.curr_bytes = {}", stats.curr_bytes);
    // dhat::assert_eq!(stats.curr_blocks, 0);
    // dhat::assert_eq!(stats.curr_bytes, 0);
}
