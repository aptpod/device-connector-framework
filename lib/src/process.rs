//! Process handlings

use crate::conf::{BgProcessConf, BgProcessWaitSignal};
use anyhow::{bail, Result};
use crossbeam::channel::Receiver;
use std::process::{Command, Stdio};

/// Starts background processes
pub fn start_bg_processes(bg_processes: &[BgProcessConf]) -> Result<()> {
    core_log!(Info, "start background processes");

    for bg_process in bg_processes {
        let waiter = if let Some(signal) = bg_process.wait_signal {
            Some(signal_waiter(signal)?)
        } else {
            None
        };

        let self_pid = format!("{}", std::process::id());
        command(&bg_process.command)
            .env("DC_RUNNER_PID", self_pid)
            .spawn()?;

        if let Some(waiter) = waiter {
            waiter.recv()?;
            core_log!(Trace, "received signal from background process");
        }
    }

    Ok(())
}

/// Executes before task
pub fn exec_before_task(script: &[String]) -> Result<()> {
    if script.is_empty() {
        return Ok(());
    }
    core_log!(Info, "execute before script");
    exec_script_lines(script)
}

/// Executes script lines
pub(crate) fn exec_script_lines(script: &[String]) -> Result<()> {
    for s in script {
        let result = command(s).spawn()?.wait()?;

        if !result.success() {
            if let Some(code) = result.code() {
                bail!("Command \'{}\' failed. result code = {}", s, code);
            } else {
                bail!("Command \'{}\' failed", s);
            }
        }
    }

    Ok(())
}

/// Creates `Command` from a string
#[cfg(unix)]
pub fn command(s: &str) -> Command {
    let mut cmd = Command::new("/bin/sh");
    cmd.arg("-c");
    cmd.arg(s);
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::inherit());
    cmd.stderr(Stdio::inherit());
    cmd
}

#[cfg(windows)]
pub fn command(s: &str) -> Command {
    let mut cmd = Command::new("cmd");
    cmd.arg("/C");
    cmd.arg(s);
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::inherit());
    cmd.stderr(Stdio::inherit());
    cmd
}

#[cfg(not(any(unix, windows)))]
pub fn command(s: &str) -> Command {
    unimplemented!("Command not supported")
}

/// Creates signal waiter
#[cfg(unix)]
pub fn signal_waiter(signal: BgProcessWaitSignal) -> Result<Receiver<()>> {
    let signal = match signal {
        BgProcessWaitSignal::Sigusr1 => signal_hook::consts::SIGUSR1,
        BgProcessWaitSignal::Sigusr2 => signal_hook::consts::SIGUSR2,
    };

    let (sender, receiver) = crossbeam::channel::unbounded();
    let mut signals = signal_hook::iterator::Signals::new([signal])?;

    std::thread::spawn(move || {
        for _signal in signals.forever() {
            if sender.send(()).is_err() {
                break;
            }
        }
    });

    Ok(receiver)
}

#[cfg(not(unix))]
pub fn signal_waiter(_signal: BgProcessWaitSignal) -> Result<Receiver<()>> {
    unimplemented!("Singal waiting not supported in this platform")
}
