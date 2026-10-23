//! The QObject QML talks to.
//!
//! All state lives in Rust; QML only renders it. A background thread owns the
//! Kanata TCP connection and pushes updates onto the Qt thread with
//! `qt_thread().queue(...)`, the only safe way to touch a QObject from another
//! thread.

use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use kanata_client::protocol::{ClientMessage, ServerMessage};
use kanata_client::{Client, Incoming};
use layout::Layout;

use crate::{Args, platform};

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
        #[qproperty(QString, current_layer)]
        #[qproperty(bool, connected)]
        #[qproperty(QString, status)]
        /// The physical layout as JSON (the `layout` crate's format); QML parses it.
        #[qproperty(QString, layout_json)]
        /// Wayland with layer-shell: use the real overlay (research R4).
        #[qproperty(bool, use_layer_shell)]
        /// Things the user should know, joined with " · " (FR-013, FR-017, …).
        #[qproperty(QString, notices)]
        /// Per-key labels for the active layer as JSON: canonical key name →
        /// `{text, style, hold, alias}` (contracts/labels.md); `{}` without a config.
        #[qproperty(QString, labels_json)]
        /// Toggled by the show/hide shortcut.
        #[qproperty(bool, overlay_shown)]
        /// `overlay_shown`, or the settings window is open (live preview).
        #[qproperty(bool, overlay_visible)]
        type OverlayState = super::OverlayStateRust;

        /// Called once by QML when the window is ready.
        #[qinvokable]
        fn start(self: Pin<&mut OverlayState>);

        /// Ask Kanata to switch layers (handy for testing the overlay).
        #[qinvokable]
        fn request_layer(self: Pin<&mut OverlayState>, name: &QString);

        /// Show or hide the overlay (the global shortcut).
        #[qinvokable]
        fn toggle_overlay(self: Pin<&mut OverlayState>);

        /// The settings window opened or closed: show the overlay while it is open.
        #[qinvokable]
        fn set_previewing(self: Pin<&mut OverlayState>, on: bool);

        /// Add a notice for the status line (duplicates are dropped).
        #[qinvokable]
        fn add_notice(self: Pin<&mut OverlayState>, text: &QString);
    }

    impl cxx_qt::Threading for OverlayState {}
}

pub struct OverlayStateRust {
    current_layer: QString,
    connected: bool,
    status: QString,
    layout_json: QString,
    use_layer_shell: bool,
    notices: QString,
    overlay_shown: bool,
    overlay_visible: bool,
    previewing: bool,
    notice_list: Vec<String>,
    labels_json: QString,
    config: Option<kbd::Config>,
    started: bool,
    args: Args,
}

impl Default for OverlayStateRust {
    fn default() -> Self {
        Self {
            current_layer: QString::default(),
            connected: false,
            status: QString::default(),
            layout_json: QString::default(),
            use_layer_shell: false,
            notices: QString::default(),
            // Always start shown (spec assumption).
            overlay_shown: true,
            overlay_visible: true,
            previewing: false,
            notice_list: Vec::new(),
            labels_json: QString::from("{}"),
            config: None,
            started: false,
            args: Args::default(),
        }
    }
}

impl qobject::OverlayState {
    fn start(mut self: Pin<&mut Self>) {
        if self.rust().started {
            return;
        }
        self.as_mut().rust_mut().started = true;
        let args = Args::from_env();
        self.as_mut().rust_mut().args = args.clone();

        let layout = match &args.layout {
            Some(path) => std::fs::read_to_string(path)
                .map_err(anyhow::Error::from)
                .and_then(|text| load_layout(&text).map_err(Into::into)),
            None => load_layout(include_str!("../../../examples/layouts/sample.kle.json"))
                .map_err(Into::into),
        };
        match layout {
            Ok(layout) => self
                .as_mut()
                .set_layout_json(QString::from(&with_canonical_names(&layout.to_json()))),
            Err(e) => self
                .as_mut()
                .set_status(QString::from(&format!("layout error: {e:#}"))),
        }

        let caps = platform::detect(&platform::qt_platform_name());
        tracing::info!("platform: {caps:?}");
        self.as_mut().set_use_layer_shell(caps.layer_shell);
        for missing in platform::missing_features(&caps) {
            self.as_mut().add_notice(&QString::from(missing));
        }

        if let Some(path) = &args.config {
            match kbd::read(path) {
                Ok(config) => self.as_mut().use_config(config),
                Err(e) => self.as_mut().add_notice(&QString::from(&format!(
                    "config: cannot read {}: {e}",
                    path.display()
                ))),
            }
        }

        let address = ADDRESS
            .get_or_init(|| Arc::new(Mutex::new(args.address.clone())))
            .clone();
        spawn_connection(self.qt_thread(), address, args.config.clone());
    }

    /// Take a freshly read config and relabel the board.
    fn use_config(mut self: Pin<&mut Self>, config: kbd::Config) {
        for w in &config.warnings {
            self.as_mut().add_notice(&QString::from(&w.to_string()));
        }
        self.as_mut().rust_mut().config = Some(config);
        self.relabel();
    }

    /// Recompute the labels: the current layer while connected, else the
    /// config's first layer (FR-010).
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
        let json: serde_json::Map<String, serde_json::Value> = labels
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
            .set_labels_json(QString::from(&serde_json::Value::Object(json).to_string()));
        for w in warnings {
            self.as_mut().add_notice(&QString::from(&w.to_string()));
        }
    }

    fn toggle_overlay(mut self: Pin<&mut Self>) {
        let shown = !*self.overlay_shown();
        self.as_mut().set_overlay_shown(shown);
        self.update_visible();
    }

    fn set_previewing(mut self: Pin<&mut Self>, on: bool) {
        self.as_mut().rust_mut().previewing = on;
        self.update_visible();
    }

    fn update_visible(mut self: Pin<&mut Self>) {
        let visible = *self.overlay_shown() || self.rust().previewing;
        tracing::debug!(
            "overlay shown: {}, previewing: {}",
            self.overlay_shown(),
            self.rust().previewing
        );
        self.as_mut().set_overlay_visible(visible);
    }

    fn add_notice(mut self: Pin<&mut Self>, text: &QString) {
        let text = text.to_string();
        if text.is_empty() || self.rust().notice_list.contains(&text) {
            return;
        }
        tracing::info!("notice: {text}");
        self.as_mut().rust_mut().notice_list.push(text);
        let joined = self.rust().notice_list.join(" · ");
        self.as_mut().set_notices(QString::from(&joined));
    }

    fn request_layer(self: Pin<&mut Self>, name: &QString) {
        let address = current_address();
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

/// Add each key's canonical Kanata name (`kanataCanonical`), so QML can look
/// up its label without knowing Kanata's key spellings.
fn with_canonical_names(layout_json: &str) -> String {
    let Ok(mut value) = serde_json::from_str::<serde_json::Value>(layout_json) else {
        return layout_json.to_owned();
    };
    if let Some(keys) = value.get_mut("keys").and_then(|k| k.as_array_mut()) {
        for key in keys {
            if let Some(name) = key.get("kanata").and_then(|k| k.as_str()) {
                let canonical = kbd::canonical(name);
                key["kanataCanonical"] = serde_json::Value::String(canonical);
            }
        }
    }
    value.to_string()
}

/// Accept either our own layout format or raw KLE JSON.
fn load_layout(text: &str) -> layout::Result<Layout> {
    Layout::from_json(text).or_else(|_| Layout::from_kle_json(text))
}

/// A state change to run on the Qt thread.
type Update = Box<dyn FnOnce(Pin<&mut qobject::OverlayState>) + Send>;

/// Where to connect to Kanata. `KanataState` changes it when it starts Kanata
/// on another port, or attaches somewhere else.
pub(crate) static ADDRESS: OnceLock<Arc<Mutex<String>>> = OnceLock::new();

pub(crate) fn current_address() -> String {
    ADDRESS
        .get()
        .and_then(|a| a.lock().ok().map(|a| a.clone()))
        .unwrap_or_else(|| Args::default().address)
}

pub(crate) fn set_address(address: String) {
    if let Some(a) = ADDRESS.get()
        && let Ok(mut a) = a.lock()
    {
        *a = address;
    }
}

/// Keep a connection to Kanata alive, reconnecting forever.
fn spawn_connection(
    qt: cxx_qt::CxxQtThread<qobject::OverlayState>,
    shared_address: Arc<Mutex<String>>,
    config_path: Option<PathBuf>,
) {
    thread::spawn(move || {
        let push = |f: Update| {
            // Err means the QObject is gone (app quitting): nothing to do.
            let _ = qt.queue(f);
        };
        loop {
            let address = shared_address.lock().map(|a| a.clone()).unwrap_or_default();
            push(Box::new({
                let address = address.clone();
                move |mut o| {
                    o.as_mut()
                        .set_status(QString::from(&format!("connecting to {address}…")))
                }
            }));

            match Client::connect(address.as_str(), Duration::from_secs(1)) {
                Ok(mut client) => {
                    push(Box::new(|mut o| {
                        o.as_mut().set_connected(true);
                        o.as_mut().set_status(QString::from("connected"));
                        o.relabel();
                    }));
                    // A (re)started Kanata may have loaded an edited config, and it
                    // only sends ConfigFileReload for live reloads: re-read it here too.
                    if let Some(path) = &config_path {
                        push(reload_config(path, &path.to_string_lossy()));
                    }
                    for line in client.incoming() {
                        match line {
                            Ok(Incoming::Message(ServerMessage::LayerChange { new }))
                            | Ok(Incoming::Message(ServerMessage::CurrentLayerName {
                                name: new,
                            })) => {
                                let received = Instant::now();
                                push(Box::new(move |mut o| {
                                    o.as_mut().set_current_layer(QString::from(&new));
                                    o.as_mut().relabel();
                                    tracing::debug!(
                                        "labels updated {:?} after the layer change arrived",
                                        received.elapsed()
                                    );
                                }));
                            }
                            Ok(Incoming::Message(ServerMessage::ConfigFileReload { new })) => {
                                if let Some(path) = &config_path {
                                    push(reload_config(path, &new));
                                }
                            }
                            // Only the kind: some messages (HoldActivated, TapActivated)
                            // carry pressed-key names, which must not be logged (VI).
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
                    push(Box::new(|mut o| {
                        o.as_mut().set_connected(false);
                        o.as_mut().set_status(QString::from("disconnected"));
                        o.relabel();
                    }));
                }
                Err(e) => tracing::debug!("connect to {address} failed: {e}"),
            }
            thread::sleep(Duration::from_secs(1));
        }
    });
}

/// Re-read the config after Kanata (re)connected or reloaded (FR-011, FR-012).
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
                .add_notice(&QString::from(&format!("config: cannot read {path}: {e}"))),
        }
        if switched {
            o.as_mut().add_notice(&QString::from(&format!(
                "Kanata switched to {kanata_now_reads}; the overlay still reads {path}"
            )));
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
