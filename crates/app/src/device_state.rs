//! One device: its Kanata session (managed or attached), its TCP connection,
//! active layer, layout and labels (feature 004, data-model › DeviceState).
//!
//! This is today's per-device code from `overlay_state.rs` and the former
//! `kanata_state.rs`, one instance per device. Background threads touch the
//! QObject only through `qt_thread().queue(...)`; Kanata's output reaches QML
//! through `log_text` only and is never passed to `tracing` (constitution VI).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use kanata_client::live::{Mode, Route, route};
use kanata_client::protocol::{ClientMessage, ServerMessage};
use kanata_client::{
    Client, DebugKeyTap, Incoming, KeyAction, KeyEvent, LaunchOptions, Probe, SessionState,
    Supervisor, SupervisorEvent, SupervisorOptions, launcher, probe,
};
use layout::Layout;
use serde_json::Value;

use crate::Args;

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
        #[qproperty(QString, device_id)]
        /// Shown on the board header and in the settings window.
        #[qproperty(QString, name)]
        #[qproperty(QString, current_layer)]
        #[qproperty(bool, connected)]
        #[qproperty(QString, status)]
        /// The physical layout as JSON (the `layout` crate's format); QML parses it.
        #[qproperty(QString, layout_json)]
        /// Per-key labels for the active layer (contracts/labels.md); `{}` without a config.
        #[qproperty(QString, labels_json)]
        /// This device's notices, joined with " · ".
        #[qproperty(QString, notices)]
        /// `none`, `attached` or `managed`.
        #[qproperty(QString, mode)]
        /// Short status for the header ("managed", "restarting Kanata (attempt 2)" …).
        #[qproperty(QString, state_text)]
        /// Recent output of a managed Kanata.
        #[qproperty(QString, log_text)]
        #[qproperty(bool, can_restart)]
        /// Attached, disconnected and a config was given (FR-014 of feature 003).
        #[qproperty(bool, can_start)]
        /// Held keys (canonical names) as a JSON array (feature 006). Memory only.
        #[qproperty(QString, pressed_json)]
        /// Recent tap-hold results: `{canonical: "tap" | "hold"}`.
        #[qproperty(QString, tap_hold_json)]
        /// Where presses come from: "debug" (a Kanata we started), "tcp"
        /// (tap-hold results only) or "off".
        #[qproperty(QString, press_source)]
        type DeviceState = super::DeviceStateRust;

        /// Start with this device's settings (`SettingsState.device(id)`) and
        /// its device check (`SettingsState.deviceCheck(id)`), both JSON.
        #[qinvokable]
        fn start(self: Pin<&mut DeviceState>, device: &QString, check: &QString);

        /// The device's settings changed: restart its session if its config,
        /// address or device check changed (FR-001a); a new layout alone is
        /// just reloaded (feature 005, research R6).
        #[qinvokable]
        fn apply(self: Pin<&mut DeviceState>, device: &QString, check: &QString);

        /// Show this layout (the layout editor's) instead of the saved one; no
        /// file is touched (feature 005, FR-013).
        #[qinvokable]
        fn preview_layout(self: Pin<&mut DeviceState>, json: &QString);

        /// Back to the saved layout (or the grid).
        #[qinvokable]
        fn end_preview(self: Pin<&mut DeviceState>);

        /// Reread the layout if `path` is this device's layout file (a layout
        /// shared by several devices was saved).
        #[qinvokable]
        fn reload_layout(self: Pin<&mut DeviceState>, path: &QString);

        /// The loaded config's `defsrc` names as written, as a JSON array.
        #[qinvokable]
        fn config_keys_json(self: &DeviceState) -> QString;

        #[qinvokable]
        fn restart(self: Pin<&mut DeviceState>);

        #[qinvokable]
        fn start_managed(self: Pin<&mut DeviceState>);

        /// Stop a managed Kanata (quit, or the device was removed).
        #[qinvokable]
        fn shutdown(self: Pin<&mut DeviceState>);

        /// Ask this device's Kanata to switch layers (handy for testing).
        #[qinvokable]
        fn request_layer(self: Pin<&mut DeviceState>, name: &QString);

        /// Whether anything shows this device's presses (its board, or the
        /// layout editor on it). Off: presses are dropped and cleared.
        #[qinvokable]
        fn set_watching(self: Pin<&mut DeviceState>, on: bool);

        /// Drop expired tap-hold results (QML calls it while some are shown).
        #[qinvokable]
        fn tick(self: Pin<&mut DeviceState>);

        /// Press to bind: the next press is sent as `captured` instead of shown.
        /// False when presses aren't available.
        #[qinvokable]
        fn capture(self: Pin<&mut DeviceState>) -> bool;

        #[qinvokable]
        fn cancel_capture(self: Pin<&mut DeviceState>);

        /// The key pressed after `capture()` (canonical name).
        #[qsignal]
        fn captured(self: Pin<&mut DeviceState>, key: QString);
    }

    impl cxx_qt::Threading for DeviceState {}
}

/// The parts of a device's settings that need a restart when they change.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Setup {
    config: Option<PathBuf>,
    layout: Option<PathBuf>,
    address: String,
    /// Kanata runs with `--debug` for presses (highlight not "off"); changing it
    /// restarts the session (feature 006, research R5).
    detailed: bool,
}

/// Shared with the output reader and connection threads, which filter key
/// events before anything reaches the Qt thread (research R3).
#[derive(Debug, Default)]
struct Live {
    mode: AtomicU8,
    watching: AtomicBool,
    armed: AtomicBool,
}

impl Live {
    fn mode(&self) -> Mode {
        match self.mode.load(Ordering::Relaxed) {
            1 => Mode::NoTyping,
            2 => Mode::Off,
            _ => Mode::All,
        }
    }

    fn set_mode(&self, mode: Mode) {
        let v = match mode {
            Mode::All => 0,
            Mode::NoTyping => 1,
            Mode::Off => 2,
        };
        self.mode.store(v, Ordering::Relaxed);
    }

    fn route(&self, event: KeyEvent) -> Route {
        route(
            event,
            self.armed.load(Ordering::Relaxed),
            self.watching.load(Ordering::Relaxed),
            self.mode(),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TapHold {
    Tap,
    Hold,
}

/// How long a tap shows, and a hold when its release can't be seen.
const TAP_SHOWN: Duration = Duration::from_secs(1);
const HOLD_SHOWN: Duration = Duration::from_millis(1500);

pub struct DeviceStateRust {
    device_id: QString,
    name: QString,
    current_layer: QString,
    connected: bool,
    status: QString,
    layout_json: QString,
    labels_json: QString,
    notices: QString,
    mode: QString,
    state_text: QString,
    log_text: QString,
    can_restart: bool,
    can_start: bool,
    pressed_json: QString,
    tap_hold_json: QString,
    press_source: QString,

    setup: Setup,
    blocked: Option<String>,
    /// The device check this session started with, as given to `begin`.
    check: String,
    config: Option<kbd::Config>,
    /// Shared with the connection thread.
    address: Arc<Mutex<String>>,
    config_path: Arc<Mutex<Option<PathBuf>>>,
    supervisor: Option<Supervisor>,
    kanata_bin: Option<PathBuf>,
    notice_list: Vec<String>,
    log_refresh_pending: bool,
    connection_started: bool,
    live: Arc<Live>,
    /// Keys held now; memory only, no history (constitution VI).
    held: BTreeSet<String>,
    tap_hold: BTreeMap<String, (TapHold, Instant)>,
}

impl Default for DeviceStateRust {
    fn default() -> Self {
        Self {
            device_id: QString::default(),
            name: QString::default(),
            current_layer: QString::default(),
            connected: false,
            status: QString::default(),
            layout_json: QString::default(),
            labels_json: QString::from("{}"),
            notices: QString::default(),
            mode: QString::from("none"),
            state_text: QString::default(),
            log_text: QString::default(),
            can_restart: false,
            can_start: false,
            pressed_json: QString::from("[]"),
            tap_hold_json: QString::from("{}"),
            press_source: QString::from("off"),
            setup: Setup::default(),
            blocked: None,
            check: String::new(),
            config: None,
            address: Arc::default(),
            config_path: Arc::default(),
            supervisor: None,
            kanata_bin: None,
            notice_list: Vec::new(),
            log_refresh_pending: false,
            connection_started: false,
            live: Arc::default(),
            held: BTreeSet::new(),
            tap_hold: BTreeMap::new(),
        }
    }
}

fn path_field(device: &Value, key: &str) -> Option<PathBuf> {
    device
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
}

impl qobject::DeviceState {
    fn start(mut self: Pin<&mut Self>, device: &QString, check: &QString) {
        self.as_mut().rust_mut().kanata_bin = Args::from_env().kanata;
        self.as_mut().begin(device, check);
        if !self.rust().connection_started {
            self.as_mut().rust_mut().connection_started = true;
            let address = self.rust().address.clone();
            let config_path = self.rust().config_path.clone();
            let live = self.rust().live.clone();
            spawn_connection(self.qt_thread(), address, config_path, live);
        }
    }

    fn apply(mut self: Pin<&mut Self>, device: &QString, check: &QString) {
        let value: Value = serde_json::from_str(&device.to_string()).unwrap_or(Value::Null);
        if let Some(name) = value.get("name").and_then(Value::as_str) {
            self.as_mut().set_name(QString::from(name));
        }
        // "All" ↔ "no typing keys" needs no restart; held keys are cleared so
        // a typing key held across the switch can't stay lit.
        let mode = mode_of(&value);
        if mode != self.rust().live.mode() {
            self.rust().live.set_mode(mode);
            self.as_mut().clear_live();
        }
        let setup = setup_of(&value);
        let same_session = setup.config == self.rust().setup.config
            && setup.address == self.rust().setup.address
            && setup.detailed == self.rust().setup.detailed
            && check.to_string() == self.rust().check;
        if same_session && setup.layout != self.rust().setup.layout {
            self.as_mut().rust_mut().setup.layout = setup.layout;
            self.reload_current_layout();
            return;
        }
        // A new check result (a config or the device list changed) can add or
        // lift a block or a warning: start over like for a new setup.
        if !same_session {
            self.as_mut().shutdown();
            {
                let mut rust = self.as_mut().rust_mut();
                rust.notice_list.clear();
                rust.config = None;
            }
            self.as_mut().set_notices(QString::default());
            self.as_mut().set_labels_json(QString::from("{}"));
            self.as_mut().set_log_text(QString::default());
            self.begin(device, check);
        }
    }

    /// Load layout and config, then attach or manage (research R4, feature 003).
    fn begin(mut self: Pin<&mut Self>, device: &QString, check: &QString) {
        let value: Value = serde_json::from_str(&device.to_string()).unwrap_or(Value::Null);
        let setup = setup_of(&value);
        let id = value
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        let name = value
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or(&id)
            .to_owned();
        self.as_mut().set_device_id(QString::from(&id));
        self.as_mut().set_name(QString::from(&name));

        if let Ok(mut a) = self.rust().address.lock() {
            *a = setup.address.clone();
        }
        if let Ok(mut c) = self.rust().config_path.lock() {
            *c = setup.config.clone();
        }

        // The config first: a device without a layout is drawn from its defsrc.
        if let Some(path) = &setup.config {
            match kbd::read(path) {
                Ok(config) => self.as_mut().use_config(config),
                Err(e) => self
                    .as_mut()
                    .add_notice(&format!("config: cannot read {}: {e}", path.display())),
            }
        }
        self.as_mut().load_layout(setup.layout.as_deref(), &name);

        self.as_mut().rust_mut().check = check.to_string();
        let check: Value = serde_json::from_str(&check.to_string()).unwrap_or(Value::Null);
        for w in check
            .get("warnings")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            self.as_mut().add_notice(w);
        }
        let blocked = check
            .get("blocked")
            .and_then(Value::as_str)
            .map(str::to_owned);
        {
            let mut rust = self.as_mut().rust_mut();
            rust.setup = setup.clone();
            rust.blocked = blocked;
            rust.live.set_mode(mode_of(&value));
        }
        self.as_mut().set_mode(QString::from("none"));
        self.as_mut().set_source("off");
        self.as_mut().set_state_text(QString::default());
        self.as_mut().set_can_restart(false);

        let qt = self.qt_thread();
        let address = setup.address;
        thread::spawn(move || {
            let found = address
                .parse()
                .map(|addr| probe(addr, Duration::from_secs(1)))
                .unwrap_or(Probe::Nothing);
            let _ = qt.queue(move |mut o| match found {
                // Someone else's Kanata: use it, never stop it.
                Probe::Kanata => o.as_mut().attach(),
                _ if o.rust().setup.config.is_none() => {}
                _ if o.rust().blocked.is_some() => {
                    let by = o.rust().blocked.clone().unwrap_or_default();
                    o.as_mut().set_state_text(QString::from(&format!(
                        "not started: conflicts with {by}"
                    )));
                }
                Probe::Nothing => {
                    let port = port_of(&o.rust().setup.address);
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

    fn preview_layout(self: Pin<&mut Self>, json: &QString) {
        self.set_layout_json(QString::from(&with_canonical_names(&json.to_string())));
    }

    fn end_preview(self: Pin<&mut Self>) {
        self.reload_current_layout();
    }

    fn reload_layout(self: Pin<&mut Self>, path: &QString) {
        let path = PathBuf::from(path.to_string());
        if self.rust().setup.layout.as_ref() == Some(&path) {
            self.reload_current_layout();
        }
    }

    fn config_keys_json(&self) -> QString {
        let names: Vec<String> = self
            .rust()
            .config
            .iter()
            .flat_map(|c| &c.source)
            .map(|k| k.written.clone())
            .collect();
        QString::from(&serde_json::to_string(&names).unwrap_or_default())
    }

    /// Reread the current setup's layout, replacing its notices.
    fn reload_current_layout(mut self: Pin<&mut Self>) {
        self.as_mut()
            .rust_mut()
            .notice_list
            .retain(|n| !n.starts_with("layout: ") && !n.starts_with("no layout for "));
        let joined = self.rust().notice_list.join(" · ");
        self.as_mut().set_notices(QString::from(&joined));
        let path = self.rust().setup.layout.clone();
        let name = self.name().to_string();
        self.load_layout(path.as_deref(), &name);
    }

    fn load_layout(mut self: Pin<&mut Self>, path: Option<&Path>, name: &str) {
        let loaded = match path {
            Some(path) => std::fs::read_to_string(path)
                .map_err(anyhow::Error::from)
                .and_then(|text| parse_layout(&text).map_err(Into::into))
                .map_err(|e| format!("layout: cannot read {}: {e:#}", path.display())),
            None => Err(String::new()),
        };
        let source: Option<Vec<String>> = self
            .rust()
            .config
            .as_ref()
            .map(|c| c.source.iter().map(|k| k.written.clone()).collect());
        let layout = match (loaded, source) {
            (Ok(layout), _) => layout,
            // No (usable) layout but a config: a grid of its keys (research R7).
            (Err(e), Some(names)) => {
                if !e.is_empty() {
                    self.as_mut().add_notice(&e);
                }
                self.as_mut()
                    .add_notice(&format!("no layout for {name}: showing a grid"));
                Layout::grid(name, &names, 12)
            }
            (Err(e), None) => {
                if !e.is_empty() {
                    self.as_mut().add_notice(&e);
                }
                match parse_layout(include_str!("../../../examples/layouts/sample.kle.json")) {
                    Ok(l) => l,
                    Err(_) => return,
                }
            }
        };
        self.as_mut()
            .set_layout_json(QString::from(&with_canonical_names(&layout.to_json())));
    }

    /// Take a freshly read config and relabel the board.
    fn use_config(mut self: Pin<&mut Self>, config: kbd::Config) {
        for w in &config.warnings {
            self.as_mut().add_notice(&w.to_string());
        }
        self.as_mut().rust_mut().config = Some(config);
        self.relabel();
    }

    /// Recompute the labels: the current layer while connected, else the
    /// config's first layer (feature 002, FR-010).
    fn relabel(mut self: Pin<&mut Self>) {
        let Some(config) = &self.rust().config else {
            return;
        };
        let layer = if *self.connected() {
            self.current_layer().to_string()
        } else {
            String::new()
        };
        let (labels, warnings) = kbd::labels(config, &layer);
        let json: serde_json::Map<String, Value> = labels
            .into_iter()
            .map(|(key, l)| {
                let value = serde_json::json!({
                    "text": l.text,
                    "style": l.style.as_str(),
                    "hold": l.hold,
                    "alias": l.alias.unwrap_or_default(),
                });
                (key, value)
            })
            .collect();
        self.as_mut()
            .set_labels_json(QString::from(&Value::Object(json).to_string()));
        for w in warnings {
            self.as_mut().add_notice(&w.to_string());
        }
    }

    fn add_notice(mut self: Pin<&mut Self>, text: &str) {
        if text.is_empty() || self.rust().notice_list.iter().any(|n| n == text) {
            return;
        }
        tracing::info!("notice ({}): {text}", self.device_id());
        self.as_mut().rust_mut().notice_list.push(text.to_owned());
        let joined = self.rust().notice_list.join(" · ");
        self.as_mut().set_notices(QString::from(&joined));
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
        let configured = port_of(&self.rust().setup.address);
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
        self.rust().live.armed.store(false, Ordering::Relaxed);
        self.as_mut().clear_live();
        self.as_mut().set_can_restart(false);
    }

    fn set_watching(self: Pin<&mut Self>, on: bool) {
        self.rust().live.watching.store(on, Ordering::Relaxed);
        if !on {
            self.clear_live();
        }
    }

    fn capture(self: Pin<&mut Self>) -> bool {
        let available = self.press_source().to_string() == "debug";
        self.rust().live.armed.store(available, Ordering::Relaxed);
        available
    }

    fn cancel_capture(self: Pin<&mut Self>) {
        self.rust().live.armed.store(false, Ordering::Relaxed);
    }

    /// A press or release that passed the filter.
    fn key_event(mut self: Pin<&mut Self>, event: KeyEvent) {
        {
            let mut rust = self.as_mut().rust_mut();
            match event.action {
                KeyAction::Press => {
                    rust.held.insert(event.key);
                }
                KeyAction::Release => {
                    if matches!(rust.tap_hold.get(&event.key), Some((TapHold::Hold, _))) {
                        rust.tap_hold.remove(&event.key);
                    }
                    rust.held.remove(&event.key);
                }
                KeyAction::Repeat => {}
            }
        }
        self.publish_live();
    }

    fn tap_hold_event(mut self: Pin<&mut Self>, key: String, kind: TapHold) {
        self.as_mut()
            .rust_mut()
            .tap_hold
            .insert(key, (kind, Instant::now()));
        self.publish_live();
    }

    fn tick(mut self: Pin<&mut Self>) {
        let releases_seen = self.press_source().to_string() == "debug";
        let before = self.rust().tap_hold.len();
        self.as_mut()
            .rust_mut()
            .tap_hold
            .retain(|_, (kind, since)| match kind {
                TapHold::Tap => since.elapsed() < TAP_SHOWN,
                TapHold::Hold => releases_seen || since.elapsed() < HOLD_SHOWN,
            });
        if self.rust().tap_hold.len() != before {
            self.publish_live();
        }
    }

    /// Forget every held key and tap-hold result (FR-004).
    fn clear_live(mut self: Pin<&mut Self>) {
        {
            let mut rust = self.as_mut().rust_mut();
            rust.held.clear();
            rust.tap_hold.clear();
        }
        self.publish_live();
    }

    fn publish_live(mut self: Pin<&mut Self>) {
        let pressed = serde_json::to_string(&self.rust().held).unwrap_or_default();
        let tap_hold: serde_json::Map<String, Value> = self
            .rust()
            .tap_hold
            .iter()
            .map(|(k, (kind, _))| {
                let kind = if *kind == TapHold::Tap { "tap" } else { "hold" };
                (k.clone(), Value::from(kind))
            })
            .collect();
        let tap_hold = Value::Object(tap_hold).to_string();
        if *self.pressed_json() != QString::from(&pressed) {
            self.as_mut().set_pressed_json(QString::from(&pressed));
        }
        if *self.tap_hold_json() != QString::from(&tap_hold) {
            self.as_mut().set_tap_hold_json(QString::from(&tap_hold));
        }
    }

    fn set_source(mut self: Pin<&mut Self>, source: &str) {
        if self.press_source().to_string() != source {
            self.as_mut().set_press_source(QString::from(source));
            self.clear_live();
        }
    }

    fn update_can_start(mut self: Pin<&mut Self>) {
        let can = self.mode().to_string() == "attached"
            && !*self.connected()
            && self.rust().setup.config.is_some()
            && self.rust().blocked.is_none();
        self.as_mut().set_can_start(can);
    }

    /// A Kanata answered on the configured address: use it, never stop it.
    fn attach(mut self: Pin<&mut Self>) {
        let source = if self.rust().live.mode() == Mode::Off {
            "off"
        } else {
            "tcp"
        };
        self.as_mut().set_source(source);
        self.as_mut().set_mode(QString::from("attached"));
        self.as_mut().set_state_text(QString::from("attached"));
        self.as_mut().set_can_restart(false);
        self.update_can_start();
    }

    /// Start and supervise our own Kanata on `port`.
    fn manage(mut self: Pin<&mut Self>, port: u16) {
        let Some(config) = self.rust().setup.config.clone() else {
            return;
        };
        let mut opts = LaunchOptions::new(config, port);
        if let Some(bin) = &self.rust().kanata_bin {
            opts.kanata_bin = bin.clone();
        }
        let mut timings = SupervisorOptions::default();
        let detailed = self.rust().setup.detailed;
        if detailed {
            // Presses come from Kanata's debug output (research R1); the tap
            // swallows every debug line, so none reaches the log (VI).
            opts.extra_args.push("--debug".into());
            let live = self.rust().live.clone();
            let qt = self.qt_thread();
            timings.tap = Some(Arc::new(DebugKeyTap::new(move |event| {
                match live.route(event) {
                    Route::Capture(key) => {
                        live.armed.store(false, Ordering::Relaxed);
                        let _ = qt.queue(move |o| o.captured(QString::from(&key)));
                    }
                    Route::Show(event) => {
                        let read = Instant::now();
                        let _ = qt.queue(move |o| {
                            o.key_event(event);
                            // Timing only, never the key (VI).
                            tracing::trace!(
                                "key event shown {:?} after it was read",
                                read.elapsed()
                            );
                        });
                    }
                    Route::Drop => {}
                }
            })));
        }
        self.as_mut()
            .set_source(if detailed { "debug" } else { "off" });
        let qt = self.qt_thread();
        let supervisor = Supervisor::start(opts, timings, move |event| {
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
                tracing::debug!("kanata ({}): {}", self.device_id(), state_text(&state));
                if state != SessionState::Running {
                    self.as_mut().clear_live();
                }
                self.as_mut()
                    .set_state_text(QString::from(&state_text(&state)));
            }
            SupervisorEvent::Ready(addr) => self.set_shared_address(addr.to_string()),
            SupervisorEvent::Attach(addr) => {
                // Another overlay's Kanata won the port; its supervisor thread has ended.
                drop(self.as_mut().rust_mut().supervisor.take());
                self.as_mut().set_shared_address(addr.to_string());
                self.attach();
            }
            SupervisorEvent::LogChanged => self.schedule_log_refresh(),
        }
    }

    fn set_shared_address(self: Pin<&mut Self>, address: String) {
        if let Ok(mut a) = self.rust().address.lock() {
            *a = address;
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

    fn request_layer(self: Pin<&mut Self>, name: &QString) {
        let address = self
            .rust()
            .address
            .lock()
            .map(|a| a.clone())
            .unwrap_or_default();
        let name = name.to_string();
        // Fire-and-forget on a short-lived connection; the main connection
        // will see the resulting LayerChange event.
        thread::spawn(move || {
            let result = Client::connect(address.as_str(), Duration::from_secs(1))
                .and_then(|mut c| c.send(&ClientMessage::ChangeLayer { new: name }));
            if let Err(e) = result {
                tracing::warn!("ChangeLayer failed: {e}");
            }
        });
    }
}

/// The device's highlight setting (feature 006); missing means "all".
fn mode_of(device: &Value) -> Mode {
    match device.get("highlight").and_then(Value::as_str) {
        Some("no-typing") => Mode::NoTyping,
        Some("off") => Mode::Off,
        _ => Mode::All,
    }
}

fn setup_of(device: &Value) -> Setup {
    Setup {
        detailed: mode_of(device) != Mode::Off,
        config: path_field(device, "config"),
        layout: path_field(device, "layout"),
        address: device
            .get("address")
            .and_then(Value::as_str)
            .unwrap_or("127.0.0.1:7070")
            .to_owned(),
    }
}

fn state_text(state: &SessionState) -> String {
    match state {
        SessionState::Checking => "checking config…".to_owned(),
        SessionState::Starting => "starting Kanata…".to_owned(),
        SessionState::WaitingForKeyboard => "waiting for its device".to_owned(),
        SessionState::Running => "managed".to_owned(),
        SessionState::Restarting { attempt, .. } => {
            format!("restarting Kanata (attempt {attempt})")
        }
        // The reason itself, in plain words (feature 003, FR-008).
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

/// Add each key's canonical Kanata name (`kanataCanonical`), so QML can look
/// up its label without knowing Kanata's key spellings.
pub(crate) fn with_canonical_names(layout_json: &str) -> String {
    let Ok(mut value) = serde_json::from_str::<Value>(layout_json) else {
        return layout_json.to_owned();
    };
    if let Some(keys) = value.get_mut("keys").and_then(|k| k.as_array_mut()) {
        for key in keys {
            if let Some(name) = key.get("kanata").and_then(|k| k.as_str()) {
                let canonical = kbd::canonical(name);
                key["kanataCanonical"] = Value::String(canonical);
            }
        }
    }
    value.to_string()
}

/// Accept either our own layout format or raw KLE JSON.
fn parse_layout(text: &str) -> layout::Result<Layout> {
    Layout::from_json(text).or_else(|_| Layout::from_kle_json(text))
}

/// A state change to run on the Qt thread.
type Update = Box<dyn FnOnce(Pin<&mut qobject::DeviceState>) + Send>;

/// Keep a connection to this device's Kanata alive, reconnecting until the
/// device is gone (its QObject no longer accepts updates).
fn spawn_connection(
    qt: cxx_qt::CxxQtThread<qobject::DeviceState>,
    shared_address: Arc<Mutex<String>>,
    config_path: Arc<Mutex<Option<PathBuf>>>,
    live: Arc<Live>,
) {
    thread::spawn(move || {
        // false means the QObject is gone (device removed or app quitting).
        let push = |f: Update| qt.queue(f).is_ok();
        loop {
            let address = shared_address.lock().map(|a| a.clone()).unwrap_or_default();
            let status = {
                let address = address.clone();
                Box::new(move |mut o: Pin<&mut qobject::DeviceState>| {
                    o.as_mut()
                        .set_status(QString::from(&format!("connecting to {address}…")))
                })
            };
            if !push(status) {
                return;
            }

            match Client::connect(address.as_str(), Duration::from_secs(1)) {
                Ok(mut client) => {
                    push(Box::new(|mut o| {
                        o.as_mut().set_connected(true);
                        o.as_mut().set_status(QString::from("connected"));
                        o.as_mut().relabel();
                        o.update_can_start();
                    }));
                    // A (re)started Kanata may have loaded an edited config, and it
                    // only sends ConfigFileReload for live reloads: re-read it here too.
                    let path = config_path.lock().ok().and_then(|p| p.clone());
                    if let Some(path) = &path {
                        push(reload_config(path, &path.to_string_lossy()));
                    }
                    for line in client.incoming() {
                        match line {
                            Ok(Incoming::Message(ServerMessage::LayerChange { new }))
                            | Ok(Incoming::Message(ServerMessage::CurrentLayerName {
                                name: new,
                            })) => {
                                let received = Instant::now();
                                let alive = push(Box::new(move |mut o| {
                                    o.as_mut().set_current_layer(QString::from(&new));
                                    o.as_mut().relabel();
                                    tracing::debug!(
                                        "labels updated {:?} after the layer change arrived",
                                        received.elapsed()
                                    );
                                }));
                                if !alive {
                                    return;
                                }
                            }
                            Ok(Incoming::Message(ServerMessage::ConfigFileReload { new })) => {
                                if let Some(path) = &path {
                                    push(reload_config(path, &new));
                                }
                            }
                            // Tap-hold results (feature 006): filtered like presses;
                            // the key name is never logged (VI).
                            Ok(Incoming::Message(ServerMessage::TapActivated { key })) => {
                                tap_hold_result(&live, &push, &key, TapHold::Tap);
                            }
                            Ok(Incoming::Message(ServerMessage::HoldActivated { key })) => {
                                tap_hold_result(&live, &push, &key, TapHold::Hold);
                            }
                            // Only the kind: some messages carry pressed-key names,
                            // which must not be logged (VI).
                            Ok(Incoming::Message(other)) => {
                                tracing::debug!("kanata: {} message", message_kind(&other));
                            }
                            Ok(other) => tracing::debug!("kanata: {other:?}"),
                            Err(e) => {
                                tracing::warn!("kanata connection error: {e}");
                                break;
                            }
                        }
                    }
                    let alive = push(Box::new(|mut o| {
                        o.as_mut().clear_live();
                        o.as_mut().set_connected(false);
                        o.as_mut().set_status(QString::from("disconnected"));
                        o.as_mut().relabel();
                        o.update_can_start();
                    }));
                    if !alive {
                        return;
                    }
                }
                Err(e) => tracing::debug!("connect to {address} failed: {e}"),
            }
            thread::sleep(Duration::from_secs(1));
        }
    });
}

/// Show a tap-hold result if the device's filter lets it through.
fn tap_hold_result(live: &Live, push: &impl Fn(Update) -> bool, key: &str, kind: TapHold) {
    let event = KeyEvent {
        key: kbd::canonical(key),
        action: KeyAction::Press,
    };
    // Never captured: press to bind waits for a real press.
    let routed = route(
        event,
        false,
        live.watching.load(Ordering::Relaxed),
        live.mode(),
    );
    if let Route::Show(event) = routed {
        push(Box::new(move |o| o.tap_hold_event(event.key, kind)));
    }
}

/// Re-read the config after Kanata (re)connected or reloaded (feature 002).
/// Runs on the connection thread; the result is applied on the Qt thread.
fn reload_config(path: &Path, kanata_now_reads: &str) -> Update {
    let result = kbd::read(path);
    let switched = std::fs::canonicalize(kanata_now_reads).ok() != std::fs::canonicalize(path).ok();
    let path = path.display().to_string();
    let kanata_now_reads = kanata_now_reads.to_owned();
    tracing::debug!("config reloaded");
    Box::new(move |mut o| {
        match result {
            Ok(config) => o.as_mut().use_config(config),
            // Keep the previous labels.
            Err(e) => o
                .as_mut()
                .add_notice(&format!("config: cannot read {path}: {e}")),
        }
        if switched {
            o.as_mut().add_notice(&format!(
                "Kanata switched to {kanata_now_reads}; the overlay still reads {path}"
            ));
        }
    })
}

/// A Kanata message's name without its contents.
fn message_kind(m: &ServerMessage) -> &'static str {
    match m {
        ServerMessage::LayerChange { .. } => "LayerChange",
        ServerMessage::LayerNames { .. } => "LayerNames",
        ServerMessage::FakeKeyNames { .. } => "FakeKeyNames",
        ServerMessage::CurrentLayerInfo { .. } => "CurrentLayerInfo",
        ServerMessage::ConfigFileReload { .. } => "ConfigFileReload",
        ServerMessage::CurrentLayerName { .. } => "CurrentLayerName",
        ServerMessage::MessagePush { .. } => "MessagePush",
        ServerMessage::Error { .. } => "Error",
        ServerMessage::HelloOk { .. } => "HelloOk",
        ServerMessage::ReloadResult { .. } => "ReloadResult",
        ServerMessage::HoldActivated { .. } => "HoldActivated",
        ServerMessage::TapActivated { .. } => "TapActivated",
    }
}
