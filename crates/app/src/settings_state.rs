//! The settings as QML sees them: the device list, each device's settings and
//! appearance (with CLI overrides when there is one device), and the device
//! check (contracts/interfaces.md › SettingsState).
//!
//! All rules live in the `settings`, `kbd` and `kanata-client` crates; this is
//! only the bridge.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::OnceLock;

use cxx_qt::CxxQtType;
use cxx_qt_lib::{QRectF, QSizeF, QString};
use serde_json::{Value, json};
use settings::{Device, Edge, Field, Overrides, Rect, Settings, Size};

use crate::Args;

/// Command-line overrides, parsed in `main` before Qt starts.
pub static OVERRIDES: OnceLock<Overrides> = OnceLock::new();

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qrectf.h");
        type QRectF = cxx_qt_lib::QRectF;
        include!("cxx-qt-lib/qsizef.h");
        type QSizeF = cxx_qt_lib::QSizeF;
    }

    #[auto_cxx_name]
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        /// JSON array of device ids, in order: the model of Main.qml's Instantiator.
        #[qproperty(QString, device_ids)]
        /// Bumped on every device change; bindings touch it (feature 001's lesson).
        #[qproperty(i32, revision)]
        /// The settings file in use, for messages.
        #[qproperty(QString, path)]
        type SettingsState = super::SettingsStateRust;

        /// Read the file, apply the command-line overrides, check the devices.
        #[qinvokable]
        fn load(self: Pin<&mut SettingsState>);

        /// One device's settings as JSON (id, name, config, layout, address,
        /// edge, margin, opacity 0–1, scale %, screen, index).
        #[qinvokable]
        fn device(self: &SettingsState, id: &QString) -> QString;

        /// The device check for one device as JSON `{blocked, warnings}`.
        #[qinvokable]
        fn device_check(self: &SettingsState, id: &QString) -> QString;

        /// Re-run the device check (configs edited, devices plugged in or out);
        /// bumps the revision only if a result changed.
        #[qinvokable]
        fn recheck(self: Pin<&mut SettingsState>);

        /// Add a device; returns its id.
        #[qinvokable]
        fn add_device(self: Pin<&mut SettingsState>, name: &QString) -> QString;

        #[qinvokable]
        fn remove_device(self: Pin<&mut SettingsState>, id: &QString);

        /// Change one field: name, config, layout, address, edge, margin,
        /// opacity (percent), scale (percent) or screen. Live; saved by `save()`.
        #[qinvokable]
        fn update_device(
            self: Pin<&mut SettingsState>,
            id: &QString,
            field: &QString,
            value: &QString,
        );

        /// Write the file if anything changed since the last save.
        #[qinvokable]
        fn save(self: Pin<&mut SettingsState>) -> bool;

        /// Window geometry for a device's board on a screen, as JSON
        /// `{x, y, width, height, scale, capped}`.
        #[qinvokable]
        fn placement(self: &SettingsState, id: &QString, screen: &QRectF, base: &QSizeF)
        -> QString;

        /// Something about the settings the user should know.
        #[qsignal]
        fn warning(self: Pin<&mut SettingsState>, text: QString);
    }
}

#[derive(Default)]
pub struct SettingsStateRust {
    device_ids: QString,
    revision: i32,
    path: QString,

    /// What is (or will be) saved.
    file: Settings,
    /// The devices shown this run (file devices, or the implicit one).
    devices: Vec<Device>,
    /// `devices` came from the command line, not the file (not saved yet).
    implicit: bool,
    overrides: Overrides,
    cli_config: Option<PathBuf>,
    cli_layout: Option<PathBuf>,
    cli_address: String,
    checks: HashMap<String, Value>,
    file_path: Option<PathBuf>,
    dirty: bool,
}

impl qobject::SettingsState {
    fn load(mut self: Pin<&mut Self>) {
        let overrides = OVERRIDES.get().cloned().unwrap_or_default();
        let args = Args::from_env();
        let path = overrides
            .settings_path
            .clone()
            .or_else(settings::default_path);
        let outcome = match &path {
            Some(path) => settings::load(path),
            None => settings::LoadOutcome {
                settings: Settings::default(),
                warnings: Vec::new(),
                writable: false,
            },
        };
        if path.is_none() {
            self.as_mut().warning(QString::from(
                "settings: neither XDG_CONFIG_HOME nor HOME is set, settings will not be saved",
            ));
        }
        for w in &outcome.warnings {
            tracing::warn!("{w}");
            self.as_mut().warning(QString::from(&w.to_string()));
        }

        let display = path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        {
            let mut rust = self.as_mut().rust_mut();
            rust.file = outcome.settings;
            rust.overrides = overrides;
            rust.cli_config = args.config;
            rust.cli_layout = args.layout;
            rust.cli_address = args.address;
            rust.file_path = path;
            rust.dirty = false;
        }
        self.as_mut().set_path(QString::from(&display));
        self.refresh(true);
    }

    fn device(&self, id: &QString) -> QString {
        let id = id.to_string();
        let rust = self.rust();
        let Some((index, d)) = rust.devices.iter().enumerate().find(|(_, d)| d.id == id) else {
            return QString::from("{}");
        };
        let path = |p: &Option<PathBuf>| p.as_ref().map(|p| p.display().to_string());
        let json = json!({
            "id": d.id,
            "name": d.name,
            "config": path(&d.config),
            "layout": path(&d.layout),
            // The first device keeps `--connect` (default 127.0.0.1:7070, FR-002);
            // the others default to 7070 + index (research R4).
            "address": d.address.clone().unwrap_or_else(|| if index == 0 {
                rust.cli_address.clone()
            } else {
                settings::default_address(index)
            }),
            "customAddress": d.address,
            "edge": d.appearance.edge.as_str(),
            "margin": d.appearance.margin,
            "opacity": f64::from(d.appearance.opacity) / 100.0,
            "scale": d.appearance.scale,
            "screen": d.appearance.screen.clone().unwrap_or_default(),
            "highlight": d.highlight.as_str(),
            "style": d.style.as_str(),
            "index": index,
        });
        QString::from(&json.to_string())
    }

    fn recheck(mut self: Pin<&mut Self>) {
        let checks = check_devices(&self.rust().devices);
        if checks != self.rust().checks {
            self.as_mut().rust_mut().checks = checks;
            let revision = self.revision() + 1;
            self.as_mut().set_revision(revision);
        }
    }

    fn device_check(&self, id: &QString) -> QString {
        let check = self.rust().checks.get(&id.to_string()).cloned();
        QString::from(&check.unwrap_or_else(|| json!({})).to_string())
    }

    fn add_device(mut self: Pin<&mut Self>, name: &QString) -> QString {
        self.as_mut().materialize();
        let id = self.as_mut().rust_mut().file.add_device(&name.to_string());
        self.as_mut().rust_mut().dirty = true;
        self.as_mut().save();
        self.refresh(true);
        QString::from(&id)
    }

    fn remove_device(mut self: Pin<&mut Self>, id: &QString) {
        self.as_mut().materialize();
        self.as_mut().rust_mut().file.remove_device(&id.to_string());
        self.as_mut().rust_mut().dirty = true;
        self.as_mut().save();
        self.refresh(true);
    }

    fn update_device(mut self: Pin<&mut Self>, id: &QString, field: &QString, value: &QString) {
        let (id, field, value) = (id.to_string(), field.to_string(), value.to_string());
        self.as_mut().materialize();
        let structural = matches!(field.as_str(), "config" | "layout" | "address");
        let number = |v: &str| v.trim().parse::<f64>().ok().map(|n| n.round() as i64);
        let home = std::env::var_os("HOME").map(PathBuf::from);
        let path = |v: &str| settings::typed_path(v, home.as_deref());
        {
            let mut rust = self.as_mut().rust_mut();
            let single = rust.file.devices.len() == 1;
            let Some(d) = rust.file.device_mut(&id) else {
                return;
            };
            let a = &mut d.appearance;
            let cleared = match field.as_str() {
                "name" if !value.trim().is_empty() => {
                    d.name = value.trim().chars().take(40).collect();
                    None
                }
                "config" => {
                    d.config = path(&value);
                    None
                }
                "layout" => {
                    d.layout = path(&value);
                    None
                }
                "address" => {
                    d.address = (!value.trim().is_empty()).then(|| value.trim().to_owned());
                    None
                }
                "style" => {
                    let Some(v) = settings::Style::parse(&value) else {
                        return;
                    };
                    d.style = v;
                    None
                }
                "highlight" => {
                    let Some(h) = settings::Highlight::parse(&value) else {
                        return;
                    };
                    d.highlight = h;
                    None
                }
                "edge" => {
                    let Ok(edge) = value.parse::<Edge>() else {
                        return;
                    };
                    a.edge = edge;
                    Some(Field::Edge)
                }
                "margin" => {
                    let Some(n) = number(&value) else { return };
                    let r = settings::MARGIN_RANGE;
                    a.margin = (n.max(0) as u32).clamp(*r.start(), *r.end());
                    Some(Field::Margin)
                }
                "opacity" => {
                    let Some(n) = number(&value) else { return };
                    let r = settings::OPACITY_RANGE;
                    a.opacity = n.clamp(i64::from(*r.start()), i64::from(*r.end())) as u8;
                    Some(Field::Opacity)
                }
                "scale" => {
                    let Some(n) = number(&value) else { return };
                    let r = settings::SCALE_RANGE;
                    a.scale = n.clamp(i64::from(*r.start()), i64::from(*r.end())) as u16;
                    Some(Field::Scale)
                }
                "screen" => {
                    a.screen = (!value.is_empty()).then_some(value.clone());
                    Some(Field::Screen)
                }
                _ => return,
            };
            // A user edit wins over the command line (feature 001, FR-011d).
            if let Some(f) = cleared
                && single
            {
                rust.overrides.clear(f);
            }
            rust.dirty = true;
        }
        self.refresh(structural);
    }

    fn save(mut self: Pin<&mut Self>) -> bool {
        if !self.rust().dirty || self.rust().implicit {
            return true;
        }
        let Some(path) = self.rust().file_path.clone() else {
            return false;
        };
        match settings::save(&path, &self.rust().file) {
            Ok(()) => {
                self.as_mut().rust_mut().dirty = false;
                true
            }
            Err(e) => {
                tracing::warn!("could not save settings to {}: {e}", path.display());
                self.as_mut()
                    .warning(QString::from(&format!("could not save settings: {e}")));
                false
            }
        }
    }

    fn placement(&self, id: &QString, screen: &QRectF, base: &QSizeF) -> QString {
        let id = id.to_string();
        let appearance = self
            .rust()
            .devices
            .iter()
            .find(|d| d.id == id)
            .map(|d| d.appearance.clone())
            .unwrap_or_default();
        let p = settings::place(
            Rect {
                x: screen.x() as i32,
                y: screen.y() as i32,
                width: screen.width() as i32,
                height: screen.height() as i32,
            },
            Size {
                width: base.width().ceil() as i32,
                height: base.height().ceil() as i32,
            },
            appearance.edge,
            appearance.margin,
            appearance.scale,
        );
        let json = json!({
            "x": p.x,
            "y": p.y,
            "width": p.size.width,
            "height": p.size.height,
            "scale": p.scale,
            "capped": p.capped,
        });
        QString::from(&json.to_string())
    }

    /// The implicit device is about to be edited: make it a saved device.
    fn materialize(mut self: Pin<&mut Self>) {
        if self.rust().implicit {
            let mut rust = self.as_mut().rust_mut();
            rust.file.devices = rust.devices.clone();
            rust.implicit = false;
        }
    }

    /// Recompute the shown devices (and, if `check`, the device check), then
    /// publish ids and bump the revision.
    fn refresh(mut self: Pin<&mut Self>, check: bool) {
        {
            let mut rust = self.as_mut().rust_mut();
            let devices = rust.file.effective_devices(
                &rust.overrides,
                rust.cli_config.as_deref(),
                rust.cli_layout.as_deref(),
            );
            rust.implicit = rust.file.devices.is_empty();
            rust.devices = devices;
            if check {
                rust.checks = check_devices(&rust.devices);
            }
        }
        let ids: Vec<&str> = self.rust().devices.iter().map(|d| d.id.as_str()).collect();
        let ids = QString::from(&serde_json::to_string(&ids).unwrap_or_default());
        if ids != *self.device_ids() {
            self.as_mut().set_device_ids(ids);
        }
        let revision = self.revision() + 1;
        self.as_mut().set_revision(revision);
    }
}

/// Run the device check (feature 004, FR-011) over the devices with a readable
/// config. The rules are in `kanata_client::check_devices`.
fn check_devices(devices: &[Device]) -> HashMap<String, Value> {
    let selections: Vec<(String, String, kbd::DeviceSelection)> = devices
        .iter()
        .filter_map(|d| {
            let config = d.config.as_ref()?;
            let parsed = kbd::read(config).ok()?;
            let selection = kanata_client::resolve_links(&parsed.devices);
            Some((d.id.clone(), file_name(config), selection))
        })
        .collect();
    let present = match kanata_client::input_devices() {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!("cannot read the input device list: {e}");
            return HashMap::new();
        }
    };
    // With a single device there is nothing to overlap with, and an unlimited
    // config is today's normal single-device setup: only warn with several.
    let several = devices.len() > 1;
    kanata_client::check_devices(&selections, &present)
        .into_iter()
        .map(|v| {
            let warnings: Vec<&String> = if several {
                v.warnings.iter().collect()
            } else {
                Vec::new()
            };
            let blocked = v.blocked_by.as_ref().map(|b| {
                devices
                    .iter()
                    .find(|d| &d.id == b)
                    .map(|d| d.name.clone())
                    .unwrap_or_else(|| b.clone())
            });
            (
                v.id.clone(),
                json!({ "blocked": blocked, "warnings": warnings }),
            )
        })
        .collect()
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}
