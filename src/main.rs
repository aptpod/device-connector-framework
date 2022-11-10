use anyhow::Result;
use clap::Parser;
use env_logger::Builder;
use std::path::PathBuf;

use device_connector::conf::Conf;
use device_connector::{ElementBank, LoadedPlugin, RunnerBuilder};

#[derive(Parser)]
struct Args {
    #[clap(short, long)]
    config: PathBuf,
}

fn main() -> Result<()> {
    if std::env::var("RUST_LOG").is_ok() {
        Builder::from_env("RUST_LOG").init();
    } else {
        Builder::new().filter_level(log::LevelFilter::Info).init();
    };

    let args = Args::parse();
    let conf = Conf::read_from_file(&args.config)?;
    let mut bank = ElementBank::new();
    let loaded_plugin = LoadedPlugin::from_conf(&conf.plugin)?;
    loaded_plugin.load_plugins(&mut bank)?;

    // Start bg processes
    device_connector::process::start_bg_processes(&conf.bg_processes)?;

    // Execute before script and blocks until completion
    device_connector::process::exec_before_script(&conf.before_task)?;

    // Regester after script
    device_connector::process::register_after_script(&conf.after_task);

    // Build runner
    let mut runner_builder = RunnerBuilder::new(&bank, &loaded_plugin, &conf);
    runner_builder.append_from_conf(&conf.tasks)?;
    let runner = runner_builder.build()?;

    runner.run()?;

    Ok(())
}
