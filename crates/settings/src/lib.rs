//! Overlay settings: the devices the overlay shows, and where each board sits
//! and how it looks.
//!
//! - [`file`]: the versioned JSON settings file (lenient load, atomic save).
//! - [`devices`]: the device list, including the implicit single device.
//! - [`cli`]: command-line overrides for a single run.
//! - [`placement`]: where the overlay goes on a screen and how far it may scale.
//!
//! No Qt here: the app's `SettingsState` QObject is a thin wrapper around this crate.
//! Settings only ever hold placement and appearance; never key-press data.

use std::fmt;
use std::ops::RangeInclusive;
use std::path::PathBuf;
use std::str::FromStr;

use serde::Serialize;

mod cli;
mod devices;
mod file;
mod placement;

pub use cli::{CliError, Overrides};
pub use devices::{default_address, slug, typed_path};
pub use file::{LoadOutcome, SettingsWarning, default_path, load, save};
pub use placement::{Placement, place};

/// Current settings file version (2: a list of devices).
pub const VERSION: u32 = 2;
/// Margin from the screen edge, logical pixels.
pub const MARGIN_RANGE: RangeInclusive<u32> = 0..=500;
/// Opacity, percent.
pub const OPACITY_RANGE: RangeInclusive<u8> = 10..=100;
/// Scale relative to the default size, percent.
pub const SCALE_RANGE: RangeInclusive<u16> = 50..=300;

/// Where the overlay sits: centred along an edge, or in a corner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Edge {
    Top,
    #[default]
    Bottom,
    Left,
    Right,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// Where the overlay sits along one axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Start,
    Centre,
    End,
}

impl Edge {
    pub const ALL: [Edge; 8] = [
        Edge::Top,
        Edge::Bottom,
        Edge::Left,
        Edge::Right,
        Edge::TopLeft,
        Edge::TopRight,
        Edge::BottomLeft,
        Edge::BottomRight,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Edge::Top => "top",
            Edge::Bottom => "bottom",
            Edge::Left => "left",
            Edge::Right => "right",
            Edge::TopLeft => "top-left",
            Edge::TopRight => "top-right",
            Edge::BottomLeft => "bottom-left",
            Edge::BottomRight => "bottom-right",
        }
    }

    /// Horizontal alignment (`Start` = left).
    pub fn horizontal(self) -> Align {
        match self {
            Edge::Left | Edge::TopLeft | Edge::BottomLeft => Align::Start,
            Edge::Right | Edge::TopRight | Edge::BottomRight => Align::End,
            Edge::Top | Edge::Bottom => Align::Centre,
        }
    }

    /// Vertical alignment (`Start` = top).
    pub fn vertical(self) -> Align {
        match self {
            Edge::Top | Edge::TopLeft | Edge::TopRight => Align::Start,
            Edge::Bottom | Edge::BottomLeft | Edge::BottomRight => Align::End,
            Edge::Left | Edge::Right => Align::Centre,
        }
    }
}

impl fmt::Display for Edge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Edge {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Edge::ALL
            .into_iter()
            .find(|e| e.as_str() == s)
            .ok_or_else(|| {
                "expected top, bottom, left, right, top-left, top-right, bottom-left or bottom-right"
                    .to_owned()
            })
    }
}

/// Where one board sits and how it looks (per device).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Appearance {
    pub edge: Edge,
    pub margin: u32,
    pub opacity: u8,
    pub scale: u16,
    /// Qt screen name (e.g. `DP-1`); `None` means the primary screen.
    pub screen: Option<String>,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            edge: Edge::Bottom,
            margin: 16,
            opacity: 85,
            scale: 100,
            screen: None,
        }
    }
}

/// A device the overlay shows, with its own Kanata and board.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Device {
    /// Unique, `[a-z0-9-]{1,32}`.
    pub id: String,
    /// 1–40 characters, shown on the board and in the settings window.
    pub name: String,
    /// The Kanata `.kbd` config; without it there is no Kanata and no labels.
    pub config: Option<PathBuf>,
    /// KLE or own-format layout; without it the board is a grid of `defsrc`.
    pub layout: Option<PathBuf>,
    /// `host:port` of its Kanata; `None` → [`default_address`] by position.
    pub address: Option<String>,
    pub appearance: Appearance,
}

/// The persisted settings: the device list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    pub version: u32,
    pub devices: Vec<Device>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: VERSION,
            devices: Vec::new(),
        }
    }
}

/// One settings field, for warnings and for dropping a single override.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Edge,
    Margin,
    Opacity,
    Scale,
    Screen,
}

impl Field {
    pub fn name(self) -> &'static str {
        match self {
            Field::Edge => "edge",
            Field::Margin => "margin",
            Field::Opacity => "opacity",
            Field::Scale => "scale",
            Field::Screen => "screen",
        }
    }
}

/// A rectangle in logical pixels (a screen's geometry).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// A size in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Size {
    pub width: i32,
    pub height: i32,
}

/// Check that `value` lies in `range`, with a readable error.
fn in_range<T: PartialOrd + fmt::Display>(
    value: T,
    range: &RangeInclusive<T>,
) -> Result<T, String> {
    if range.contains(&value) {
        Ok(value)
    } else {
        Err(format!(
            "{value} is outside {}–{}",
            range.start(),
            range.end()
        ))
    }
}

/// An empty screen name means "primary".
fn screen_name(name: &str) -> Option<String> {
    (!name.is_empty() && name != "primary").then(|| name.to_owned())
}
