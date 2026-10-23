//! Who runs Kanata: the overlay (managed), someone else (attached), or nobody
//! (contracts/interfaces.md › KanataState).
//!
//! The supervisor (in `kanata-client`) does the work; this bridges its events
//! to QML. Kanata's output reaches QML through `log_text` only; it is never
//! passed to `tracing` (constitution VI).

use std::path::PathBuf;
use std::pin::Pin;
use std::thread;
use std::time::Duration;

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use kanata_client::{
    LaunchOptions, Probe, SessionState, Supervisor, SupervisorEvent, SupervisorOptions, launcher,
    probe,
};

use crate::{Args, overlay_state};

/// `log_text` is refreshed at most this often.
const LOG_REFRESH: Duration = Duration::from_millis(250);

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
        /// `none`, `attached` or `managed`.
        #[qproperty(QString, mode)]
        /// Short status for the header ("managed", "restarting Kanata (attempt 2)" …).
        #[qproperty(QString, state_text)]
        /// Recent output of a managed Kanata.
        #[qproperty(QString, log_text)]
        #[qproperty(bool, can_restart)]
        /// Attached, disconnected and a config was given (FR-014).
        #[qproperty(bool, can_start)]
        type KanataState = super::KanataStateRust;

        /// Called once by Main.qml: attach, manage or neither.
        #[qinvokable]
        fn start(self: Pin<&mut KanataState>);

        /// Restart a managed Kanata (FR-009).
        #[qinvokable]
        fn restart(self: Pin<&mut KanataState>);

        /// Attached Kanata is gone: start our own (US3 scenario 4).
        #[qinvokable]
        fn start_managed(self: Pin<&mut KanataState>);

        /// Stop a managed Kanata; called on quit.
        #[qinvokable]
        fn shutdown(self: Pin<&mut KanataState>);

        #[qinvokable]
        fn set_connected_state(self: Pin<&mut KanataState>, connected: bool);

    }

    impl cxx_qt::Threading for KanataState {}
}

#[derive(Default)]
pub struct KanataStateRust {
    mode: QString,
    state_text: QString,
    log_text: QString,
    can_restart: bool,
    can_start: bool,

    supervisor: Option<Supervisor>,
    connected: bool,
    config: Option<PathBuf>,
    kanata_bin: Option<PathBuf>,
    address: String,
    log_refresh_pending: bool,
}

impl qobject::KanataState {
    fn start(mut self: Pin<&mut Self>) {
        let args = Args::from_env();
        {
            let mut rust = self.as_mut().rust_mut();
            rust.config = args.config.clone();
            rust.kanata_bin = args.kanata.clone();
            rust.address = args.address.clone();
        }
        self.as_mut().set_mode(QString::from("none"));

        let qt = self.qt_thread();
        let address = args.address;
        thread::spawn(move || {
            let found = address
                .parse()
                .map(|addr| probe(addr, Duration::from_secs(1)))
                .unwrap_or(Probe::Nothing);
            let _ = qt.queue(move |mut o| match found {
                Probe::Kanata => o.as_mut().attach(),
                _ if o.rust().config.is_none() => {}
                Probe::Nothing => {
                    let port = port_of(&o.rust().address);
                    o.as_mut().manage(port);
                }
                // Something else owns the port: run our Kanata elsewhere.
                Probe::NotKanata => {
                    let port = launcher::free_port().unwrap_or(0);
                    o.as_mut().manage(port);
                }
            });
        });
    }

    fn restart(self: Pin<&mut Self>) {
        if let Some(s) = &self.rust().supervisor {
            s.restart();
        }
    }

    fn start_managed(mut self: Pin<&mut Self>) {
        if !*self.can_start() {
            return;
        }
        let configured = port_of(&self.rust().address);
        let addr = std::net::SocketAddr::from(([127, 0, 0, 1], configured));
        let port = match probe(addr, Duration::from_millis(300)) {
            Probe::Nothing => configured,
            _ => launcher::free_port().unwrap_or(configured),
        };
        self.as_mut().manage(port);
    }

    fn shutdown(mut self: Pin<&mut Self>) {
        if let Some(s) = self.as_mut().rust_mut().supervisor.take() {
            s.stop();
        }
    }

    fn set_connected_state(mut self: Pin<&mut Self>, connected: bool) {
        self.as_mut().rust_mut().connected = connected;
        self.update_can_start();
    }

    fn update_can_start(mut self: Pin<&mut Self>) {
        let can = self.mode().to_string() == "attached"
            && !self.rust().connected
            && self.rust().config.is_some();
        self.as_mut().set_can_start(can);
    }

    /// A Kanata answered on the configured address: use it, never stop it.
    fn attach(mut self: Pin<&mut Self>) {
        self.as_mut().set_mode(QString::from("attached"));
        self.as_mut().set_state_text(QString::from("attached"));
        self.as_mut().set_can_restart(false);
        self.update_can_start();
    }

    /// Start and supervise our own Kanata on `port`.
    fn manage(mut self: Pin<&mut Self>, port: u16) {
        let Some(config) = self.rust().config.clone() else {
            return;
        };
        let mut opts = LaunchOptions::new(config, port);
        if let Some(bin) = &self.rust().kanata_bin {
            opts.kanata_bin = bin.clone();
        }
        let qt = self.qt_thread();
        let supervisor = Supervisor::start(opts, SupervisorOptions::default(), move |event| {
            let _ = qt.queue(move |o| o.on_event(event));
        });
        self.as_mut().rust_mut().supervisor = Some(supervisor);
        self.as_mut().set_mode(QString::from("managed"));
        self.as_mut().set_can_restart(true);
        self.update_can_start();
    }

    fn on_event(mut self: Pin<&mut Self>, event: SupervisorEvent) {
        match event {
            SupervisorEvent::State(state) => {
                tracing::debug!("kanata: {}", state_text(&state));
                self.as_mut()
                    .set_state_text(QString::from(state_text(&state)));
            }
            SupervisorEvent::Ready(addr) => overlay_state::set_address(addr.to_string()),
            SupervisorEvent::Attach(addr) => {
                // Another overlay's Kanata won the port; its supervisor thread has ended.
                drop(self.as_mut().rust_mut().supervisor.take());
                overlay_state::set_address(addr.to_string());
                self.attach();
            }
            SupervisorEvent::LogChanged => self.schedule_log_refresh(),
        }
    }

    /// Refresh `log_text` at most every `LOG_REFRESH`.
    fn schedule_log_refresh(mut self: Pin<&mut Self>) {
        if self.rust().log_refresh_pending {
            return;
        }
        self.as_mut().rust_mut().log_refresh_pending = true;
        let qt = self.qt_thread();
        thread::spawn(move || {
            thread::sleep(LOG_REFRESH);
            let _ = qt.queue(|mut o| {
                o.as_mut().rust_mut().log_refresh_pending = false;
                let text = o
                    .rust()
                    .supervisor
                    .as_ref()
                    .map(|s| s.log().join("\n"))
                    .unwrap_or_default();
                o.as_mut().set_log_text(QString::from(&text));
            });
        });
    }
}

fn state_text(state: &SessionState) -> String {
    match state {
        SessionState::Checking => "checking config…".to_owned(),
        SessionState::Starting => "starting Kanata…".to_owned(),
        SessionState::WaitingForKeyboard => "waiting for a keyboard".to_owned(),
        SessionState::Running => "managed".to_owned(),
        SessionState::Restarting { attempt, .. } => {
            format!("restarting Kanata (attempt {attempt})")
        }
        // The reason itself, in plain words (FR-008), so the header says why.
        SessionState::Failed(f) => f.notice(),
        SessionState::Stopped => String::new(),
    }
}

fn port_of(address: &str) -> u16 {
    address
        .rsplit(':')
        .next()
        .and_then(|p| p.parse().ok())
        .unwrap_or(7070)
}
