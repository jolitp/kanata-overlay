//! The overlay settings as QML sees them: file values with CLI overrides on top.
//!
//! All rules live in the `settings` crate; this is only the bridge.

use std::path::PathBuf;
use std::pin::Pin;
use std::sync::OnceLock;

use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;
use settings::{Edge, Field, Overrides, Rect, Settings, Size};

/// Command-line overrides, parsed in `main` before Qt starts.
pub static OVERRIDES: OnceLock<Overrides> = OnceLock::new();

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
        /// `top`, `bottom`, `left` or `right`.
        #[qproperty(QString, edge)]
        /// Logical pixels from the edge.
        #[qproperty(i32, margin)]
        /// 0.10 – 1.00.
        #[qproperty(f64, opacity)]
        /// Percent, as configured (before capping to fit the screen).
        #[qproperty(i32, scale)]
        /// Qt screen name; empty means the primary screen.
        #[qproperty(QString, screen_name)]
        /// The settings file in use, for messages.
        #[qproperty(QString, path)]
        type SettingsState = super::SettingsStateRust;

        /// Read the file and apply the command-line overrides.
        #[qinvokable]
        fn load(self: Pin<&mut SettingsState>);

        #[qinvokable]
        fn update_edge(self: Pin<&mut SettingsState>, edge: &QString);
        #[qinvokable]
        fn update_margin(self: Pin<&mut SettingsState>, margin: i32);
        #[qinvokable]
        fn update_opacity_percent(self: Pin<&mut SettingsState>, percent: i32);
        #[qinvokable]
        fn update_scale_percent(self: Pin<&mut SettingsState>, percent: i32);
        #[qinvokable]
        fn update_screen_name(self: Pin<&mut SettingsState>, name: &QString);

        /// Write the file if anything changed since the last save.
        #[qinvokable]
        fn save(self: Pin<&mut SettingsState>) -> bool;

        /// Window geometry for the board on a screen, as JSON
        /// `{x, y, width, height, scale, capped}`.
        #[qinvokable]
        fn placement(
            self: &SettingsState,
            screen_x: f64,
            screen_y: f64,
            screen_w: f64,
            screen_h: f64,
            base_w: f64,
            base_h: f64,
        ) -> QString;

        /// Something about the settings the user should know.
        #[qsignal]
        fn warning(self: Pin<&mut SettingsState>, text: QString);
    }
}

pub struct SettingsStateRust {
    edge: QString,
    margin: i32,
    opacity: f64,
    scale: i32,
    screen_name: QString,
    path: QString,

    file: Settings,
    overrides: Overrides,
    file_path: Option<PathBuf>,
    dirty: bool,
}

impl Default for SettingsStateRust {
    fn default() -> Self {
        let mut this = Self {
            edge: QString::default(),
            margin: 0,
            opacity: 0.0,
            scale: 0,
            screen_name: QString::default(),
            path: QString::default(),
            file: Settings::default(),
            overrides: Overrides::default(),
            file_path: None,
            dirty: false,
        };
        this.copy_from(&Settings::default());
        this
    }
}

impl SettingsStateRust {
    fn copy_from(&mut self, s: &Settings) {
        self.edge = QString::from(s.edge.as_str());
        self.margin = s.margin as i32;
        self.opacity = f64::from(s.opacity) / 100.0;
        self.scale = i32::from(s.scale);
        self.screen_name = QString::from(s.screen.as_deref().unwrap_or(""));
    }
}

impl qobject::SettingsState {
    fn load(mut self: Pin<&mut Self>) {
        let overrides = OVERRIDES.get().cloned().unwrap_or_default();
        let path = overrides
            .settings_path
            .clone()
            .or_else(settings::default_path);
        let outcome = match &path {
            Some(path) => settings::load(path),
            None => settings::LoadOutcome {
                settings: Settings::default(),
                warnings: Vec::new(),
                writable: false,
            },
        };
        if path.is_none() {
            self.as_mut().warning(QString::from(
                "settings: neither XDG_CONFIG_HOME nor HOME is set, settings will not be saved",
            ));
        }
        for w in &outcome.warnings {
            tracing::warn!("{w}");
            self.as_mut().warning(QString::from(&w.to_string()));
        }

        let display = path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        {
            let mut rust = self.as_mut().rust_mut();
            rust.file = outcome.settings;
            rust.overrides = overrides;
            rust.file_path = path;
            rust.dirty = false;
        }
        self.as_mut().set_path(QString::from(&display));
        self.publish();
    }

    fn update_edge(self: Pin<&mut Self>, edge: &QString) {
        if let Ok(edge) = edge.to_string().parse::<Edge>() {
            self.change(Field::Edge, |s| s.edge = edge);
        }
    }

    fn update_margin(self: Pin<&mut Self>, margin: i32) {
        let range = settings::MARGIN_RANGE;
        let margin = (margin.max(0) as u32).clamp(*range.start(), *range.end());
        self.change(Field::Margin, |s| s.margin = margin);
    }

    fn update_opacity_percent(self: Pin<&mut Self>, percent: i32) {
        let range = settings::OPACITY_RANGE;
        let percent = percent.clamp(i32::from(*range.start()), i32::from(*range.end())) as u8;
        self.change(Field::Opacity, |s| s.opacity = percent);
    }

    fn update_scale_percent(self: Pin<&mut Self>, percent: i32) {
        let range = settings::SCALE_RANGE;
        let percent = percent.clamp(i32::from(*range.start()), i32::from(*range.end())) as u16;
        self.change(Field::Scale, |s| s.scale = percent);
    }

    fn update_screen_name(self: Pin<&mut Self>, name: &QString) {
        let name = name.to_string();
        let screen = (!name.is_empty()).then_some(name);
        self.change(Field::Screen, |s| s.screen = screen);
    }

    fn save(mut self: Pin<&mut Self>) -> bool {
        if !self.rust().dirty {
            return true;
        }
        let Some(path) = self.rust().file_path.clone() else {
            return false;
        };
        match settings::save(&path, &self.rust().file) {
            Ok(()) => {
                self.as_mut().rust_mut().dirty = false;
                true
            }
            Err(e) => {
                tracing::warn!("could not save settings to {}: {e}", path.display());
                self.as_mut()
                    .warning(QString::from(&format!("could not save settings: {e}")));
                false
            }
        }
    }

    fn placement(
        &self,
        screen_x: f64,
        screen_y: f64,
        screen_w: f64,
        screen_h: f64,
        base_w: f64,
        base_h: f64,
    ) -> QString {
        let effective = self.rust().overrides.effective(&self.rust().file);
        let p = settings::place(
            Rect {
                x: screen_x as i32,
                y: screen_y as i32,
                width: screen_w as i32,
                height: screen_h as i32,
            },
            Size {
                width: base_w.ceil() as i32,
                height: base_h.ceil() as i32,
            },
            effective.edge,
            effective.margin,
            effective.scale,
        );
        let json = serde_json::json!({
            "x": p.x,
            "y": p.y,
            "width": p.size.width,
            "height": p.size.height,
            "scale": p.scale,
            "capped": p.capped,
        });
        QString::from(&json.to_string())
    }

    /// Apply a user edit: it wins over the command line and is saved later.
    fn change(mut self: Pin<&mut Self>, field: Field, edit: impl FnOnce(&mut Settings)) {
        {
            let mut rust = self.as_mut().rust_mut();
            edit(&mut rust.file);
            rust.overrides.clear(field);
            rust.dirty = true;
        }
        self.publish();
    }

    /// Push the effective settings to the QML properties.
    fn publish(mut self: Pin<&mut Self>) {
        let effective = self.rust().overrides.effective(&self.rust().file);
        let mut values = SettingsStateRust::default();
        values.copy_from(&effective);
        self.as_mut().set_edge(values.edge);
        self.as_mut().set_margin(values.margin);
        self.as_mut().set_opacity(values.opacity);
        self.as_mut().set_scale(values.scale);
        self.as_mut().set_screen_name(values.screen_name);
    }
}
