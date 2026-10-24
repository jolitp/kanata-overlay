//! Physical keyboard layouts.
//!
//! Kanata's config knows *which* keys exist (`defsrc`) but not *where* they are.
//! A [`Layout`] supplies the geometry: each [`Key`] has a position, size and
//! rotation in keyboard units (1u = one standard key), plus the Kanata key name
//! that ties it to `defsrc`:
//!
//! ```text
//! physical key ──kanata name──▶ index in defsrc ──▶ action in the active deflayer
//! ```
//!
//! Layouts can be imported from Keyboard Layout Editor (KLE) JSON and are saved
//! in the app's own JSON format ([`Layout::to_json`]).

pub mod edit;

use serde::{Deserialize, Serialize};

/// Current version of the on-disk layout format.
pub const FORMAT_VERSION: u32 = 1;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid KLE JSON: {0}")]
    Kle(#[source] serde_json::Error),
    #[error("invalid layout file: {0}")]
    Format(#[source] serde_json::Error),
    #[error("unsupported layout format version {0} (this build reads up to {FORMAT_VERSION})")]
    Version(u32),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// A physical keyboard: a named set of keys.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layout {
    pub version: u32,
    pub name: String,
    pub keys: Vec<Key>,
    /// A reference picture shown under the keys in the layout editor only
    /// (e.g. a screenshot of the vendor's configurator); boards ignore it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<Background>,
}

/// The layout editor's reference image, placed in key units.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Background {
    /// Absolute path of the image file.
    pub path: String,
    /// Top-left corner, in units.
    pub x: f64,
    pub y: f64,
    /// Width in units; the height follows the image's aspect ratio.
    pub width: f64,
    /// 0.05–1.
    pub opacity: f64,
}

/// One physical key. Units are keyboard units (1u ≈ 19.05 mm).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Key {
    /// Top-left corner, before rotation.
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    /// Clockwise rotation in degrees around (`rx`, `ry`).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub rotation: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub rx: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub ry: f64,
    /// Kanata key name (`a`, `caps`, `lsft`, …). `None` until the user binds it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kanata: Option<String>,
    /// Legend printed on the cap in the source layout, kept to help binding.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub legend: String,
}

fn is_zero(v: &f64) -> bool {
    *v == 0.0
}

impl Layout {
    /// A plain grid of 1u keys, `columns` per row, for a device without a
    /// layout file: every key is bound to its Kanata name (feature 004, R7).
    pub fn grid(name: &str, names: &[String], columns: usize) -> Self {
        let columns = columns.max(1);
        let keys = names
            .iter()
            .enumerate()
            .map(|(i, n)| Key {
                x: (i % columns) as f64,
                y: (i / columns) as f64,
                width: 1.0,
                height: 1.0,
                rotation: 0.0,
                rx: 0.0,
                ry: 0.0,
                kanata: Some(n.clone()),
                legend: n.clone(),
            })
            .collect();
        Self {
            version: FORMAT_VERSION,
            name: name.to_owned(),
            keys,
            background: None,
        }
    }

    /// Import a layout exported from keyboard-layout-editor.com
    /// ("Download JSON", or the raw data tab wrapped in `[...]`).
    ///
    /// Keys whose legend clearly names a key get a guessed Kanata binding;
    /// everything else is left unbound for the user to assign in the editor.
    pub fn from_kle_json(json: &str) -> Result<Self> {
        let kb: kle_serial::Keyboard = serde_json::from_str(json).map_err(Error::Kle)?;
        let keys = kb
            .keys
            .into_iter()
            .filter(|k| !k.decal && !k.ghosted)
            .map(|k| {
                let legend = k
                    .legends
                    .iter()
                    .flatten()
                    .map(|l| l.text.trim())
                    .find(|t| !t.is_empty())
                    .unwrap_or_default()
                    .to_owned();
                Key {
                    // KLE quirk: stepped/L-shaped keys can extend left/up of (x, y).
                    x: k.x.min(k.x + k.x2),
                    y: k.y.min(k.y + k.y2),
                    width: k.width,
                    height: k.height,
                    rotation: k.rotation,
                    rx: k.rx,
                    ry: k.ry,
                    kanata: guess_kanata_name(&legend).map(str::to_owned),
                    legend,
                }
            })
            .collect();
        Ok(Self {
            version: FORMAT_VERSION,
            name: kb.metadata.name,
            keys,
            background: None,
        })
    }

    /// Load a layout saved by this app.
    pub fn from_json(json: &str) -> Result<Self> {
        let layout: Self = serde_json::from_str(json).map_err(Error::Format)?;
        if layout.version > FORMAT_VERSION {
            return Err(Error::Version(layout.version));
        }
        Ok(layout)
    }

    /// Serialize in the app's own format.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("layout always serializes")
    }

    /// Bounding box `(width, height)` in keyboard units, ignoring rotation.
    pub fn size(&self) -> (f64, f64) {
        self.keys.iter().fold((0.0_f64, 0.0_f64), |(w, h), k| {
            (w.max(k.x + k.width), h.max(k.y + k.height))
        })
    }

    /// Keys not yet bound to a Kanata key name.
    pub fn unbound(&self) -> impl Iterator<Item = &Key> {
        self.keys.iter().filter(|k| k.kanata.is_none())
    }
}

/// Best-effort mapping from a printed legend to a Kanata key name.
///
/// Only unambiguous legends are mapped: a bare "Shift" could be either side, so
/// it stays unbound and the user picks.
pub fn guess_kanata_name(legend: &str) -> Option<&'static str> {
    #[rustfmt::skip]
    const SINGLE: &[(&str, &str)] = &[
        ("a", "a"), ("b", "b"), ("c", "c"), ("d", "d"), ("e", "e"), ("f", "f"),
        ("g", "g"), ("h", "h"), ("i", "i"), ("j", "j"), ("k", "k"), ("l", "l"),
        ("m", "m"), ("n", "n"), ("o", "o"), ("p", "p"), ("q", "q"), ("r", "r"),
        ("s", "s"), ("t", "t"), ("u", "u"), ("v", "v"), ("w", "w"), ("x", "x"),
        ("y", "y"), ("z", "z"),
        ("1", "1"), ("2", "2"), ("3", "3"), ("4", "4"), ("5", "5"),
        ("6", "6"), ("7", "7"), ("8", "8"), ("9", "9"), ("0", "0"),
        ("`", "grv"), ("-", "-"), ("=", "="), ("[", "["), ("]", "]"),
        ("\\", "\\"), (";", ";"), ("'", "'"), (",", ","), (".", "."), ("/", "/"),
    ];
    #[rustfmt::skip]
    const WORDS: &[(&str, &str)] = &[
        ("esc", "esc"), ("escape", "esc"), ("tab", "tab"),
        ("caps", "caps"), ("caps lock", "caps"), ("capslock", "caps"),
        ("enter", "ret"), ("return", "ret"), ("backspace", "bspc"),
        ("space", "spc"), ("del", "del"), ("delete", "del"),
        ("ins", "ins"), ("insert", "ins"), ("home", "home"), ("end", "end"),
        ("pgup", "pgup"), ("page up", "pgup"), ("pgdn", "pgdn"), ("page down", "pgdn"),
        ("f1", "f1"), ("f2", "f2"), ("f3", "f3"), ("f4", "f4"), ("f5", "f5"),
        ("f6", "f6"), ("f7", "f7"), ("f8", "f8"), ("f9", "f9"), ("f10", "f10"),
        ("f11", "f11"), ("f12", "f12"),
        ("←", "left"), ("→", "rght"), ("↑", "up"), ("↓", "down"),
        ("left", "left"), ("right", "rght"), ("up", "up"), ("down", "down"),
    ];

    let l = legend.trim().to_lowercase();
    if l.is_empty() {
        return None;
    }
    SINGLE
        .iter()
        .chain(WORDS)
        .find(|(from, _)| *from == l)
        .map(|(_, to)| *to)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_of_names() {
        let names: Vec<String> = (0..13).map(|i| format!("k{i}")).collect();
        let g = Layout::grid("mouse", &names, 12);
        assert_eq!(g.keys.len(), 13);
        assert_eq!((g.keys[12].x, g.keys[12].y), (0.0, 1.0));
        assert!(
            g.keys
                .iter()
                .zip(&names)
                .all(|(k, n)| k.kanata.as_ref() == Some(n))
        );
        assert!(g.unbound().next().is_none());
    }

    // Two rows, including a 1.5u Tab and a rotated thumb key like you would find
    // on a split/ergonomic board.
    const KLE: &str = r#"[
        {"name": "test board"},
        ["Esc", "1", "2"],
        [{"w": 1.5}, "Tab", "Q"],
        [{"r": 15, "rx": 4, "ry": 3}, "Shift"]
    ]"#;

    #[test]
    fn imports_kle() {
        let layout = Layout::from_kle_json(KLE).unwrap();
        assert_eq!(layout.name, "test board");
        assert_eq!(layout.keys.len(), 6);

        let tab = &layout.keys[3];
        assert_eq!((tab.x, tab.y, tab.width), (0.0, 1.0, 1.5));
        assert_eq!(tab.kanata.as_deref(), Some("tab"));

        let q = &layout.keys[4];
        assert_eq!(q.x, 1.5);
        assert_eq!(q.kanata.as_deref(), Some("q"));

        let thumb = &layout.keys[5];
        assert_eq!((thumb.rotation, thumb.rx, thumb.ry), (15.0, 4.0, 3.0));
        assert_eq!(thumb.kanata, None, "bare Shift is ambiguous");
        assert_eq!(layout.unbound().count(), 1);
    }

    #[test]
    fn round_trips_own_format() {
        let layout = Layout::from_kle_json(KLE).unwrap();
        let back = Layout::from_json(&layout.to_json()).unwrap();
        assert_eq!(layout, back);
    }

    #[test]
    fn rejects_future_versions() {
        let json = r#"{"version": 99, "name": "x", "keys": []}"#;
        assert!(matches!(Layout::from_json(json), Err(Error::Version(99))));
    }

    #[test]
    fn guesses_names() {
        assert_eq!(guess_kanata_name("Caps Lock"), Some("caps"));
        assert_eq!(guess_kanata_name(" A "), Some("a"));
        assert_eq!(guess_kanata_name("Shift"), None);
        assert_eq!(guess_kanata_name(""), None);
    }
}
