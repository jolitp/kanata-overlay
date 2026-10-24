//! Reading key presses from a managed Kanata's debug output (feature 006).
//!
//! Kanata's TCP protocol reports no key presses, and while Kanata runs it holds
//! the device for itself. Started with `--debug`, it logs every input event as
//! `process recv ev KeyEvent { code: KEY_A (30), value: Press }` (Kanata 1.12,
//! research R1). That output is not a stable interface (constitution II), so it
//! is read only here, behind [`OutputTap`], and a format change only disables
//! highlighting.
//!
//! [`DebugKeyTap`] swallows *every* debug and trace line, key event or not, so
//! nothing Kanata logs at those levels reaches the supervisor's log, the log
//! window or `tracing` (constitution VI).

use std::fmt;

/// Offered every line a managed Kanata prints (ANSI codes stripped) before it
/// is logged.
pub trait OutputTap: Send + Sync + fmt::Debug {
    /// `true`: the line was consumed and must not be logged.
    fn line(&self, line: &str) -> bool;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyAction {
    Press,
    Release,
    Repeat,
}

/// One input event. `key` is Kanata's `OsCode` name, which is already the
/// canonical key name (`KEY_CAPSLOCK`, `BTN_SIDE`).
#[derive(Clone, PartialEq, Eq)]
pub struct KeyEvent {
    pub key: String,
    pub action: KeyAction,
}

// Never print key names by accident (constitution VI).
impl fmt::Debug for KeyEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "KeyEvent({:?})", self.action)
    }
}

const MARKER: &str = "process recv ev KeyEvent { code: ";

/// Parse Kanata's debug line for an input event; `None` for anything else.
pub fn parse_key_event(line: &str) -> Option<KeyEvent> {
    let rest = &line[line.find(MARKER)? + MARKER.len()..];
    let (key, rest) = rest.split_once(' ')?;
    let value = rest.split_once("value: ")?.1;
    let value: String = value
        .chars()
        .take_while(|c| c.is_ascii_alphabetic())
        .collect();
    let action = match value.as_str() {
        "Press" => KeyAction::Press,
        "Release" => KeyAction::Release,
        "Repeat" => KeyAction::Repeat,
        _ => return None,
    };
    let valid = !key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    valid.then(|| KeyEvent {
        key: key.to_owned(),
        action,
    })
}

/// Consumes debug and trace lines; passes key events to `sink`.
pub struct DebugKeyTap {
    sink: Box<dyn Fn(KeyEvent) + Send + Sync>,
}

impl DebugKeyTap {
    pub fn new(sink: impl Fn(KeyEvent) + Send + Sync + 'static) -> Self {
        Self {
            sink: Box::new(sink),
        }
    }
}

impl fmt::Debug for DebugKeyTap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DebugKeyTap")
    }
}

impl OutputTap for DebugKeyTap {
    fn line(&self, line: &str) -> bool {
        if !(line.contains("[DEBUG]") || line.contains("[TRACE]")) {
            return false;
        }
        if let Some(event) = parse_key_event(line) {
            (self.sink)(event);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn parses_kanata_lines() {
        let e = parse_key_event(
            "08:00:03.7792 [DEBUG] process recv ev KeyEvent { code: KEY_A (30), value: Press }",
        )
        .unwrap();
        assert_eq!((e.key.as_str(), e.action), ("KEY_A", KeyAction::Press));
        let e = parse_key_event(
            "[DEBUG] process recv ev KeyEvent { code: BTN_SIDE (275), value: Release }",
        )
        .unwrap();
        assert_eq!((e.key.as_str(), e.action), ("BTN_SIDE", KeyAction::Release));
        let e =
            parse_key_event("process recv ev KeyEvent { code: KEY_CAPSLOCK (58), value: Repeat }")
                .unwrap();
        assert_eq!(e.action, KeyAction::Repeat);
        assert_eq!(parse_key_event("[DEBUG] layer stack: base"), None);
        assert_eq!(
            parse_key_event("process recv ev KeyEvent { code: KEY_A (30), value: Tap }"),
            None
        );
        assert_eq!(parse_key_event("process recv ev KeyEvent { code: "), None);
        assert!(
            !format!("{e:?}").contains("KEY_"),
            "Debug never shows the key"
        );
    }

    #[test]
    fn tap_swallows_every_debug_line() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let tap = DebugKeyTap::new({
            let seen = seen.clone();
            move |e: KeyEvent| seen.lock().unwrap().push(e.key)
        });
        assert!(tap.line("1 [DEBUG] process recv ev KeyEvent { code: KEY_B (48), value: Press }"));
        assert!(tap.line("2 [DEBUG] HoldActivated: key=caps coord=(0, 1)"));
        assert!(tap.line("3 [TRACE] anything"));
        assert!(tap.line("4 [DEBUG] process recv ev KeyEvent { code: ??? }"));
        assert!(!tap.line("5 [INFO] kanata v1.12.0 starting"));
        assert!(!tap.line("6 [ERROR] something"));
        assert_eq!(*seen.lock().unwrap(), ["KEY_B"]);
    }
}
