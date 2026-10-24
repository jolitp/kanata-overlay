//! The overlay's place in the desktop (feature 007): Start at login, requests
//! from a second instance, and the permission check. Startup work that must
//! happen before Qt (single instance, desktop entry) is in `main.rs`; it hands
//! over the listener and the launch target here.

use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{Mutex, OnceLock};
use std::thread;

use cxx_qt::Threading;
use cxx_qt_lib::QString;
use serde_json::json;

use crate::desktop_entry;
use crate::single_instance::{self, Request};

/// This instance's socket, set by `main` once it is the primary instance.
pub static LISTENER: Mutex<Option<UnixListener>> = Mutex::new(None);
/// What the desktop and login entries run (research R9), set by `main`.
pub static LAUNCH: OnceLock<PathBuf> = OnceLock::new();

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    #[auto_cxx_name]
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        /// The login entry exists (FR-007).
        #[qproperty(bool, start_at_login)]
        /// `{items: [{name, status}], fix, nixos}` (FR-010, FR-011).
        #[qproperty(QString, permissions_json)]
        type DesktopState = super::DesktopStateRust;

        /// Serve requests from later starts; read the login entry's state.
        #[qinvokable]
        fn start(self: Pin<&mut DesktopState>);

        #[qinvokable]
        fn set_login(self: Pin<&mut DesktopState>, on: bool);

        #[qinvokable]
        fn check_permissions(self: Pin<&mut DesktopState>);

        /// Another start of the overlay asked for the settings window.
        #[qsignal]
        fn open_settings(self: Pin<&mut DesktopState>);
    }

    impl cxx_qt::Threading for DesktopState {}
}

#[derive(Default)]
pub struct DesktopStateRust {
    start_at_login: bool,
    permissions_json: QString,
}

fn autostart_path() -> Option<PathBuf> {
    desktop_entry::config_home().map(|c| desktop_entry::autostart_path(&c))
}

impl qobject::DesktopState {
    fn start(mut self: Pin<&mut Self>) {
        let on = autostart_path().is_some_and(|p| p.is_file());
        self.as_mut().set_start_at_login(on);
        let listener = LISTENER.lock().ok().and_then(|mut l| l.take());
        if let Some(listener) = listener {
            let qt = self.qt_thread();
            thread::spawn(move || {
                single_instance::serve(listener, |request| match request {
                    Request::Settings => {
                        let _ = qt.queue(|o| o.open_settings());
                    }
                });
            });
        }
    }

    fn set_login(mut self: Pin<&mut Self>, on: bool) {
        let Some(path) = autostart_path() else {
            return;
        };
        let result = match (on, LAUNCH.get()) {
            (true, Some(exec)) => {
                desktop_entry::write_if_changed(&path, &desktop_entry::autostart_entry(exec))
                    .map(|_| ())
            }
            (true, None) => Err(std::io::Error::other("no launch target")),
            (false, _) => match std::fs::remove_file(&path) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
                _ => Ok(()),
            },
        };
        if let Err(e) = result {
            tracing::warn!("start at login: {}: {e}", path.display());
        }
        self.as_mut().set_start_at_login(path.is_file());
    }

    fn check_permissions(mut self: Pin<&mut Self>) {
        use kanata_client::permissions::{gather, report};
        let r = report(&gather());
        let items: Vec<_> = r
            .items
            .iter()
            .map(|(item, status)| json!({"name": item.name(), "status": status.as_str()}))
            .collect();
        let value = json!({"items": items, "fix": r.fix, "nixos": r.nixos});
        self.as_mut()
            .set_permissions_json(QString::from(&value.to_string()));
    }
}
