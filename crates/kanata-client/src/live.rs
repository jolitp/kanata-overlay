//! What happens to a key event (feature 006, data-model › Filter): captured
//! for press-to-bind, shown on the board, or dropped right away. Runs on the
//! output reader thread, before anything reaches the UI (research R3).

use crate::output_tap::{KeyAction, KeyEvent};

/// A device's highlight setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    All,
    NoTyping,
    Off,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    /// Press to bind took this key (any key, whatever the mode).
    Capture(String),
    Show(KeyEvent),
    Drop,
}

/// The filter, first matching rule wins.
pub fn route(event: KeyEvent, armed: bool, watching: bool, mode: Mode) -> Route {
    if armed && event.action == KeyAction::Press {
        return Route::Capture(event.key);
    }
    if event.action == KeyAction::Repeat
        || !watching
        || mode == Mode::Off
        || (mode == Mode::NoTyping && kbd::is_typing_key(&event.key))
    {
        return Route::Drop;
    }
    Route::Show(event)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(key: &str, action: KeyAction) -> KeyEvent {
        KeyEvent {
            key: key.into(),
            action,
        }
    }

    #[test]
    fn every_rule() {
        use KeyAction::*;
        let a = || ev("KEY_A", Press);
        let shift = || ev("KEY_LEFTSHIFT", Press);
        assert_eq!(
            route(a(), true, false, Mode::NoTyping),
            Route::Capture("KEY_A".into())
        );
        assert_eq!(
            route(ev("KEY_A", Release), true, true, Mode::All),
            Route::Show(ev("KEY_A", Release))
        );
        assert_eq!(
            route(ev("KEY_A", Repeat), false, true, Mode::All),
            Route::Drop
        );
        assert_eq!(route(a(), false, false, Mode::All), Route::Drop);
        assert_eq!(route(a(), false, true, Mode::Off), Route::Drop);
        assert_eq!(route(a(), false, true, Mode::NoTyping), Route::Drop);
        assert_eq!(
            route(shift(), false, true, Mode::NoTyping),
            Route::Show(shift())
        );
        assert_eq!(route(a(), false, true, Mode::All), Route::Show(a()));
    }
}
