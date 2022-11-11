use anyhow::Result;

use device_connector::conf::Conf;
use device_connector::{ElementBank, LoadedPlugin, RunnerBuilder};

use std::thread;
use std::time::Duration;

const CONFIG: &str = r#"
runner:
  channel_capacity: 16
  
task:
  - id: 1
    element: text-src
    conf:
      text: "hello, world!"
      interval_ms: 100
    
  - id: 2
    element: stat-filter
    from:
      - - 1
    conf:
      interval_ms: 1000
      
  - id: 3
    element: stdout-sink
    from:
      - - 2
    conf:
      separator: "\n"
"#;

#[test]
fn run_hello() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .init();
    let conf = Conf::from_yaml(CONFIG)?;
    let mut bank = ElementBank::new();
    let loaded_plugin = LoadedPlugin::from_conf(&conf.plugin)?;
    loaded_plugin.load_plugins(&mut bank)?;

    let mut runner_builder = RunnerBuilder::new(&bank, &loaded_plugin, &conf);
    runner_builder.append_from_conf(&conf.tasks)?;
    let runner = runner_builder.build()?;

    thread::spawn(|| {
        thread::sleep(Duration::from_millis(1000));
        std::process::exit(0);
    });

    runner.run()?;

    Ok(())
}
