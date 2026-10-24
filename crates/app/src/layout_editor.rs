//! The layout editor's QObject (feature 005, contracts/interfaces.md): wraps
//! one `layout::edit::Editor` for one device and publishes it as JSON
//! properties. QML draws the canvas and only shows transient drag offsets; every
//! change goes through an invokable here (research R4).

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::pin::Pin;

use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;
use layout::edit::{
    self, Edge, EditError, Editor, Field, FileStamp, Mode, SNAP_ANGLE, SNAP_UNIT, Target,
};
use layout::{Background, Layout};
use serde_json::{Value, json};

use crate::device_state::with_canonical_names;

/// At most this many entries in the key-name picker.
const PICKER_LIMIT: usize = 200;

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
        /// The device being edited; empty when no session is open.
        #[qproperty(QString, device_id)]
        #[qproperty(QString, device_name)]
        /// The layout being edited (keys carry `kanataCanonical`).
        #[qproperty(QString, layout_json)]
        /// `{keys: [{unbound, duplicate, notInConfig, overlapping}], counts}`.
        #[qproperty(QString, flags_json)]
        /// Selected key indices as a JSON array.
        #[qproperty(QString, selection_json)]
        #[qproperty(bool, can_undo)]
        #[qproperty(bool, can_redo)]
        #[qproperty(bool, dirty)]
        #[qproperty(bool, snap)]
        /// Snapping steps for transient drags: 0 when snapping is off.
        #[qproperty(f64, snap_unit)]
        #[qproperty(f64, snap_angle)]
        /// Last error ("changed" = the file changed on disk), empty if none.
        #[qproperty(QString, error)]
        /// The file Save writes.
        #[qproperty(QString, save_path)]
        type LayoutEditor = super::LayoutEditorRust;

        /// Start a session: the device (`SettingsState.device(id)`), its board
        /// (`DeviceState.layoutJson`), its config's key names
        /// (`DeviceState.configKeysJson()`) and the settings file in use.
        #[qinvokable]
        fn open(
            self: Pin<&mut LayoutEditor>,
            device: &QString,
            board: &QString,
            config_keys: &QString,
            settings_path: &QString,
        );

        #[qinvokable]
        fn close(self: Pin<&mut LayoutEditor>);

        /// mode: "replace", "add" or "toggle".
        #[qinvokable]
        fn select(self: Pin<&mut LayoutEditor>, indices: &QString, mode: &QString);
        #[qinvokable]
        fn select_all(self: Pin<&mut LayoutEditor>);
        #[qinvokable]
        fn select_none(self: Pin<&mut LayoutEditor>);
        #[qinvokable]
        fn select_rect(
            self: Pin<&mut LayoutEditor>,
            x: f64,
            y: f64,
            w: f64,
            h: f64,
            mode: &QString,
        );

        #[qinvokable]
        fn move_selection(self: Pin<&mut LayoutEditor>, dx: f64, dy: f64);
        #[qinvokable]
        fn rotate_selection(self: Pin<&mut LayoutEditor>, degrees: f64);
        /// edge: "left", "right", "top" or "bottom".
        #[qinvokable]
        fn resize_selection(self: Pin<&mut LayoutEditor>, edge: &QString, delta: f64);
        /// field: "x", "y", "width", "height" or "rotation".
        #[qinvokable]
        fn set_geometry(self: Pin<&mut LayoutEditor>, field: &QString, value: f64);
        #[qinvokable]
        fn set_snapping(self: Pin<&mut LayoutEditor>, on: bool);

        #[qinvokable]
        fn add_key(self: Pin<&mut LayoutEditor>, x: f64, y: f64);
        #[qinvokable]
        fn duplicate_selection(self: Pin<&mut LayoutEditor>);
        #[qinvokable]
        fn delete_selection(self: Pin<&mut LayoutEditor>);

        /// Bind the selected key; "" clears. False (and `error`) if refused.
        #[qinvokable]
        fn bind(self: Pin<&mut LayoutEditor>, name: &QString) -> bool;
        #[qinvokable]
        fn set_legend(self: Pin<&mut LayoutEditor>, text: &QString);

        /// The name to bind for a pressed key (feature 006): the config's own
        /// spelling of it, else Kanata's shortest plain-text one.
        #[qinvokable]
        fn name_for(self: &LayoutEditor, canonical: &QString) -> QString;

        /// Picker entries `[{name, canonical, aliases, group}]` (research R8).
        #[qinvokable]
        fn key_names(self: &LayoutEditor, filter: &QString) -> QString;

        /// Put a reference image under the keys (replacing any), sized to the
        /// layout's width.
        #[qinvokable]
        fn set_background_image(self: Pin<&mut LayoutEditor>, path: &QString);
        #[qinvokable]
        fn clear_background(self: Pin<&mut LayoutEditor>);
        /// Place the image: `x`, `y`, `width` (units) and `opacity` (0–1).
        #[qinvokable]
        fn set_background_geometry(
            self: Pin<&mut LayoutEditor>,
            x: f64,
            y: f64,
            width: f64,
            opacity: f64,
        );

        #[qinvokable]
        fn undo(self: Pin<&mut LayoutEditor>);
        #[qinvokable]
        fn redo(self: Pin<&mut LayoutEditor>);

        /// Save; returns the path, or "" with `error` set. `overwrite` skips the
        /// changed-on-disk check.
        #[qinvokable]
        fn save(self: Pin<&mut LayoutEditor>, overwrite: bool) -> QString;
        /// Save to a chosen file (the device was removed while editing).
        #[qinvokable]
        fn save_as(self: Pin<&mut LayoutEditor>, path: &QString) -> QString;
        /// Reread the save target, dropping the edits (after "changed").
        #[qinvokable]
        fn reload(self: Pin<&mut LayoutEditor>);

        /// After a save; `is_new`: the device should now use `path`.
        #[qsignal]
        fn saved(self: Pin<&mut LayoutEditor>, device_id: QString, path: QString, is_new: bool);
    }
}

#[derive(Default)]
pub struct LayoutEditorRust {
    device_id: QString,
    device_name: QString,
    layout_json: QString,
    flags_json: QString,
    selection_json: QString,
    can_undo: bool,
    can_redo: bool,
    dirty: bool,
    snap: bool,
    snap_unit: f64,
    snap_angle: f64,
    error: QString,
    save_path: QString,

    editor: Option<Editor>,
    /// The config's `defsrc` names as written.
    config_keys: Vec<String>,
    /// The device's layout file, if any.
    current: Option<PathBuf>,
    /// `current` is in the app's own format (saved in place).
    current_is_own: bool,
    /// `current` as it was when read, to notice edits made elsewhere.
    stamp: Option<FileStamp>,
    layouts_dir: PathBuf,
}

impl qobject::LayoutEditor {
    fn open(
        mut self: Pin<&mut Self>,
        device: &QString,
        board: &QString,
        config_keys: &QString,
        settings_path: &QString,
    ) {
        let device: Value = serde_json::from_str(&device.to_string()).unwrap_or(Value::Null);
        let text = |k: &str| {
            device
                .get(k)
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned()
        };
        let current = Some(text("layout"))
            .filter(|p| !p.is_empty())
            .map(PathBuf::from);
        let own = current
            .as_deref()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|t| Layout::from_json(&t).ok());
        // Start from the board as shown (the file, the grid or the sample),
        // but from the file itself when it is ours (no canonical-name extras).
        let layout = own
            .clone()
            .or_else(|| Layout::from_json(&board.to_string()).ok());
        let settings_path = PathBuf::from(settings_path.to_string());
        {
            let mut rust = self.as_mut().rust_mut();
            rust.config_keys = serde_json::from_str(&config_keys.to_string()).unwrap_or_default();
            rust.current_is_own = own.is_some();
            rust.stamp = current.as_deref().and_then(FileStamp::of);
            rust.current = current;
            rust.layouts_dir = settings::layouts_dir(&settings_path);
            rust.editor = Some(Editor::new(layout.unwrap_or_else(|| Layout {
                version: layout::FORMAT_VERSION,
                name: text("name"),
                keys: Vec::new(),
                background: None,
            })));
        }
        self.as_mut().set_device_id(QString::from(&text("id")));
        self.as_mut().set_device_name(QString::from(&text("name")));
        self.as_mut().set_error(QString::default());
        self.publish();
    }

    fn close(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().editor = None;
        self.as_mut().set_device_id(QString::default());
        self.as_mut().set_error(QString::default());
        self.publish();
    }

    /// Run an operation on the open session, then republish everything.
    fn edit(mut self: Pin<&mut Self>, f: impl FnOnce(&mut Editor) -> Result<(), EditError>) {
        let result = match self.as_mut().rust_mut().editor.as_mut() {
            Some(e) => f(e),
            None => return,
        };
        let error = result.err().map(|e| e.to_string()).unwrap_or_default();
        self.as_mut().set_error(QString::from(&error));
        self.publish();
    }

    fn select(self: Pin<&mut Self>, indices: &QString, mode: &QString) {
        let indices: Vec<usize> = serde_json::from_str(&indices.to_string()).unwrap_or_default();
        let mode = parse_mode(&mode.to_string());
        self.edit(|e| {
            e.select(&indices, mode);
            Ok(())
        });
    }

    fn select_all(self: Pin<&mut Self>) {
        self.edit(|e| {
            e.select_all();
            Ok(())
        });
    }

    fn select_none(self: Pin<&mut Self>) {
        self.edit(|e| {
            e.select_none();
            Ok(())
        });
    }

    fn select_rect(self: Pin<&mut Self>, x: f64, y: f64, w: f64, h: f64, mode: &QString) {
        let mode = parse_mode(&mode.to_string());
        self.edit(|e| {
            e.select_rect(x, y, w, h, mode);
            Ok(())
        });
    }

    fn move_selection(self: Pin<&mut Self>, dx: f64, dy: f64) {
        self.edit(|e| {
            e.move_by(dx, dy);
            Ok(())
        });
    }

    fn rotate_selection(self: Pin<&mut Self>, degrees: f64) {
        self.edit(|e| {
            e.rotate_by(degrees);
            Ok(())
        });
    }

    fn resize_selection(self: Pin<&mut Self>, edge: &QString, delta: f64) {
        let edge = match edge.to_string().as_str() {
            "left" => Edge::Left,
            "top" => Edge::Top,
            "bottom" => Edge::Bottom,
            _ => Edge::Right,
        };
        self.edit(|e| e.resize(edge, delta));
    }

    fn set_geometry(self: Pin<&mut Self>, field: &QString, value: f64) {
        let field = match field.to_string().as_str() {
            "x" => Field::X,
            "y" => Field::Y,
            "width" => Field::Width,
            "height" => Field::Height,
            "rotation" => Field::Rotation,
            _ => return,
        };
        if value.is_finite() {
            self.edit(|e| {
                e.set_geometry(field, value);
                Ok(())
            });
        }
    }

    fn set_snapping(self: Pin<&mut Self>, on: bool) {
        self.edit(|e| {
            e.snap = on;
            Ok(())
        });
    }

    fn add_key(self: Pin<&mut Self>, x: f64, y: f64) {
        self.edit(|e| {
            e.add_key(x, y);
            Ok(())
        });
    }

    fn duplicate_selection(self: Pin<&mut Self>) {
        self.edit(|e| {
            e.duplicate();
            Ok(())
        });
    }

    fn delete_selection(self: Pin<&mut Self>) {
        self.edit(|e| {
            e.delete();
            Ok(())
        });
    }

    fn bind(mut self: Pin<&mut Self>, name: &QString) -> bool {
        let name = name.to_string();
        self.as_mut().edit(|e| e.bind(Some(&name)));
        self.error().is_empty()
    }

    fn set_legend(self: Pin<&mut Self>, text: &QString) {
        let text = text.to_string();
        self.edit(|e| e.set_legend(&text));
    }

    fn set_background_image(self: Pin<&mut Self>, path: &QString) {
        let path = path.to_string();
        self.edit(|e| {
            let (w, _) = e.layout().size();
            let current = e.layout().background.clone();
            e.set_background(Some(Background {
                path,
                x: current.as_ref().map_or(0.0, |b| b.x),
                y: current.as_ref().map_or(0.0, |b| b.y),
                width: current
                    .as_ref()
                    .map_or(if w > 0.0 { w } else { 15.0 }, |b| b.width),
                opacity: current.as_ref().map_or(0.5, |b| b.opacity),
            }));
            Ok(())
        });
    }

    fn clear_background(self: Pin<&mut Self>) {
        self.edit(|e| {
            e.set_background(None);
            Ok(())
        });
    }

    fn set_background_geometry(self: Pin<&mut Self>, x: f64, y: f64, width: f64, opacity: f64) {
        if ![x, y, width, opacity].iter().all(|v| v.is_finite()) {
            return;
        }
        self.edit(|e| {
            if let Some(b) = e.layout().background.clone() {
                e.set_background(Some(Background {
                    x,
                    y,
                    width,
                    opacity,
                    ..b
                }));
            }
            Ok(())
        });
    }

    fn undo(self: Pin<&mut Self>) {
        self.edit(|e| {
            e.undo();
            Ok(())
        });
    }

    fn redo(self: Pin<&mut Self>) {
        self.edit(|e| {
            e.redo();
            Ok(())
        });
    }

    fn name_for(&self, canonical: &QString) -> QString {
        let canonical = canonical.to_string();
        let name = self
            .rust()
            .config_keys
            .iter()
            .find(|k| kbd::canonical(k) == canonical)
            .map(String::as_str)
            .or_else(|| kbd::preferred_name(&canonical))
            .unwrap_or_default();
        QString::from(name)
    }

    fn key_names(&self, filter: &QString) -> QString {
        let bound: HashSet<String> = self
            .rust()
            .editor
            .iter()
            .flat_map(|e| &e.layout().keys)
            .filter_map(|k| k.kanata.as_deref().map(kbd::canonical))
            .collect();
        let entries = picker_entries(&self.rust().config_keys, &bound, &filter.to_string());
        QString::from(&Value::Array(entries).to_string())
    }

    fn target(&self) -> Target {
        let rust = self.rust();
        let name = self.device_name().to_string();
        let stem = Some(settings::slug(&name))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| self.device_id().to_string());
        edit::save_target(
            rust.current.as_deref(),
            rust.current_is_own,
            &rust.layouts_dir,
            &stem,
            Path::exists,
        )
    }

    fn save(mut self: Pin<&mut Self>, overwrite: bool) -> QString {
        let target = self.target();
        if let Target::InPlace(path) = &target
            && !overwrite
            && FileStamp::of(path) != self.rust().stamp
        {
            self.as_mut().set_error(QString::from("changed"));
            return QString::default();
        }
        let is_new = matches!(target, Target::New(_));
        self.write(target.path().to_path_buf(), is_new)
    }

    fn save_as(self: Pin<&mut Self>, path: &QString) -> QString {
        self.write(PathBuf::from(path.to_string()), true)
    }

    fn write(mut self: Pin<&mut Self>, path: PathBuf, is_new: bool) -> QString {
        let text = {
            let mut rust = self.as_mut().rust_mut();
            let Some(editor) = rust.editor.as_mut() else {
                return QString::default();
            };
            editor.normalise_origin();
            editor.layout().to_json()
        };
        if let Err(e) = edit::write_atomic(&path, &text) {
            self.as_mut().set_error(QString::from(&format!(
                "cannot save {}: {e}",
                path.display()
            )));
            self.publish();
            return QString::default();
        }
        {
            let mut rust = self.as_mut().rust_mut();
            if let Some(e) = rust.editor.as_mut() {
                e.mark_saved();
            }
            rust.stamp = FileStamp::of(&path);
            rust.current = Some(path.clone());
            rust.current_is_own = true;
        }
        self.as_mut().set_error(QString::default());
        self.as_mut().publish();
        let path = QString::from(&path.to_string_lossy().into_owned());
        let id = self.device_id().clone();
        self.as_mut().saved(id, path.clone(), is_new);
        path
    }

    fn reload(mut self: Pin<&mut Self>) {
        let Some(path) = self.rust().current.clone() else {
            return;
        };
        let read = std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|t| Layout::from_json(&t).map_err(|e| e.to_string()));
        match read {
            Ok(layout) => {
                {
                    let mut rust = self.as_mut().rust_mut();
                    let snap = rust.editor.as_ref().is_none_or(|e| e.snap);
                    let mut editor = Editor::new(layout);
                    editor.snap = snap;
                    rust.editor = Some(editor);
                    rust.stamp = FileStamp::of(&path);
                    rust.current_is_own = true;
                }
                self.as_mut().set_error(QString::default());
            }
            Err(e) => {
                let message = format!("cannot read {}: {e}", path.display());
                self.as_mut().set_error(QString::from(&message));
            }
        }
        self.publish();
    }

    /// Refresh every property from the session.
    fn publish(mut self: Pin<&mut Self>) {
        let (layout, flags, selection, undo, redo, dirty, snap) = match &self.rust().editor {
            Some(e) => {
                let config: HashSet<String> = self
                    .rust()
                    .config_keys
                    .iter()
                    .map(|k| kbd::canonical(k))
                    .collect();
                let d = edit::diagnose(e.layout(), &config);
                (
                    with_canonical_names(&e.layout().to_json()),
                    serde_json::to_string(&d).unwrap_or_default(),
                    serde_json::to_string(e.selection()).unwrap_or_default(),
                    e.can_undo(),
                    e.can_redo(),
                    e.dirty(),
                    e.snap,
                )
            }
            None => (
                String::new(),
                String::from("{\"keys\":[],\"counts\":{}}"),
                String::from("[]"),
                false,
                false,
                false,
                true,
            ),
        };
        let path = if self.rust().editor.is_some() {
            self.target().path().to_string_lossy().into_owned()
        } else {
            String::new()
        };
        self.as_mut().set_layout_json(QString::from(&layout));
        self.as_mut().set_flags_json(QString::from(&flags));
        self.as_mut().set_selection_json(QString::from(&selection));
        self.as_mut().set_can_undo(undo);
        self.as_mut().set_can_redo(redo);
        self.as_mut().set_dirty(dirty);
        self.as_mut().set_snap(snap);
        self.as_mut()
            .set_snap_unit(if snap { SNAP_UNIT } else { 0.0 });
        self.as_mut()
            .set_snap_angle(if snap { SNAP_ANGLE } else { 0.0 });
        self.as_mut().set_save_path(QString::from(&path));
    }
}

fn parse_mode(mode: &str) -> Mode {
    match mode {
        "add" => Mode::Add,
        "toggle" => Mode::Toggle,
        _ => Mode::Replace,
    }
}

/// The key-name picker (research R8): the config's keys not on the layout yet,
/// then the rest of the config's keys, then one entry per other Kanata key
/// (its shortest plain-text spelling, a few others as hints). The filter
/// matches any spelling.
/// Sort key for spellings: plain text first ("ret" before "⏎"), then shortest.
fn spelling_order<'a>(n: &&'a str) -> (bool, usize, &'a str) {
    (!n.is_ascii(), n.chars().count(), *n)
}

fn picker_entries(config_keys: &[String], bound: &HashSet<String>, filter: &str) -> Vec<Value> {
    let filter = filter.trim().to_lowercase();
    let mut by_canonical: std::collections::BTreeMap<&str, Vec<&str>> = Default::default();
    for (name, canonical) in kbd::key_names() {
        by_canonical.entry(canonical).or_default().push(name);
    }
    let spellings = |canonical: &str| -> Vec<&str> {
        let mut v: Vec<&str> = by_canonical
            .get(canonical)
            .into_iter()
            .flatten()
            .copied()
            .collect();
        v.sort_by_key(spelling_order);
        v
    };
    let aliases_of = |canonical: &str, except: &str| -> Vec<&str> {
        let mut v = spellings(canonical);
        v.retain(|n| *n != except);
        v.truncate(6);
        v
    };
    let matches = |name: &str, canonical: &str| {
        filter.is_empty()
            || name.to_lowercase().contains(&filter)
            || spellings(canonical)
                .iter()
                .any(|a| a.to_lowercase().contains(&filter))
    };
    let mut seen: HashSet<String> = HashSet::new();
    let mut unbound = Vec::new();
    let mut config = Vec::new();
    for name in config_keys {
        let canonical = kbd::canonical(name);
        if !seen.insert(canonical.clone()) {
            continue;
        }
        let aliases = aliases_of(&canonical, name);
        if matches(name, &canonical) {
            let group = if bound.contains(&canonical) {
                "config"
            } else {
                "unbound"
            };
            let entry =
                json!({"name": name, "canonical": canonical, "aliases": aliases, "group": group});
            if group == "unbound" {
                unbound.push(entry);
            } else {
                config.push(entry);
            }
        }
    }
    let mut other = Vec::new();
    for (canonical, names) in &by_canonical {
        if seen.contains(*canonical) {
            continue;
        }
        let Some(name) = names.iter().copied().min_by_key(spelling_order) else {
            continue;
        };
        let aliases = aliases_of(canonical, name);
        if matches(name, canonical) {
            other.push(
                json!({"name": name, "canonical": canonical, "aliases": aliases, "group": "other"}),
            );
        }
    }
    unbound
        .into_iter()
        .chain(config)
        .chain(other)
        .take(PICKER_LIMIT)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picker_puts_unbound_config_keys_first() {
        let config = vec!["esc".to_owned(), "caps".to_owned(), "a".to_owned()];
        let bound: HashSet<String> = [kbd::canonical("a")].into_iter().collect();
        let all = picker_entries(&config, &bound, "");
        let groups: Vec<&str> = all.iter().map(|e| e["group"].as_str().unwrap()).collect();
        assert_eq!(&groups[..3], ["unbound", "unbound", "config"]);
        assert_eq!(all[0]["name"], "esc");
        assert!(all.iter().skip(3).all(|e| e["group"] == "other"));
        // One entry per key: Enter appears once, as its shortest spelling.
        let enter: Vec<&Value> = all
            .iter()
            .filter(|e| e["canonical"] == "KEY_ENTER")
            .collect();
        assert!(enter.len() <= 1);
        let f13 = picker_entries(&config, &bound, "f13");
        assert_eq!(f13.len(), 1);
        assert_eq!(f13[0]["group"], "other");
        // Filtering matches other spellings too.
        let ret = picker_entries(&[], &HashSet::new(), "return");
        let enter = ret.iter().find(|e| e["canonical"] == "KEY_ENTER").unwrap();
        assert_eq!(enter["name"], "ent", "a plain-text spelling, not a glyph");
    }
}
