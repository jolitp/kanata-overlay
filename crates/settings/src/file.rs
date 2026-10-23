//! The settings file: `$XDG_CONFIG_HOME/kanata-overlay/settings.json`.
//!
//! Version 2 holds a list of devices, each with its own appearance. A version 1
//! file (one board's appearance at the top level) is read as one device, `main`.
//!
//! Loading never fails: every bad field falls back to its default with a
//! warning, and a bad device is skipped. Saving is atomic (temp file, rename).

use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

use crate::{
    Appearance, Device, Field, MARGIN_RANGE, OPACITY_RANGE, SCALE_RANGE, Settings, VERSION,
    in_range,
};

/// Something the user should know about the settings file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsWarning {
    InvalidField { field: String, reason: String },
    InvalidDevice { index: usize, reason: String },
    Unreadable { reason: String },
    NotJson { reason: String },
    NewerVersion { found: u64 },
}

impl fmt::Display for SettingsWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SettingsWarning::InvalidField { field, reason } => {
                write!(f, "settings: invalid {field} ({reason}), using the default")
            }
            SettingsWarning::InvalidDevice { index, reason } => {
                write!(f, "settings: device {} skipped ({reason})", index + 1)
            }
            SettingsWarning::Unreadable { reason } => {
                write!(f, "settings file unreadable ({reason}), using defaults")
            }
            SettingsWarning::NotJson { reason } => {
                write!(
                    f,
                    "settings file is not valid JSON ({reason}), using defaults"
                )
            }
            SettingsWarning::NewerVersion { found } => write!(
                f,
                "settings file is version {found} (newer than {VERSION}), using defaults"
            ),
        }
    }
}

/// The result of reading the settings file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadOutcome {
    pub settings: Settings,
    pub warnings: Vec<SettingsWarning>,
    /// `false` when saving would overwrite something we could not understand.
    pub writable: bool,
}

/// The default settings path, or `None` if neither `XDG_CONFIG_HOME` nor `HOME` is set.
pub fn default_path() -> Option<PathBuf> {
    path_from(
        std::env::var_os("XDG_CONFIG_HOME"),
        std::env::var_os("HOME"),
    )
}

fn path_from(xdg_config_home: Option<OsString>, home: Option<OsString>) -> Option<PathBuf> {
    let base = match xdg_config_home.filter(|v| !v.is_empty()) {
        Some(dir) => PathBuf::from(dir),
        None => PathBuf::from(home.filter(|v| !v.is_empty())?).join(".config"),
    };
    Some(base.join("kanata-overlay").join("settings.json"))
}

/// Read the settings file. A missing file gives no devices, silently.
pub fn load(path: &Path) -> LoadOutcome {
    match fs::read_to_string(path) {
        Ok(text) => parse(&text),
        Err(e) if e.kind() == io::ErrorKind::NotFound => LoadOutcome {
            settings: Settings::default(),
            warnings: Vec::new(),
            writable: true,
        },
        Err(e) => unusable(SettingsWarning::Unreadable {
            reason: e.to_string(),
        }),
    }
}

fn unusable(warning: SettingsWarning) -> LoadOutcome {
    LoadOutcome {
        settings: Settings::default(),
        warnings: vec![warning],
        writable: false,
    }
}

/// Parse settings JSON (version 1 or 2), validating each field on its own.
pub(crate) fn parse(text: &str) -> LoadOutcome {
    let object = match serde_json::from_str::<Value>(text) {
        Ok(Value::Object(object)) => object,
        Ok(_) => {
            return unusable(SettingsWarning::NotJson {
                reason: "expected an object".to_owned(),
            });
        }
        Err(e) => {
            return unusable(SettingsWarning::NotJson {
                reason: e.to_string(),
            });
        }
    };

    let version = object.get("version").and_then(Value::as_u64).unwrap_or(1);
    if version > u64::from(VERSION) {
        return unusable(SettingsWarning::NewerVersion { found: version });
    }

    let mut warnings = Vec::new();
    let devices = match object.get("devices") {
        Some(Value::Array(list)) => {
            let mut devices = Vec::new();
            for (index, entry) in list.iter().enumerate() {
                match parse_device(entry, index, &mut warnings) {
                    Ok(device) => devices.push(device),
                    Err(reason) => warnings.push(SettingsWarning::InvalidDevice { index, reason }),
                }
            }
            dedupe_ids(&mut devices);
            devices
        }
        Some(_) => {
            warnings.push(SettingsWarning::InvalidField {
                field: "devices".to_owned(),
                reason: "not a list".to_owned(),
            });
            Vec::new()
        }
        // Version 1: the top level is one board's appearance.
        None if version == 1 => vec![Device {
            id: "main".to_owned(),
            name: "Keyboard".to_owned(),
            config: None,
            layout: None,
            address: None,
            appearance: parse_appearance(&object, "", &mut warnings),
        }],
        None => Vec::new(),
    };

    LoadOutcome {
        settings: Settings {
            version: VERSION,
            devices,
        },
        warnings,
        writable: true,
    }
}

fn parse_device(
    entry: &Value,
    index: usize,
    warnings: &mut Vec<SettingsWarning>,
) -> Result<Device, String> {
    let Value::Object(object) = entry else {
        return Err("not an object".to_owned());
    };
    let id = object
        .get("id")
        .and_then(Value::as_str)
        .map(crate::slug)
        .filter(|id| !id.is_empty())
        .ok_or("missing id")?;
    let name = object
        .get("name")
        .and_then(Value::as_str)
        .map(|n| n.trim().chars().take(40).collect::<String>())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| id.clone());
    let path = |key: &str| {
        object
            .get(key)
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
    };
    let address = object
        .get("address")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_owned);
    let prefix = format!("devices[{}].", index + 1);
    Ok(Device {
        appearance: parse_appearance(object, &prefix, warnings),
        id,
        name,
        config: path("config"),
        layout: path("layout"),
        address,
    })
}

/// Duplicate ids get `-2`, `-3`, … (data-model › Device).
fn dedupe_ids(devices: &mut [Device]) {
    let mut seen: Vec<String> = Vec::new();
    for device in devices.iter_mut() {
        let base = device.id.clone();
        let mut n = 2;
        while seen.contains(&device.id) {
            device.id = format!("{base}-{n}");
            n += 1;
        }
        seen.push(device.id.clone());
    }
}

/// Read edge, margin, opacity, scale and screen from `object`; each bad field
/// falls back to its default with a warning naming `prefix + field`.
fn parse_appearance(
    object: &Map<String, Value>,
    prefix: &str,
    warnings: &mut Vec<SettingsWarning>,
) -> Appearance {
    let mut appearance = Appearance::default();
    let mut apply = |field: Field, outcome: Result<Option<()>, String>| {
        if let Err(reason) = outcome {
            warnings.push(SettingsWarning::InvalidField {
                field: format!("{prefix}{}", field.name()),
                reason,
            });
        }
    };

    apply(
        Field::Edge,
        read(object, Field::Edge, |v| {
            appearance.edge = v.as_str().ok_or("not a string")?.parse()?;
            Ok(())
        }),
    );
    apply(
        Field::Margin,
        read(object, Field::Margin, |v| {
            appearance.margin = in_range(whole::<u32>(v, &MARGIN_RANGE)?, &MARGIN_RANGE)?;
            Ok(())
        }),
    );
    apply(
        Field::Opacity,
        read(object, Field::Opacity, |v| {
            appearance.opacity = in_range(whole::<u8>(v, &OPACITY_RANGE)?, &OPACITY_RANGE)?;
            Ok(())
        }),
    );
    apply(
        Field::Scale,
        read(object, Field::Scale, |v| {
            appearance.scale = in_range(whole::<u16>(v, &SCALE_RANGE)?, &SCALE_RANGE)?;
            Ok(())
        }),
    );
    apply(
        Field::Screen,
        read(object, Field::Screen, |v| {
            appearance.screen = match v {
                Value::Null => None,
                Value::String(s) => crate::screen_name(s),
                _ => return Err("not a string or null".to_owned()),
            };
            Ok(())
        }),
    );
    appearance
}

/// Run `check` on the field if present: `Ok(None)` if absent, `Err` if invalid.
fn read<T>(
    object: &Map<String, Value>,
    field: Field,
    check: impl FnOnce(&Value) -> Result<T, String>,
) -> Result<Option<T>, String> {
    object.get(field.name()).map(check).transpose()
}

/// A non-negative whole number that fits `T` (else reported against `range`).
fn whole<T: TryFrom<u64> + std::fmt::Display>(
    v: &Value,
    range: &std::ops::RangeInclusive<T>,
) -> Result<T, String> {
    let n = v.as_u64().ok_or("not a whole number in range")?;
    T::try_from(n).map_err(|_| format!("{n} is outside {}–{}", range.start(), range.end()))
}

fn device_json(d: &Device) -> Value {
    let path = |p: &Option<PathBuf>| p.as_ref().map(|p| p.display().to_string());
    json!({
        "id": d.id,
        "name": d.name,
        "config": path(&d.config),
        "layout": path(&d.layout),
        "address": d.address,
        "edge": d.appearance.edge,
        "margin": d.appearance.margin,
        "opacity": d.appearance.opacity,
        "scale": d.appearance.scale,
        "screen": d.appearance.screen,
    })
}

/// Write the settings (always version 2) atomically, creating the directory if needed.
pub fn save(path: &Path, settings: &Settings) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let value = json!({
        "version": VERSION,
        "devices": settings.devices.iter().map(device_json).collect::<Vec<_>>(),
    });
    let mut text = serde_json::to_string_pretty(&value).map_err(io::Error::other)?;
    text.push('\n');

    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    fs::write(&tmp, text)?;
    fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Edge;

    /// A fresh, empty directory under the system temp dir.
    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "kanata-overlay-settings-{}-{name}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn device(id: &str, appearance: Appearance) -> Device {
        Device {
            id: id.to_owned(),
            name: id.to_owned(),
            config: Some(PathBuf::from(format!("/k/{id}.kbd"))),
            layout: None,
            address: None,
            appearance,
        }
    }

    /// The appearance of the only device of a v1 file.
    fn v1(text: &str) -> (Appearance, Vec<SettingsWarning>) {
        let out = parse(text);
        assert_eq!(out.settings.devices.len(), 1, "{text}");
        (out.settings.devices[0].appearance.clone(), out.warnings)
    }

    #[test]
    fn missing_file_gives_no_devices() {
        let out = load(&temp_dir("missing").join("settings.json"));
        assert_eq!(out.settings, Settings::default());
        assert!(out.warnings.is_empty());
        assert!(out.writable);
    }

    #[test]
    fn v2_round_trips_two_devices() {
        let path = temp_dir("roundtrip").join("nested/dir/settings.json");
        let settings = Settings {
            version: VERSION,
            devices: vec![
                device("defy", Appearance::default()),
                Device {
                    address: Some("127.0.0.1:7171".into()),
                    layout: Some("/k/mouse.json".into()),
                    ..device(
                        "mouse",
                        Appearance {
                            edge: Edge::BottomRight,
                            margin: 0,
                            opacity: 40,
                            scale: 250,
                            screen: Some("DP-1".to_owned()),
                        },
                    )
                },
            ],
        };
        save(&path, &settings).unwrap();
        let out = load(&path);
        assert_eq!(out.settings, settings);
        assert!(out.warnings.is_empty());

        let text = fs::read_to_string(&path).unwrap();
        assert!(text.ends_with("}\n"));
        assert!(text.contains("\"version\": 2"));
        assert!(text.contains("\"edge\": \"bottom-right\""));
        let leftovers: Vec<_> = fs::read_dir(path.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(leftovers, vec![OsString::from("settings.json")]);
    }

    #[test]
    fn v1_migrates_to_one_main_device() {
        let out = parse(r#"{"version":1,"edge":"top-left","opacity":40}"#);
        assert!(out.writable);
        let d = &out.settings.devices[0];
        assert_eq!((d.id.as_str(), d.name.as_str()), ("main", "Keyboard"));
        assert_eq!(d.appearance.edge, Edge::TopLeft);
        assert_eq!(d.appearance.opacity, 40);
        assert_eq!(d.config, None);
    }

    #[test]
    fn bad_field_falls_back_alone() {
        let (a, w) = v1(r#"{"version":1,"edge":"diagonal","opacity":40}"#);
        assert_eq!(a.edge, Edge::Bottom);
        assert_eq!(a.opacity, 40);
        assert!(matches!(
            w.as_slice(),
            [SettingsWarning::InvalidField { field, .. }] if field == "edge"
        ));
    }

    #[test]
    fn out_of_range_values_fall_back() {
        for text in [
            r#"{"opacity":5}"#,
            r#"{"opacity":500}"#,
            r#"{"opacity":"80"}"#,
        ] {
            let (a, w) = v1(text);
            assert_eq!(a.opacity, 85, "{text}");
            assert_eq!(w.len(), 1, "{text}");
        }
        assert_eq!(v1(r#"{"scale":49}"#).0.scale, 100);
        assert_eq!(v1(r#"{"margin":501}"#).0.margin, 16);
        assert_eq!(v1(r#"{"margin":-1}"#).0.margin, 16);
    }

    #[test]
    fn invalid_devices_are_skipped_and_fields_fall_back() {
        let out = parse(
            r#"{"version":2,"devices":[
                {"id":"defy","name":"Defy","opacity":5},
                "nonsense",
                {"name":"no id"},
                {"id":"Mouse Pad","edge":"bottom-right"}
            ]}"#,
        );
        let ids: Vec<_> = out.settings.devices.iter().map(|d| d.id.as_str()).collect();
        assert_eq!(ids, ["defy", "mouse-pad"]);
        assert_eq!(out.settings.devices[0].appearance.opacity, 85);
        assert_eq!(out.settings.devices[1].name, "mouse-pad");
        assert!(out.warnings.contains(&SettingsWarning::InvalidDevice {
            index: 1,
            reason: "not an object".into()
        }));
        assert!(out.warnings.iter().any(
            |w| matches!(w, SettingsWarning::InvalidField { field, .. } if field == "devices[1].opacity")
        ));
    }

    #[test]
    fn duplicate_ids_get_a_suffix() {
        let out = parse(r#"{"version":2,"devices":[{"id":"a"},{"id":"a"},{"id":"a"}]}"#);
        let ids: Vec<_> = out.settings.devices.iter().map(|d| d.id.as_str()).collect();
        assert_eq!(ids, ["a", "a-2", "a-3"]);
    }

    #[test]
    fn newer_version_is_not_writable() {
        let out = parse(r#"{"version":3,"devices":[]}"#);
        assert_eq!(out.settings, Settings::default());
        assert!(!out.writable);
        assert_eq!(
            out.warnings,
            vec![SettingsWarning::NewerVersion { found: 3 }]
        );
    }

    #[test]
    fn non_json_is_not_writable() {
        for text in ["not json", "[1, 2]"] {
            let out = parse(text);
            assert_eq!(out.settings, Settings::default());
            assert!(!out.writable);
            assert!(matches!(
                out.warnings.as_slice(),
                [SettingsWarning::NotJson { .. }]
            ));
        }
    }

    #[test]
    fn unknown_fields_are_ignored() {
        let (a, w) = v1(r#"{"version":1,"edge":"top","colour":"red"}"#);
        assert_eq!(a.edge, Edge::Top);
        assert!(w.is_empty());
    }

    #[test]
    fn screen_null_empty_and_named() {
        assert_eq!(v1(r#"{"screen":null}"#).0.screen, None);
        assert_eq!(v1(r#"{"screen":""}"#).0.screen, None);
        assert_eq!(
            v1(r#"{"screen":"HDMI-A-1"}"#).0.screen.as_deref(),
            Some("HDMI-A-1")
        );
        assert_eq!(v1(r#"{"screen":3}"#).1.len(), 1);
    }

    #[test]
    fn default_path_prefers_xdg_config_home() {
        assert_eq!(
            path_from(Some("/x".into()), Some("/home/u".into())),
            Some(PathBuf::from("/x/kanata-overlay/settings.json"))
        );
        assert_eq!(
            path_from(Some("".into()), Some("/home/u".into())),
            Some(PathBuf::from(
                "/home/u/.config/kanata-overlay/settings.json"
            ))
        );
        assert_eq!(path_from(None, None), None);
    }
}
