//! What the session can do for the overlay (research R4).
//!
//! On Wayland, the overlay needs `zwlr_layer_shell_v1`; Qt and LayerShellQt do
//! not say whether the compositor offers it, so we ask the registry ourselves on a
//! short-lived connection.

use wayland_client::protocol::wl_registry;
use wayland_client::{Connection, Dispatch, QueueHandle};

#[cxx_qt::bridge]
pub mod ffi {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;

        include!("kanata-overlay/cpp/qt_extras.h");
        fn kog_platform_name() -> QString;
        fn kog_set_desktop_file_name(name: &QString);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Platform {
    Wayland,
    X11,
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capabilities {
    pub platform: Platform,
    pub layer_shell: bool,
}

/// Detect the session from Qt's platform plugin name (`wayland`, `xcb`, …).
pub fn detect(qt_platform_name: &str) -> Capabilities {
    let platform = match qt_platform_name {
        "wayland" => Platform::Wayland,
        "xcb" => Platform::X11,
        other => Platform::Other(other.to_owned()),
    };
    let layer_shell = platform == Platform::Wayland
        && probe_layer_shell().unwrap_or_else(|e| {
            tracing::debug!("layer-shell probe failed: {e}");
            false
        });
    Capabilities {
        platform,
        layer_shell,
    }
}

/// The parts of the overlay this session cannot provide (FR-013).
pub fn missing_features(caps: &Capabilities) -> Vec<&'static str> {
    match (&caps.platform, caps.layer_shell) {
        (Platform::Wayland, true) => Vec::new(),
        (Platform::X11, _) => vec!["always-on-top is best-effort on X11"],
        _ => vec![
            "not always on top",
            "not on all desktops",
            "no edge placement",
        ],
    }
}

/// Qt's platform plugin name; valid once the QGuiApplication exists.
pub fn qt_platform_name() -> String {
    ffi::kog_platform_name().to_string()
}

pub fn set_desktop_file_name(name: &str) {
    ffi::kog_set_desktop_file_name(&ffi::QString::from(name));
}

struct Globals {
    layer_shell: bool,
}

impl Dispatch<wl_registry::WlRegistry, ()> for Globals {
    fn event(
        state: &mut Self,
        _: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global { interface, .. } = event
            && interface == "zwlr_layer_shell_v1"
        {
            state.layer_shell = true;
        }
    }
}

fn probe_layer_shell() -> Result<bool, Box<dyn std::error::Error>> {
    let conn = Connection::connect_to_env()?;
    let mut queue = conn.new_event_queue();
    let _registry = conn.display().get_registry(&queue.handle(), ());
    let mut globals = Globals { layer_shell: false };
    queue.roundtrip(&mut globals)?;
    Ok(globals.layer_shell)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_missing_features() {
        let caps = |platform, layer_shell| Capabilities {
            platform,
            layer_shell,
        };
        assert!(missing_features(&caps(Platform::Wayland, true)).is_empty());
        assert_eq!(missing_features(&caps(Platform::X11, false)).len(), 1);
        assert_eq!(missing_features(&caps(Platform::Wayland, false)).len(), 3);
    }
}
