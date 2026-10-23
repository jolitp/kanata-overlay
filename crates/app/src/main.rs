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

mod global_shortcut;
mod kanata_state;
mod overlay_state;
mod platform;
mod settings_state;

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
    match settings::Overrides::parse(std::env::args().skip(1)) {
        Ok(overrides) => {
            let _ = settings_state::OVERRIDES.set(overrides);
        }
        Err(e) => {
            eprintln!("kanata-overlay: {e}");
            std::process::exit(2);
        }
    }

    // Wayland app id of our windows, and KGlobalAccel's view of who we are.
    platform::set_desktop_file_name("io.github.jolitp.kanataoverlay");
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
