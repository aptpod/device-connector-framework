mod env_replacer;
mod show;

use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use clap::Parser;

use dc_core as dc;

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

#[derive(Parser)]
#[command(version, about)]
pub enum Command {
    /// Run with a given configuration
    Run {
        /// Config file path
        config: PathBuf,
        /// Regex string using for replacement by enviroment variable in the configuration.
        #[clap(long)]
        env_replacer: Option<regex::Regex>,
        #[clap(long)]
        log_level: Option<LogLevel>,
    },
    /// List loaded elements
    List {
        #[clap(long)]
        log_level: Option<LogLevel>,
    },
    /// Show an element infomation
    Show {
        /// Target element id
        element: String,
        #[clap(long)]
        /// Print markdown string
        markdown: bool,
        #[clap(long)]
        log_level: Option<LogLevel>,
    },
}

impl Command {
    fn log_level(&self) -> &Option<LogLevel> {
        match self {
            Command::Run { log_level, .. } => log_level,
            Command::List { log_level, .. } => log_level,
            Command::Show { log_level, .. } => log_level,
        }
    }
}

const ENV_LOG_LEVEL: &str = "DC_LOG";
const ENV_PLUGIN_DIRS: &str = "DC_PLUGIN_PATH";

fn main() -> Result<()> {
    let args = Command::parse();

    let env_var_level = if let Ok(level) = std::env::var(ENV_LOG_LEVEL) {
        match level.as_str() {
            "error" => Some(LogLevel::Error),
            "warn" => Some(LogLevel::Warn),
            "info" => Some(LogLevel::Info),
            "debug" => Some(LogLevel::Debug),
            "trace" => Some(LogLevel::Trace),
            _ => None,
        }
    } else {
        None
    };

    let log_level = env_var_level
        .or(*args.log_level())
        .unwrap_or(LogLevel::Info);

    let log_level = match log_level {
        LogLevel::Error => dc::LogLevel::Error,
        LogLevel::Warn => dc::LogLevel::Warn,
        LogLevel::Info => dc::LogLevel::Info,
        LogLevel::Debug => dc::LogLevel::Debug,
        LogLevel::Trace => dc::LogLevel::Trace,
    };

    dc::init_log("runner", Some(log_level));

    let mut runner = dc::RunnerBuilder::new();

    if let Some(dir) = default_plugin_dir() {
        if dir.is_dir() {
            log::debug!("append a default plugin directory {}", dir.display());
            runner = runner.append_dir(dir)?;
        }
    }

    if let Ok(path) = std::env::var(ENV_PLUGIN_DIRS) {
        for dir in path.split(':') {
            runner = runner.append_dir(dir)?;
        }
    }

    match args {
        Command::Run {
            config,
            env_replacer,
            ..
        } => {
            let config = std::fs::read_to_string(&config)
                .with_context(|| format!("Reading {} failed", config.display()))?;
            let config = if let Some(env_replacer) = env_replacer {
                env_replacer::env_replace(config, env_replacer)
            } else {
                config
            };
            runner.config(config)?.run()?;
        }
        Command::List { .. } => {
            let list = runner.element_info_list();
            for element in list {
                println!("{}", element.id);
            }
        }
        Command::Show {
            element, markdown, ..
        } => {
            let list = runner.element_info_list();
            if let Some(info) = list.iter().find(|info| info.id == element) {
                show::show_element_info(info, markdown);
            } else {
                bail!("Unknown element \"{}\"", element);
            }
        }
    }

    Ok(())
}

#[cfg(unix)]
fn default_plugin_dir() -> Option<PathBuf> {
    if std::env::consts::OS == "linux" {
        let path = if std::env::consts::ARCH == "arm" {
            PathBuf::from("/usr/lib/arm-linux-gnueabihf/dc-plugins")
        } else {
            PathBuf::from(format!(
                "/usr/lib/{}-linux-gnu/dc-plugins",
                std::env::consts::ARCH
            ))
        };

        if path.exists() && path.is_dir() {
            return Some(path);
        }
    }

    let path = PathBuf::from("/usr/lib/dc-plugins");
    if path.exists() && path.is_dir() {
        return Some(path);
    }

    log::warn!("default plugin directory not found");

    None
}

#[cfg(not(unix))]
fn default_plugin_dir() -> Option<PathBuf> {
    None
}
