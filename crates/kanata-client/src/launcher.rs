//! Launching Kanata as a managed child process.
//!
//! This is the "managed" backend: the overlay owns the `kanata` process, runs it
//! as the current user (see the README for the one-time `input`/`uinput` group
//! setup that makes root unnecessary), and talks to it over a localhost-only TCP
//! port.
//!
//! Still to do (tracked as a Spec Kit feature, not here): readiness probing,
//! log capture, restart-with-backoff and `PR_SET_PDEATHSIG` so Kanata can never
//! outlive the overlay and leave the keyboard grabbed.

use std::ffi::OsString;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

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
}

impl LaunchOptions {
    pub fn new(config: impl Into<PathBuf>, port: u16) -> Self {
        Self {
            kanata_bin: PathBuf::from("kanata"),
            config: config.into(),
            port,
            extra_args: Vec::new(),
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
        let mut cmd = Command::new(&self.kanata_bin);
        cmd.args(self.args())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        cmd
    }

    /// Spawn Kanata. The caller owns the child and must stop it on exit.
    pub fn spawn(&self) -> Result<Child> {
        Ok(self.command().spawn()?)
    }
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
