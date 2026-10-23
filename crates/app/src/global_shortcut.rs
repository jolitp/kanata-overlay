//! A system-wide shortcut, registered with KDE's KGlobalAccel (research R5).
//!
//! The C++ shim owns a `QAction` parented to this object and calls our
//! `activate()` invokable when the shortcut fires. We never see other keys.

use std::pin::Pin;

use cxx_qt::casting::Upcast;

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;

        include!("kanata-overlay/cpp/global_shortcut.h");
        fn kog_register_global_shortcut(
            receiver: Pin<&mut QObject>,
            id: &QString,
            text: &QString,
            default_keys: &QString,
        ) -> bool;
    }

    #[auto_cxx_name]
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        /// Stable id, shown nowhere but used by KGlobalAccel to remember rebinding.
        #[qproperty(QString, shortcut_id)]
        /// Label in System Settings › Shortcuts.
        #[qproperty(QString, text)]
        /// Default combination in `QKeySequence` portable text, e.g. `Meta+Alt+K`.
        #[qproperty(QString, default_keys)]
        #[qproperty(bool, registered)]
        type GlobalShortcut = super::GlobalShortcutRust;

        /// Register with KGlobalAccel (once); sets `registered`.
        #[qinvokable]
        fn register_shortcut(self: Pin<&mut GlobalShortcut>);

        /// Called by the C++ shim when the shortcut fires.
        #[qinvokable]
        fn activate(self: Pin<&mut GlobalShortcut>);

        #[qsignal]
        fn activated(self: Pin<&mut GlobalShortcut>);
    }
}

#[derive(Default)]
pub struct GlobalShortcutRust {
    shortcut_id: cxx_qt_lib::QString,
    text: cxx_qt_lib::QString,
    default_keys: cxx_qt_lib::QString,
    registered: bool,
}

impl qobject::GlobalShortcut {
    fn register_shortcut(mut self: Pin<&mut Self>) {
        if *self.registered() {
            return;
        }
        let id = self.shortcut_id().clone();
        let text = self.text().clone();
        let keys = self.default_keys().clone();
        let ok =
            qobject::kog_register_global_shortcut(self.as_mut().upcast_pin(), &id, &text, &keys);
        if !ok {
            tracing::warn!("global shortcut {id} could not be registered");
        }
        self.set_registered(ok);
    }

    fn activate(self: Pin<&mut Self>) {
        self.activated();
    }
}
