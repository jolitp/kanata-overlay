//! What is global to the overlay: the platform, global notices (shortcuts,
//! settings file, missing overlay features) and showing or hiding all boards.
//!
//! Everything per device (its Kanata, connection, layer and labels) lives in
//! [`crate::device_state`].

use std::pin::Pin;

use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;

use crate::platform;

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
        /// Wayland with layer-shell: use the real overlay (feature 001, research R4).
        #[qproperty(bool, use_layer_shell)]
        /// Global notices, joined with " · "; shown on the first board.
        #[qproperty(QString, notices)]
        /// Toggled by the show/hide shortcut.
        #[qproperty(bool, overlay_shown)]
        /// `overlay_shown`, or the settings window is open (live preview).
        #[qproperty(bool, overlay_visible)]
        /// The settings window is open: every board shows, hidden ones too.
        #[qproperty(bool, in_preview)]
        /// Devices whose board is hidden on its own (its tray icon), as a JSON
        /// array of ids (feature 007).
        #[qproperty(QString, hidden_json)]
        type OverlayState = super::OverlayStateRust;

        /// Show or hide one device's board (its tray icon). `all_ids` is every
        /// device, as a JSON array: showing one while everything is hidden
        /// shows just that one.
        #[qinvokable]
        fn toggle_device(self: Pin<&mut OverlayState>, id: &QString, all_ids: &QString);

        /// Called once by QML before the devices start.
        #[qinvokable]
        fn start(self: Pin<&mut OverlayState>);

        /// Show or hide every board (the global shortcut).
        #[qinvokable]
        fn toggle_overlay(self: Pin<&mut OverlayState>);

        /// The settings window opened or closed: show the boards while it is open.
        #[qinvokable]
        fn set_previewing(self: Pin<&mut OverlayState>, on: bool);

        /// Add a global notice (duplicates are dropped).
        #[qinvokable]
        fn add_notice(self: Pin<&mut OverlayState>, text: &QString);
    }
}

pub struct OverlayStateRust {
    use_layer_shell: bool,
    notices: QString,
    overlay_shown: bool,
    overlay_visible: bool,
    in_preview: bool,
    hidden_json: QString,
    hidden: Vec<String>,
    previewing: bool,
    notice_list: Vec<String>,
    started: bool,
}

impl Default for OverlayStateRust {
    fn default() -> Self {
        Self {
            use_layer_shell: false,
            notices: QString::default(),
            // Always start shown (feature 001 assumption).
            overlay_shown: true,
            overlay_visible: true,
            in_preview: false,
            hidden_json: QString::from("[]"),
            hidden: Vec::new(),
            previewing: false,
            notice_list: Vec::new(),
            started: false,
        }
    }
}

impl qobject::OverlayState {
    fn start(mut self: Pin<&mut Self>) {
        if self.rust().started {
            return;
        }
        self.as_mut().rust_mut().started = true;
        let caps = platform::detect(&platform::qt_platform_name());
        tracing::info!("platform: {caps:?}");
        self.as_mut().set_use_layer_shell(caps.layer_shell);
        for missing in platform::missing_features(&caps) {
            self.as_mut().add_notice(&QString::from(missing));
        }
    }

    fn toggle_overlay(mut self: Pin<&mut Self>) {
        let shown = !*self.overlay_shown();
        self.as_mut().set_overlay_shown(shown);
        // Showing everything brings back boards hidden on their own too.
        if shown {
            self.as_mut().set_hidden(Vec::new());
        }
        self.update_visible();
    }

    fn toggle_device(mut self: Pin<&mut Self>, id: &QString, all_ids: &QString) {
        let id = id.to_string();
        let all: Vec<String> = serde_json::from_str(&all_ids.to_string()).unwrap_or_default();
        if !*self.overlay_shown() {
            // Everything hidden: show just this board.
            let others = all.into_iter().filter(|d| *d != id).collect();
            self.as_mut().set_hidden(others);
            self.as_mut().set_overlay_shown(true);
            self.update_visible();
            return;
        }
        let mut hidden = self.rust().hidden.clone();
        if let Some(i) = hidden.iter().position(|d| *d == id) {
            hidden.remove(i);
        } else {
            hidden.push(id);
        }
        self.set_hidden(hidden);
    }

    fn set_hidden(mut self: Pin<&mut Self>, hidden: Vec<String>) {
        let json = serde_json::to_string(&hidden).unwrap_or_default();
        self.as_mut().rust_mut().hidden = hidden;
        self.as_mut().set_hidden_json(QString::from(&json));
    }

    fn set_previewing(mut self: Pin<&mut Self>, on: bool) {
        self.as_mut().rust_mut().previewing = on;
        self.as_mut().set_in_preview(on);
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
}
