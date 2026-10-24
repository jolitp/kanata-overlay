//! Reads the part of a Kanata `.kbd` config the overlay needs and turns each
//! key's action on a layer into a short label.
//!
//! - [`read`] / [`parse`]: `defsrc`, `deflayer`, `deflayermap`, `defalias`
//!   (research R3); anything else is ignored or reported as skipped.
//! - [`labels`]: what each physical key shows on a layer (contracts/labels.md).
//! - [`canonical`]: one name per key, whatever spelling the config uses.
//!
//! No Qt here. Config text is never put into errors or warnings (FR-013).

use std::collections::HashMap;
use std::fmt;
use std::path::Path;

mod action;
mod key_names;
mod label;
mod lexer;
mod reader;

pub use label::labels;
pub use lexer::SyntaxError;

/// A key name as written in the config, and the name Kanata means by it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyName {
    pub written: String,
    /// Kanata's OsCode name (e.g. `KEY_ENTER`), or `written` if unknown.
    pub canonical: String,
}

impl KeyName {
    pub fn new(written: &str) -> Self {
        Self {
            written: written.to_owned(),
            canonical: canonical(written),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layer {
    pub name: String,
    /// One action per `defsrc` key.
    pub actions: Vec<Action>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub source: Vec<KeyName>,
    /// In file order; the first is Kanata's default layer.
    pub layers: Vec<Layer>,
    pub aliases: HashMap<String, Action>,
    pub delegate_to_first_layer: bool,
    /// Which input devices this config's Kanata grabs (`defcfg`).
    pub devices: DeviceSelection,
    pub warnings: Vec<ConfigWarning>,
}

/// The devices a config limits Kanata to (Kanata 1.12 `defcfg`, Linux).
/// All empty means Kanata auto-detects (every keyboard, and mice too if
/// `mouse_buttons`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeviceSelection {
    /// `linux-dev`: device paths.
    pub paths: Vec<String>,
    /// `linux-dev-names-include`: exact device names.
    pub include: Vec<String>,
    /// `linux-dev-names-exclude`: exact device names.
    pub exclude: Vec<String>,
    /// `defsrc` has mouse buttons, so auto-detection also grabs mice.
    pub mouse_buttons: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Modifier {
    Ctrl,
    Alt,
    Shift,
    Super,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Side {
    Left,
    Right,
}

/// What a key does (research R4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Key(KeyName),
    Combo {
        mods: Vec<(Modifier, Side)>,
        key: KeyName,
    },
    Transparent,
    NoOp,
    UseDefsrc,
    Alias(String),
    LayerSwitch(String),
    LayerHold(String),
    TapHold {
        tap: Box<Action>,
        hold: Box<Action>,
    },
    OneShot(Box<Action>),
    Unicode(String),
    CapsWord,
    /// Anything else, as config text (macros, tap-dance, mouse…).
    Other(String),
}

/// The config can't be used at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    Io(String),
    Syntax(SyntaxError),
    NoDefsrc,
    NoLayers,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::Io(e) => f.write_str(e),
            ConfigError::Syntax(at) => write!(
                f,
                "unbalanced parentheses or unclosed string/comment at line {}, column {}",
                at.line, at.column
            ),
            ConfigError::NoDefsrc => f.write_str("no defsrc"),
            ConfigError::NoLayers => f.write_str("no deflayer or deflayermap"),
        }
    }
}

impl std::error::Error for ConfigError {}

/// Something the overlay could not use, shown once as a notice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigWarning {
    Skipped {
        forms: Vec<String>,
    },
    LayerLength {
        layer: String,
        expected: usize,
        found: usize,
    },
    DuplicateAlias {
        name: String,
    },
    UnknownAlias {
        name: String,
    },
    AliasLoop {
        name: String,
    },
    UnknownLayer {
        name: String,
    },
}

impl fmt::Display for ConfigWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigWarning::Skipped { forms } => {
                write!(f, "config: skipped {}", forms.join(", "))
            }
            ConfigWarning::LayerLength {
                layer,
                expected,
                found,
            } => write!(
                f,
                "config: layer {layer} has {found} keys, defsrc has {expected}"
            ),
            ConfigWarning::DuplicateAlias { name } => {
                write!(f, "config: alias @{name} is defined twice, using the last")
            }
            ConfigWarning::UnknownAlias { name } => write!(f, "config: unknown alias @{name}"),
            ConfigWarning::AliasLoop { name } => write!(f, "config: alias @{name} loops"),
            ConfigWarning::UnknownLayer { name } => {
                write!(f, "config: no layer {name}, showing the first layer")
            }
        }
    }
}

/// How a label is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelStyle {
    Normal,
    /// Comes from a lower layer or `defsrc` (transparent key).
    Inherited,
    /// The key does nothing on this layer.
    Disabled,
}

impl LabelStyle {
    pub fn as_str(self) -> &'static str {
        match self {
            LabelStyle::Normal => "normal",
            LabelStyle::Inherited => "inherited",
            LabelStyle::Disabled => "disabled",
        }
    }
}

/// What one physical key shows on the active layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyLabel {
    pub text: String,
    pub style: LabelStyle,
    /// Tap-hold key: draw the hold marker.
    pub hold: bool,
    /// The alias the action came from, if any.
    pub alias: Option<String>,
}

/// Parse config text.
pub fn parse(text: &str) -> Result<Config, ConfigError> {
    let forms = lexer::parse(text).map_err(|(_, at)| ConfigError::Syntax(at))?;
    reader::read(&forms)
}

/// Read and parse a config file. Only reads; never writes (FR-004).
pub fn read(path: &Path) -> Result<Config, ConfigError> {
    let text = std::fs::read_to_string(path).map_err(|e| ConfigError::Io(e.to_string()))?;
    parse(&text)
}

/// Kanata's name for a key (`ret`, `enter` and `⏎` are all `KEY_ENTER`);
/// unknown names come back unchanged.
pub fn canonical(name: &str) -> String {
    key_names::KEY_ALIASES
        .binary_search_by(|(alias, _)| alias.as_bytes().cmp(name.as_bytes()))
        .map(|i| key_names::KEY_ALIASES[i].1.to_owned())
        .unwrap_or_else(|_| name.to_owned())
}

/// Every key name Kanata accepts, with its canonical key (`("ret", "KEY_ENTER")`).
pub fn key_names() -> impl Iterator<Item = (&'static str, &'static str)> {
    key_names::KEY_ALIASES.iter().copied()
}

/// Whether Kanata accepts `name` as a key name.
pub fn is_key_name(name: &str) -> bool {
    key_names::KEY_ALIASES
        .binary_search_by(|(alias, _)| alias.as_bytes().cmp(name.as_bytes()))
        .is_ok()
}

/// Keys a password is made of (feature 006, research R8): hidden under the
/// "no typing keys" highlight setting.
pub fn is_typing_key(canonical: &str) -> bool {
    const OTHERS: &[&str] = &[
        "KEY_MINUS",
        "KEY_EQUAL",
        "KEY_LEFTBRACE",
        "KEY_RIGHTBRACE",
        "KEY_BACKSLASH",
        "KEY_SEMICOLON",
        "KEY_APOSTROPHE",
        "KEY_GRAVE",
        "KEY_COMMA",
        "KEY_DOT",
        "KEY_SLASH",
        "KEY_SPACE",
        "KEY_102ND",
        "KEY_KPDOT",
    ];
    let single = |prefix: &str, ok: fn(char) -> bool| {
        canonical
            .strip_prefix(prefix)
            .is_some_and(|rest| rest.chars().count() == 1 && rest.chars().all(ok))
    };
    single("KEY_", |c| c.is_ascii_uppercase() || c.is_ascii_digit())
        || single("KEY_KP", |c| c.is_ascii_digit())
        || OTHERS.contains(&canonical)
}

/// The spelling to show or write for a canonical key: plain text first, then
/// the shortest (`KEY_ENTER` → `ent`).
pub fn preferred_name(canonical: &str) -> Option<&'static str> {
    key_names()
        .filter(|(_, c)| *c == canonical)
        .map(|(n, _)| n)
        .min_by_key(|n| (!n.is_ascii(), n.chars().count(), *n))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typing_keys_and_preferred_names() {
        for k in [
            "KEY_A",
            "KEY_Z",
            "KEY_0",
            "KEY_SPACE",
            "KEY_KP5",
            "KEY_SLASH",
        ] {
            assert!(is_typing_key(k), "{k}");
        }
        for k in [
            "KEY_LEFTSHIFT",
            "KEY_ESC",
            "KEY_UP",
            "KEY_F1",
            "BTN_SIDE",
            "KEY_KPENTER",
            "KEY_F10",
        ] {
            assert!(!is_typing_key(k), "{k}");
        }
        assert_eq!(preferred_name("KEY_ENTER"), Some("ent"));
        assert_eq!(preferred_name("KEY_A"), Some("a"));
        assert_eq!(preferred_name("NOPE"), None);
    }

    #[test]
    fn key_name_table() {
        for name in ["caps", "ret", "f13", "mbck", "lsft", ";"] {
            assert!(is_key_name(name), "{name}");
        }
        for name in ["nonsense", "", "Caps "] {
            assert!(!is_key_name(name), "{name}");
        }
        assert_eq!(key_names().count(), key_names::KEY_ALIASES.len());
        assert!(key_names().any(|(n, c)| n == "ret" && c == "KEY_ENTER"));
    }

    #[test]
    fn canonical_names() {
        assert_eq!(canonical("ret"), "KEY_ENTER");
        assert_eq!(canonical("enter"), "KEY_ENTER");
        assert_eq!(canonical("⏎"), "KEY_ENTER");
        assert_eq!(canonical("lsft"), canonical("lshift"));
        assert_eq!(canonical(";"), "KEY_SEMICOLON");
        assert_eq!(canonical("\\"), "KEY_BACKSLASH");
        assert_eq!(canonical("right"), canonical("rght"));
        assert_eq!(canonical("nonsense"), "nonsense");
    }

    #[test]
    fn table_is_sorted_for_binary_search() {
        let t = key_names::KEY_ALIASES;
        assert!(t.windows(2).all(|w| w[0].0.as_bytes() < w[1].0.as_bytes()));
    }

    #[test]
    fn errors_never_contain_config_text() {
        let err = parse("(defsrc a b\n(deflayer secret-text").unwrap_err();
        assert!(!err.to_string().contains("secret"));
    }
}
