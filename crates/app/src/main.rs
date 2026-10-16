//! kanata-overlay: shows the active Kanata layer on a picture of your keyboard.
//!
//!     kanata-overlay [--connect 127.0.0.1:7070] [--layout my-board.json]
//!
//! `--layout` accepts KLE JSON (keyboard-layout-editor.com) or the app's own
//! layout format. Without it, a small sample board is shown.

mod overlay_state;

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QUrl};

/// Command-line options (hand-parsed for now; swap for `clap` when they grow).
#[derive(Debug, Clone)]
pub struct Args {
    pub address: String,
    pub layout: Option<std::path::PathBuf>,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            address: "127.0.0.1:7070".to_owned(),
            layout: None,
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

    let mut app = QGuiApplication::new();
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
