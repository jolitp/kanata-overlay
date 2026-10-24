//! One overlay per user session and settings file (feature 007, research R5):
//! a Unix socket in the runtime directory. A second start tells the first to
//! open its settings and exits.

use std::io::{BufRead, BufReader, Write as _};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Request {
    /// Open the settings window.
    Settings,
}

pub fn parse(line: &str) -> Option<Request> {
    match line.trim() {
        "settings" => Some(Request::Settings),
        _ => None,
    }
}

/// FNV-1a: stable across runs and builds (unlike `DefaultHasher`).
fn fnv1a(bytes: &[u8]) -> u32 {
    bytes.iter().fold(0x811c_9dc5, |h, b| {
        (h ^ u32::from(*b)).wrapping_mul(0x0100_0193)
    })
}

/// The socket for the instance using `settings_path`.
pub fn socket_path(settings_path: &Path) -> PathBuf {
    let hash = fnv1a(settings_path.to_string_lossy().as_bytes());
    match std::env::var_os("XDG_RUNTIME_DIR").filter(|v| !v.is_empty()) {
        Some(dir) => PathBuf::from(dir).join(format!("kanata-overlay-{hash:08x}.sock")),
        None => {
            // SAFETY: getuid never fails.
            let uid = unsafe { libc::getuid() };
            PathBuf::from(format!("/tmp/kanata-overlay-{uid}-{hash:08x}.sock"))
        }
    }
}

pub enum Claim {
    /// We are the instance; serve `Request`s on this listener.
    Primary(UnixListener),
    /// Another instance answered and was asked to open its settings.
    Secondary,
}

/// Become the instance for `path`, or hand over to the one already running.
pub fn claim(path: &Path) -> std::io::Result<Claim> {
    if let Ok(mut other) = UnixStream::connect(path) {
        let _ = other.set_write_timeout(Some(Duration::from_secs(1)));
        if other.write_all(b"settings\n").is_ok() {
            return Ok(Claim::Secondary);
        }
    }
    // Nobody answered: a stale socket from a crashed run.
    let _ = std::fs::remove_file(path);
    Ok(Claim::Primary(UnixListener::bind(path)?))
}

/// Read requests from each connection, calling `on` for each (runs forever on
/// the calling thread).
pub fn serve(listener: UnixListener, on: impl Fn(Request)) {
    for stream in listener.incoming().map_while(Result::ok) {
        let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
        for line in BufReader::new(stream).lines().map_while(Result::ok) {
            if let Some(r) = parse(&line) {
                on(r);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn requests() {
        assert_eq!(parse("settings\n"), Some(Request::Settings));
        assert_eq!(parse("quit"), None);
    }

    #[test]
    fn one_socket_per_settings_file() {
        let a = socket_path(Path::new("/h/.config/kanata-overlay/settings.json"));
        assert_eq!(
            a,
            socket_path(Path::new("/h/.config/kanata-overlay/settings.json"))
        );
        assert_ne!(a, socket_path(Path::new("/tmp/scratch/settings.json")));
    }

    #[test]
    fn second_claim_hands_over() {
        let dir = std::env::temp_dir().join(format!("single-instance-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("s.sock");
        std::fs::write(&path, "stale").unwrap();
        let Claim::Primary(listener) = claim(&path).unwrap() else {
            panic!("a stale file must not count as a running instance");
        };
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || serve(listener, move |r| tx.send(r).unwrap()));
        assert!(matches!(claim(&path).unwrap(), Claim::Secondary));
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(2)).unwrap(),
            Request::Settings
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
