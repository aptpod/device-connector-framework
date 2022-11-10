//! Process handlings

use crate::conf::{BgProcessConf, BgProcessWaitSignal};
use anyhow::Result;
use crossbeam_channel::Receiver;
use std::process::{Command, Stdio};

/// Creates `Command` from a string
pub fn command(s: &str) -> Command {
    let mut cmd = Command::new("/bin/sh");
    cmd.arg("-c");
    cmd.arg(s);
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::inherit());
    cmd.stderr(Stdio::inherit());
    cmd
}

/// Starts background processes
pub fn start_bg_processes(bg_processes: &[BgProcessConf]) -> Result<()> {
    log::info!("start background processes");

    for bg_process in bg_processes {
        let waiter = if let Some(signal) = bg_process.wait_signal {
            Some(signal_waiter(signal)?)
        } else {
            None
        };

        let self_pid = format!("{}", std::process::id());
        command(&bg_process.command)
            .env("DEVICE_CONNECTOR_PID", self_pid)
            .spawn()?;

        if let Some(waiter) = waiter {
            waiter.recv()?;
            log::trace!("received signal from background process");
        }
    }

    Ok(())
}

/// Executes before script
pub fn exec_before_script(script: &[String]) -> Result<()> {
    log::info!("execute before script");
    exec_script_lines(script)
}

/// Executes script lines
pub(crate) fn exec_script_lines(script: &[String]) -> Result<()> {
    for s in script {
        command(s).spawn()?.wait()?;
    }

    Ok(())
}

/// Registers after script
pub fn register_after_script(script: &[String]) {
    *crate::finalizer::AFTER_SCRIPT.lock().unwrap() = script.to_vec();
}

/// Creates signal waiter
pub fn signal_waiter(signal: BgProcessWaitSignal) -> Result<Receiver<()>> {
    let signal = match signal {
        BgProcessWaitSignal::Sigusr1 => signal_hook::consts::SIGUSR1,
        BgProcessWaitSignal::Sigusr2 => signal_hook::consts::SIGUSR2,
    };

    let (sender, receiver) = crossbeam_channel::unbounded();
    let mut signals = signal_hook::iterator::Signals::new(&[signal])?;

    std::thread::spawn(move || {
        for _signal in signals.forever() {
            if sender.send(()).is_err() {
                break;
            }
        }
    });

    Ok(receiver)
}
