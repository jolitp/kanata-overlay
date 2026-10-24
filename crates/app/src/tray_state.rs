//! One system tray icon per device (feature 007, research R1–R3), through
//! `ksni` (StatusNotifierItem without Qt Widgets). Each icon shows its device's
//! layer as a short label drawn into the pixmap; every icon has the same menu.
//! Menu actions only queue signals to the Qt thread; QML connects them to the
//! existing actions.

use std::collections::BTreeMap;
use std::pin::Pin;

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use ksni::blocking::{Handle, TrayMethods};
use ksni::menu::{CheckmarkItem, StandardItem, SubMenu};
use ksni::{Icon, MenuItem, ToolTip, Tray};
use serde_json::Value;

/// Icon size in pixels; the tray scales it.
const ICON_SIZE: i32 = 32;

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
        /// False when no system tray answered (FR-004).
        #[qproperty(bool, available)]
        type TrayState = super::TrayStateRust;

        /// Every device as JSON `[{id, name, layer, state, connected, managed, compact}]`;
        /// icons are created, updated and dropped to match.
        #[qinvokable]
        fn update(self: Pin<&mut TrayState>, devices: &QString);

        /// Show/Hide all boards was chosen.
        #[qsignal]
        fn toggle_requested(self: Pin<&mut TrayState>);
        /// An icon was clicked, or Show/Hide <device> chosen: that board only.
        #[qsignal]
        fn device_toggle_requested(self: Pin<&mut TrayState>, device_id: QString);
        /// Settings… on this device's icon.
        #[qsignal]
        fn settings_requested(self: Pin<&mut TrayState>, device_id: QString);
        #[qsignal]
        fn restart_requested(self: Pin<&mut TrayState>, device_id: QString);
        /// Compact board ticked or unticked on this device's icon.
        #[qsignal]
        fn compact_requested(self: Pin<&mut TrayState>, device_id: QString, compact: bool);
        #[qsignal]
        fn quit_requested(self: Pin<&mut TrayState>);
    }

    impl cxx_qt::Threading for TrayState {}
}

/// The parts of the tray that are plain logic (tested).
pub mod model {
    /// The icon's label (research R2): the layer's first 4 characters with the
    /// first one uppercase; "–" when not connected or no layer.
    pub fn short_label(layer: &str, connected: bool) -> String {
        let layer = layer.trim();
        if !connected || layer.is_empty() {
            return "–".to_owned();
        }
        let mut chars = layer.chars().take(4);
        let first: String = chars
            .next()
            .into_iter()
            .flat_map(char::to_uppercase)
            .collect();
        first + &chars.collect::<String>()
    }

    /// Tooltip (title, text): the device name, then "layer · state".
    pub fn tooltip(name: &str, layer: &str, state: &str, connected: bool) -> (String, String) {
        let layer = if connected && !layer.is_empty() {
            layer
        } else {
            "not connected"
        };
        let text = if state.is_empty() || state == layer {
            layer.to_owned()
        } else {
            format!("{layer} · {state}")
        };
        (name.to_owned(), text)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn labels() {
            assert_eq!(short_label("base", true), "Base");
            assert_eq!(short_label("nav", true), "Nav");
            assert_eq!(short_label("symbols", true), "Symb");
            assert_eq!(short_label("12", true), "12");
            assert_eq!(short_label("élan", true), "Élan");
            assert_eq!(short_label("base", false), "–");
            assert_eq!(short_label("", true), "–");
        }

        #[test]
        fn tooltips() {
            assert_eq!(
                tooltip("defy", "nav", "managed", true),
                ("defy".into(), "nav · managed".into())
            );
            assert_eq!(
                tooltip("mouse", "base", "starting Kanata…", false),
                ("mouse".into(), "not connected · starting Kanata…".into())
            );
            assert_eq!(tooltip("x", "base", "", true).1, "base");
        }
    }
}

/// One device's icon, served on its own thread by `ksni`.
struct DeviceTray {
    id: String,
    name: String,
    tooltip: (String, String),
    icon: Icon,
    /// Managed devices (id, name), for the Restart Kanata submenu.
    managed: Vec<(String, String)>,
    /// This device's board is compact.
    compact: bool,
    qt: cxx_qt::CxxQtThread<qobject::TrayState>,
}

impl Tray for DeviceTray {
    fn id(&self) -> String {
        format!("kanata-overlay-{}", self.id)
    }

    fn title(&self) -> String {
        self.name.clone()
    }

    fn icon_pixmap(&self) -> Vec<Icon> {
        vec![self.icon.clone()]
    }

    fn tool_tip(&self) -> ToolTip {
        ToolTip {
            title: self.tooltip.0.clone(),
            description: self.tooltip.1.clone(),
            ..Default::default()
        }
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        let id = QString::from(&self.id);
        let _ = self.qt.queue(move |o| o.device_toggle_requested(id));
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let restart: Vec<MenuItem<Self>> = self
            .managed
            .iter()
            .map(|(id, name)| {
                let id = id.clone();
                StandardItem {
                    label: name.clone(),
                    activate: Box::new(move |t: &mut Self| {
                        let id = QString::from(&id);
                        let _ = t.qt.queue(move |o| o.restart_requested(id));
                    }),
                    ..Default::default()
                }
                .into()
            })
            .collect();
        vec![
            StandardItem {
                label: format!("Show/Hide {}", self.name),
                activate: Box::new(|t: &mut Self| {
                    let id = QString::from(&t.id);
                    let _ = t.qt.queue(move |o| o.device_toggle_requested(id));
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Show/Hide all boards".into(),
                activate: Box::new(|t: &mut Self| {
                    let _ = t.qt.queue(|o| o.toggle_requested());
                }),
                ..Default::default()
            }
            .into(),
            CheckmarkItem {
                label: "Compact board".into(),
                checked: self.compact,
                activate: Box::new(|t: &mut Self| {
                    let id = QString::from(&t.id);
                    let compact = !t.compact;
                    let _ = t.qt.queue(move |o| o.compact_requested(id, compact));
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: format!("Settings… ({})", self.name),
                activate: Box::new(|t: &mut Self| {
                    let id = QString::from(&t.id);
                    let _ = t.qt.queue(move |o| o.settings_requested(id));
                }),
                ..Default::default()
            }
            .into(),
            SubMenu {
                label: "Restart Kanata".into(),
                enabled: !restart.is_empty(),
                submenu: restart,
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Quit".into(),
                activate: Box::new(|t: &mut Self| {
                    let _ = t.qt.queue(|o| o.quit_requested());
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

struct Entry {
    handle: Handle<DeviceTray>,
    label: String,
    connected: bool,
    color: (u8, u8, u8),
}

pub struct TrayStateRust {
    available: bool,
    trays: BTreeMap<String, Entry>,
    /// A spawn failed: no tray; don't keep trying for every update.
    gave_up: bool,
}

impl Default for TrayStateRust {
    fn default() -> Self {
        Self {
            available: true,
            trays: BTreeMap::new(),
            gave_up: false,
        }
    }
}

fn icon(label: &str, connected: bool, color: (u8, u8, u8)) -> Icon {
    let (r, g, b) = color;
    Icon {
        width: ICON_SIZE,
        height: ICON_SIZE,
        data: crate::platform::ffi::kog_tray_icon(
            &QString::from(label),
            connected,
            r.into(),
            g.into(),
            b.into(),
            ICON_SIZE,
        ),
    }
}

impl qobject::TrayState {
    fn update(mut self: Pin<&mut Self>, devices: &QString) {
        let devices: Vec<Value> = serde_json::from_str(&devices.to_string()).unwrap_or_default();
        let text = |d: &Value, k: &str| d.get(k).and_then(Value::as_str).unwrap_or("").to_owned();
        let flag = |d: &Value, k: &str| d.get(k).and_then(Value::as_bool).unwrap_or(false);
        let managed: Vec<(String, String)> = devices
            .iter()
            .filter(|d| flag(d, "managed"))
            .map(|d| (text(d, "id"), text(d, "name")))
            .collect();
        let ids: Vec<String> = devices.iter().map(|d| text(d, "id")).collect();
        // Devices that are gone take their icons with them.
        self.as_mut().rust_mut().trays.retain(|id, e| {
            let keep = ids.contains(id);
            if !keep {
                e.handle.shutdown();
            }
            keep
        });
        for d in &devices {
            let (id, name, layer, state) = (
                text(d, "id"),
                text(d, "name"),
                text(d, "layer"),
                text(d, "state"),
            );
            let connected = flag(d, "connected");
            let compact = flag(d, "compact");
            let label = model::short_label(&layer, connected);
            let color = crate::layer_color::layer_color(&layer);
            let tooltip = model::tooltip(&name, &layer, &state, connected);
            if let Some(entry) = self.as_mut().rust_mut().trays.get_mut(&id) {
                // Redraw only when the picture changes.
                let new_icon =
                    (entry.label != label || entry.connected != connected || entry.color != color)
                        .then(|| icon(&label, connected, color));
                entry.label = label;
                entry.connected = connected;
                entry.color = color;
                let managed = managed.clone();
                entry.handle.update(move |t| {
                    t.compact = compact;
                    t.name = name;
                    t.tooltip = tooltip;
                    t.managed = managed;
                    if let Some(i) = new_icon {
                        t.icon = i;
                    }
                });
                continue;
            }
            if self.rust().gave_up {
                continue;
            }
            let tray = DeviceTray {
                id: id.clone(),
                name,
                tooltip,
                icon: icon(&label, connected, color),
                managed: managed.clone(),
                compact,
                qt: self.qt_thread(),
            };
            match tray.spawn() {
                Ok(handle) => {
                    self.as_mut().rust_mut().trays.insert(
                        id,
                        Entry {
                            handle,
                            label,
                            connected,
                            color,
                        },
                    );
                }
                Err(e) => {
                    tracing::warn!("no system tray: {e}");
                    self.as_mut().rust_mut().gave_up = true;
                    self.as_mut().set_available(false);
                }
            }
        }
    }
}
