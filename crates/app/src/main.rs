//! kanata-overlay: shows the active Kanata layer on a picture of your keyboard.
//!
//!     kanata-overlay [--connect 127.0.0.1:7070] [--layout my-board.json]
//!                    [--settings FILE] [--edge top|bottom|left|right] [--margin PX]
//!                    [--opacity PERCENT] [--scale PERCENT] [--screen NAME]
//!                    [--config my-config.kbd] [--kanata PROGRAM]
//!
//! `--layout` accepts KLE JSON (keyboard-layout-editor.com) or the app's own
//! layout format. Without it, a small sample board is shown. The placement
//! options override the settings file for this run only. `--config` names the
//! Kanata config whose actions label the keys (read only, not saved); with it,
//! the overlay starts and supervises Kanata itself when none is running
//! (`--kanata` names the program, default `kanata` on PATH).

mod desktop_entry;
mod desktop_state;
mod device_state;
mod global_shortcut;
mod layer_color;
mod layout_editor;
mod overlay_state;
mod platform;
mod settings_state;
mod single_instance;
mod tray_state;

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QString, QUrl};

/// Command-line options (hand-parsed for now; swap for `clap` when they grow).
#[derive(Debug, Clone)]
pub struct Args {
    pub address: String,
    pub layout: Option<std::path::PathBuf>,
    pub config: Option<std::path::PathBuf>,
    pub kanata: Option<std::path::PathBuf>,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            address: "127.0.0.1:7070".to_owned(),
            layout: None,
            config: None,
            kanata: None,
        }
    }
}

impl Args {
    pub fn from_env() -> Self {
        let mut args = Self::default();
        let mut it = std::env::args().skip(1);
        while let Some(arg) = it.next() {
            match arg.as_str() {
                "--connect" | "-c" => args.address = it.next().unwrap_or(args.address),
                "--layout" | "-l" => args.layout = it.next().map(Into::into),
                "--config" => args.config = it.next().map(Into::into),
                "--kanata" => args.kanata = it.next().map(Into::into),
                // Qt consumes its own flags (e.g. -platform); ignore the rest.
                _ => {}
            }
        }
        args
    }
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    // Bad placement options stop us before any window opens (contracts/cli.md).
    let settings_path = match settings::Overrides::parse(std::env::args().skip(1)) {
        Ok(overrides) => {
            let path = overrides
                .settings_path
                .clone()
                .or_else(settings::default_path);
            let _ = settings_state::OVERRIDES.set(overrides);
            path
        }
        Err(e) => {
            eprintln!("kanata-overlay: {e}");
            std::process::exit(2);
        }
    };

    // One overlay per settings file (feature 007, FR-008): a second start opens
    // the running one's settings instead.
    let socket = single_instance::socket_path(
        settings_path
            .as_deref()
            .unwrap_or(std::path::Path::new("kanata-overlay")),
    );
    match single_instance::claim(&socket) {
        Ok(single_instance::Claim::Secondary) => {
            println!("kanata-overlay is already running; opened its settings");
            return;
        }
        Ok(single_instance::Claim::Primary(listener)) => {
            if let Ok(mut l) = desktop_state::LISTENER.lock() {
                *l = Some(listener);
            }
        }
        Err(e) => tracing::warn!("single instance: {}: {e}", socket.display()),
    }

    // Known to the desktop (FR-009): menu entry and icon, pointing at
    // something that starts outside the development shell (research R9).
    if let Some(data) = desktop_entry::data_home() {
        match desktop_entry::launch_target(&data, Args::from_env().kanata.as_deref()) {
            Ok(target) => {
                if let Err(e) = desktop_entry::install(&data, &target) {
                    tracing::warn!("desktop entry: {e}");
                }
                // A login entry from an earlier build follows the new target.
                if let Some(login) =
                    desktop_entry::config_home().map(|c| desktop_entry::autostart_path(&c))
                    && login.is_file()
                    && let Err(e) = desktop_entry::write_if_changed(
                        &login,
                        &desktop_entry::autostart_entry(&target),
                    )
                {
                    tracing::warn!("login entry: {e}");
                }
                let _ = desktop_state::LAUNCH.set(target);
            }
            Err(e) => tracing::warn!("launch target: {e}"),
        }
    }

    // Wayland app id of our windows, and KGlobalAccel's view of who we are.
    platform::set_desktop_file_name(desktop_entry::APP_ID);
    // Without a platform theme (e.g. Nix's Qt, which can't load the desktop's
    // own Qt plugins), file dialogs fall back to a bare QML one: no hidden
    // files, no typed paths. The portal gives the desktop's real dialog.
    if std::env::var_os("QT_QPA_PLATFORMTHEME").is_none() {
        // SAFETY: still single-threaded; nothing reads the environment concurrently.
        unsafe { std::env::set_var("QT_QPA_PLATFORMTHEME", "xdgdesktopportal") };
    }
    let mut app = QGuiApplication::new();
    if let Some(app) = app.as_mut() {
        app.set_application_name(&QString::from("kanata-overlay"));
    }
    let mut engine = QQmlApplicationEngine::new();

    if let Some(engine) = engine.as_mut() {
        engine.load(&QUrl::from(
            "qrc:/qt/qml/io/github/jolitp/kanataoverlay/qml/Main.qml",
        ));
    }

    if let Some(app) = app.as_mut() {
        app.exec();
    }
}
