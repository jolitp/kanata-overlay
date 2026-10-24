//! The device list: which devices the overlay shows, including the implicit
//! single device of today's `--config`/`--layout` usage (data-model › Settings).

use std::path::{Path, PathBuf};

use crate::{Appearance, Device, Overrides, Settings};

/// Device *i* talks to its Kanata here unless it sets `address` (research R4).
pub fn default_address(index: usize) -> String {
    format!("127.0.0.1:{}", 7070 + index)
}

/// A config or layout path as typed or pasted in the settings window: trimmed,
/// `None` if empty, and a leading `~/` expanded against `home`.
pub fn typed_path(text: &str, home: Option<&Path>) -> Option<PathBuf> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    match (text.strip_prefix("~/"), home) {
        (Some(rest), Some(home)) => Some(home.join(rest)),
        _ if text == "~" => home.map(Path::to_path_buf),
        _ => Some(PathBuf::from(text)),
    }
}

/// An id from a name: `[a-z0-9-]{1,32}` ("Mouse keypad" → `mouse-keypad`).
pub fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.trim().to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    out.truncate(32);
    out.trim_end_matches('-').to_owned()
}

impl Settings {
    /// The devices to show this run.
    ///
    /// - No devices saved: one implicit `main` device from the command line
    ///   (with or without a config), with the CLI appearance overrides.
    /// - One device: the CLI overrides apply to it, and a missing config or
    ///   layout comes from the CLI (a migrated version 1 file).
    /// - Several devices: the CLI appearance overrides are ignored.
    pub fn effective_devices(
        &self,
        cli: &Overrides,
        cli_config: Option<&Path>,
        cli_layout: Option<&Path>,
    ) -> Vec<Device> {
        match self.devices.as_slice() {
            [] => vec![Device {
                id: "main".to_owned(),
                name: "Keyboard".to_owned(),
                config: cli_config.map(Path::to_path_buf),
                layout: cli_layout.map(Path::to_path_buf),
                address: None,
                appearance: cli.effective(&Appearance::default()),
            }],
            [only] => {
                let mut d = only.clone();
                d.appearance = cli.effective(&only.appearance);
                if d.config.is_none() {
                    d.config = cli_config.map(Path::to_path_buf);
                }
                if d.layout.is_none() {
                    d.layout = cli_layout.map(Path::to_path_buf);
                }
                vec![d]
            }
            many => many.to_vec(),
        }
    }

    /// Add a device named `name`; returns its new, unique id.
    pub fn add_device(&mut self, name: &str) -> String {
        let base = Some(slug(name))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "device".to_owned());
        let mut id = base.clone();
        let mut n = 2;
        while self.devices.iter().any(|d| d.id == id) {
            id = format!("{base}-{n}");
            n += 1;
        }
        self.devices.push(Device {
            id: id.clone(),
            name: name.trim().chars().take(40).collect(),
            config: None,
            layout: None,
            address: None,
            appearance: Appearance::default(),
        });
        id
    }

    pub fn remove_device(&mut self, id: &str) {
        self.devices.retain(|d| d.id != id);
    }

    pub fn device_mut(&mut self, id: &str) -> Option<&mut Device> {
        self.devices.iter_mut().find(|d| d.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Edge;
    use std::path::PathBuf;

    fn cli(args: &[&str]) -> Overrides {
        Overrides::parse(args.iter().map(|s| s.to_string())).unwrap()
    }

    #[test]
    fn no_devices_gives_an_implicit_main_from_the_cli() {
        let s = Settings::default();
        let d = s.effective_devices(
            &cli(&["--edge", "top"]),
            Some(Path::new("/k/main.kbd")),
            Some(Path::new("/k/board.json")),
        );
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].id, "main");
        assert_eq!(d[0].config.as_deref(), Some(Path::new("/k/main.kbd")));
        assert_eq!(d[0].layout.as_deref(), Some(Path::new("/k/board.json")));
        assert_eq!(d[0].appearance.edge, Edge::Top);

        let bare = s.effective_devices(&cli(&[]), None, None);
        assert_eq!(bare[0].config, None);
    }

    #[test]
    fn a_single_device_takes_cli_overrides_and_missing_paths() {
        let mut s = Settings::default();
        let id = s.add_device("Keyboard");
        s.device_mut(&id).unwrap().layout = Some(PathBuf::from("/saved.json"));
        let d = s.effective_devices(
            &cli(&["--opacity", "50"]),
            Some(Path::new("/cli.kbd")),
            Some(Path::new("/cli.json")),
        );
        assert_eq!(d[0].appearance.opacity, 50);
        assert_eq!(d[0].config.as_deref(), Some(Path::new("/cli.kbd")));
        assert_eq!(d[0].layout.as_deref(), Some(Path::new("/saved.json")));
    }

    #[test]
    fn several_devices_ignore_cli_appearance() {
        let mut s = Settings::default();
        s.add_device("Defy");
        s.add_device("Mouse keypad");
        let d = s.effective_devices(&cli(&["--edge", "top"]), Some(Path::new("/x.kbd")), None);
        assert_eq!(d.len(), 2);
        assert!(
            d.iter()
                .all(|d| d.appearance.edge == Edge::Bottom && d.config.is_none())
        );
    }

    #[test]
    fn ids_and_addresses() {
        assert_eq!(slug("Mouse keypad"), "mouse-keypad");
        assert_eq!(slug("  Défy!! 2 "), "d-fy-2");
        assert_eq!(slug(&"x".repeat(40)).len(), 32);
        let mut s = Settings::default();
        assert_eq!(s.add_device("Mouse keypad"), "mouse-keypad");
        assert_eq!(s.add_device("Mouse keypad"), "mouse-keypad-2");
        assert_eq!(s.add_device("!!!"), "device");
        s.remove_device("mouse-keypad");
        assert_eq!(s.devices.len(), 2);
        assert_eq!(default_address(0), "127.0.0.1:7070");
        assert_eq!(default_address(1), "127.0.0.1:7071");
    }

    #[test]
    fn typed_paths() {
        let home = Path::new("/home/me");
        assert_eq!(typed_path("  ", Some(home)), None);
        assert_eq!(
            typed_path(" ~/.config/kanata/defy.kbd\n", Some(home)),
            Some(PathBuf::from("/home/me/.config/kanata/defy.kbd"))
        );
        assert_eq!(
            typed_path("/etc/x.kbd", Some(home)),
            Some(PathBuf::from("/etc/x.kbd"))
        );
        assert_eq!(typed_path("~/x.kbd", None), Some(PathBuf::from("~/x.kbd")));
        assert_eq!(
            typed_path("~user/x.kbd", Some(home)),
            Some(PathBuf::from("~user/x.kbd"))
        );
    }
}
