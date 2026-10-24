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
        type OverlayState = super::OverlayStateRust;

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
        self.update_visible();
    }

    fn set_previewing(mut self: Pin<&mut Self>, on: bool) {
        self.as_mut().rust_mut().previewing = on;
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
