//! Launching Kanata as a managed child process.
//!
//! This is the "managed" backend: the overlay owns the `kanata` process, runs it
//! as the current user (see the README for the one-time `input`/`uinput` group
//! setup that makes root unnecessary), and talks to it over a localhost-only TCP
//! port.
//!
//! Every Kanata this builds gets a parent-death signal, so it can never outlive
//! the overlay and leave the keyboard grabbed (constitution III). Readiness,
//! log capture and restarts live in [`crate::supervisor`].

use std::ffi::OsString;
use std::io;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener};
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::Result;

/// How to start a managed Kanata process.
#[derive(Debug, Clone)]
pub struct LaunchOptions {
    /// Path to (or name of) the `kanata` binary.
    pub kanata_bin: PathBuf,
    /// Config file passed with `--cfg`.
    pub config: PathBuf,
    /// TCP port for Kanata's server. Always bound to 127.0.0.1: anyone who can
    /// reach the port can switch layers and press fake keys.
    pub port: u16,
    /// Extra arguments appended verbatim (e.g. `--debug`).
    pub extra_args: Vec<OsString>,
    /// Ask the kernel to SIGTERM the child when the thread that spawned it
    /// dies. Spawn from a thread that lives as long as the app (research R1).
    pub parent_death_signal: bool,
}

impl LaunchOptions {
    pub fn new(config: impl Into<PathBuf>, port: u16) -> Self {
        Self {
            kanata_bin: PathBuf::from("kanata"),
            config: config.into(),
            port,
            extra_args: Vec::new(),
            parent_death_signal: true,
        }
    }

    /// Address the TCP client should connect to.
    pub fn address(&self) -> SocketAddr {
        SocketAddrV4::new(Ipv4Addr::LOCALHOST, self.port).into()
    }

    /// Command-line arguments for Kanata, without the binary itself.
    pub fn args(&self) -> Vec<OsString> {
        let mut args: Vec<OsString> = vec![
            "--cfg".into(),
            self.config.clone().into_os_string(),
            "--port".into(),
            self.address().to_string().into(),
            // Without this, kanata waits for a keypress before exiting on error,
            // which would hang the child process.
            "--no-wait".into(),
        ];
        args.extend(self.extra_args.iter().cloned());
        args
    }

    /// Build the `Command` (stdout/stderr piped for log capture).
    pub fn command(&self) -> Command {
        self.command_with(self.args())
    }

    /// `kanata --cfg PATH --check --no-wait`: validates the config without
    /// grabbing anything. Still a Kanata we start, so it gets the same
    /// parent-death signal.
    pub fn check_command(&self) -> Command {
        self.command_with(vec![
            "--cfg".into(),
            self.config.clone().into_os_string(),
            "--check".into(),
            "--no-wait".into(),
        ])
    }

    fn command_with(&self, args: Vec<OsString>) -> Command {
        let mut cmd = Command::new(&self.kanata_bin);
        cmd.args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if self.parent_death_signal {
            with_parent_death_signal(&mut cmd);
        }
        cmd
    }

    /// Spawn Kanata. The caller owns the child and must stop it on exit.
    pub fn spawn(&self) -> Result<Child> {
        Ok(self.command().spawn()?)
    }
}

/// Make the child get SIGTERM when the spawning thread dies, and exit at once
/// if the parent is already gone by the time the child runs.
pub fn with_parent_death_signal(cmd: &mut Command) {
    let parent = std::process::id() as libc::pid_t;
    // SAFETY: the closure runs in the forked child before exec and only makes
    // async-signal-safe calls (prctl, getppid); it allocates nothing.
    unsafe {
        cmd.pre_exec(move || {
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM) != 0 {
                return Err(io::Error::last_os_error());
            }
            if libc::getppid() != parent {
                return Err(io::Error::other("parent exited before kanata started"));
            }
            Ok(())
        });
    }
}

/// Stop a child: SIGTERM, wait up to `grace`, then SIGKILL. Always reaps it.
pub fn terminate(child: &mut Child, grace: Duration) -> io::Result<ExitStatus> {
    if let Some(status) = child.try_wait()? {
        return Ok(status);
    }
    // SAFETY: plain kill(2) on our own, not yet reaped child.
    unsafe {
        libc::kill(child.id() as libc::pid_t, libc::SIGTERM);
    }
    let deadline = Instant::now() + grace;
    while Instant::now() < deadline {
        if let Some(status) = child.try_wait()? {
            return Ok(status);
        }
        thread::sleep(Duration::from_millis(50));
    }
    child.kill()?;
    child.wait()
}

/// Ask the OS for a currently free localhost port.
///
/// There is a small race between releasing it and Kanata binding it; the
/// launcher should retry with a new port if Kanata fails to bind.
pub fn free_port() -> Result<u16> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
    Ok(listener.local_addr()?.port())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_expected_args() {
        let mut opts = LaunchOptions::new("/home/me/.config/kanata/config.kbd", 7070);
        opts.extra_args.push("--debug".into());
        let args: Vec<String> = opts
            .args()
            .into_iter()
            .map(|a| a.into_string().unwrap())
            .collect();
        assert_eq!(
            args,
            [
                "--cfg",
                "/home/me/.config/kanata/config.kbd",
                "--port",
                "127.0.0.1:7070",
                "--no-wait",
                "--debug"
            ]
        );
    }

    #[test]
    fn free_port_is_nonzero() {
        assert_ne!(free_port().unwrap(), 0);
    }
}
