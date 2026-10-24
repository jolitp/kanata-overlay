//! Editing a layout (feature 005): an [`Editor`] owns one layout, the
//! selection and the undo history; [`diagnose`] marks unbound, duplicate,
//! not-in-config and overlapping keys; [`save_target`], [`write_atomic`] and
//! [`FileStamp`] decide where and how a layout is saved.
//!
//! Rotation follows KLE: a key is rotated by `rotation` degrees (clockwise,
//! y down) around the absolute point (`rx`, `ry`). Every key the editor touches
//! is first rewritten so that point is its own centre ([`normalise`]); then
//! moves, rotations and resizes compose simply (research R3).

use std::collections::HashSet;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::Serialize;

use crate::{Background, Key, Layout};

/// Snapping steps: positions and sizes, and angles.
pub const SNAP_UNIT: f64 = 0.25;
pub const SNAP_ANGLE: f64 = 5.0;
/// Key size limits, in units.
pub const MIN_SIZE: f64 = 0.25;
pub const MAX_SIZE: f64 = 20.0;
/// Longest legend kept; longer input is cut.
pub const LEGEND_MAX: usize = 32;
/// Reference image limits: width in units, opacity.
pub const BACKGROUND_WIDTH: (f64, f64) = (0.5, 200.0);
pub const BACKGROUND_OPACITY: (f64, f64) = (0.05, 1.0);
const UNDO_LIMIT: usize = 200;
/// Keys only touching don't overlap: each side is shrunk by this much first.
const OVERLAP_SHRINK: f64 = 0.05;

// ── geometry ────────────────────────────────────────────────────────────

type Point = (f64, f64);

/// `p` rotated by `deg` degrees (clockwise on screen) around `c`.
fn rotate_point(p: Point, c: Point, deg: f64) -> Point {
    let (s, co) = deg.to_radians().sin_cos();
    let (dx, dy) = (p.0 - c.0, p.1 - c.1);
    (c.0 + dx * co - dy * s, c.1 + dx * s + dy * co)
}

fn rect_corners(k: &Key, inset: f64) -> [Point; 4] {
    let (x0, y0) = (k.x + inset, k.y + inset);
    let (x1, y1) = (k.x + k.width - inset, k.y + k.height - inset);
    let c = (k.rx, k.ry);
    [(x0, y0), (x1, y0), (x1, y1), (x0, y1)].map(|p| rotate_point(p, c, k.rotation))
}

/// The four corners of a key as drawn (absolute units).
pub fn corners(k: &Key) -> [Point; 4] {
    rect_corners(k, 0.0)
}

/// The centre of a key as drawn.
pub fn centre(k: &Key) -> Point {
    rotate_point(
        (k.x + k.width / 2.0, k.y + k.height / 2.0),
        (k.rx, k.ry),
        k.rotation,
    )
}

/// Rewrite a rotated key so it rotates around its own centre; the key stays
/// exactly where it is drawn. Unrotated keys are left as they are.
pub fn normalise(k: &mut Key) {
    if k.rotation == 0.0 {
        return;
    }
    let c = centre(k);
    place(k, c);
}

/// Put a key's centre (and rotation centre) at `c`.
fn place(k: &mut Key, c: Point) {
    k.x = c.0 - k.width / 2.0;
    k.y = c.1 - k.height / 2.0;
    k.rx = c.0;
    k.ry = c.1;
}

/// An angle in (−180, 180].
fn normalise_angle(a: f64) -> f64 {
    let a = a.rem_euclid(360.0);
    if a > 180.0 { a - 360.0 } else { a }
}

/// Rotate a key by `d` degrees around `pivot`.
pub fn rotate_about(k: &mut Key, pivot: Point, d: f64) {
    let c = rotate_point(centre(k), pivot, d);
    k.rotation = normalise_angle(k.rotation + d);
    place(k, c);
}

/// Move a key.
pub fn translate(k: &mut Key, dx: f64, dy: f64) {
    k.x += dx;
    k.y += dy;
    k.rx += dx;
    k.ry += dy;
}

fn snap(v: f64, step: f64) -> f64 {
    (v / step).round() * step
}

/// Bounding box `(min, max)` of the drawn keys, `None` without keys.
fn bounds<'a>(keys: impl IntoIterator<Item = &'a Key>) -> Option<(Point, Point)> {
    keys.into_iter().flat_map(corners).fold(None, |b, p| {
        let ((x0, y0), (x1, y1)) = b.unwrap_or((p, p));
        Some(((x0.min(p.0), y0.min(p.1)), (x1.max(p.0), y1.max(p.1))))
    })
}

/// Shift a layout so its drawn top-left corner is at (0, 0).
pub fn normalise_origin(layout: &mut Layout) {
    if let Some(((x, y), _)) = bounds(&layout.keys)
        && (x.abs() > 1e-9 || y.abs() > 1e-9)
    {
        for k in &mut layout.keys {
            translate(k, -x, -y);
        }
        // The reference image stays aligned with the keys.
        if let Some(b) = &mut layout.background {
            b.x -= x;
            b.y -= y;
        }
    }
}

// ── editor ──────────────────────────────────────────────────────────────

/// How a selection request combines with the current selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Replace,
    Add,
    Toggle,
}

/// The edge of a key being dragged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    Left,
    Right,
    Top,
    Bottom,
}

/// A value set exactly in the inspector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    X,
    Y,
    Width,
    Height,
    Rotation,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EditError {
    #[error("select exactly one key")]
    NeedOneKey,
    #[error("“{0}” is not a key name Kanata knows")]
    UnknownName(String),
}

type Snapshot = (Layout, Vec<usize>);

/// One editing session (data-model › Editor).
#[derive(Debug, Clone)]
pub struct Editor {
    layout: Layout,
    selection: Vec<usize>,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    saved: Layout,
    /// Snap to [`SNAP_UNIT`] and [`SNAP_ANGLE`].
    pub snap: bool,
}

impl Editor {
    pub fn new(layout: Layout) -> Self {
        Self {
            saved: layout.clone(),
            layout,
            selection: Vec::new(),
            undo: Vec::new(),
            redo: Vec::new(),
            snap: true,
        }
    }

    pub fn layout(&self) -> &Layout {
        &self.layout
    }

    /// Selected key indices, in the order they were selected.
    pub fn selection(&self) -> &[usize] {
        &self.selection
    }

    /// Changed since opened or last saved.
    pub fn dirty(&self) -> bool {
        self.layout != self.saved
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn mark_saved(&mut self) {
        self.saved = self.layout.clone();
    }

    pub fn undo(&mut self) {
        if let Some(s) = self.undo.pop() {
            let current = (self.layout.clone(), self.selection.clone());
            self.redo.push(current);
            (self.layout, self.selection) = s;
        }
    }

    pub fn redo(&mut self) {
        if let Some(s) = self.redo.pop() {
            let current = (self.layout.clone(), self.selection.clone());
            self.undo.push(current);
            (self.layout, self.selection) = s;
        }
    }

    /// Apply a change as one undo step (none if nothing changed).
    fn commit(&mut self, f: impl FnOnce(&mut Self)) {
        let before = (self.layout.clone(), self.selection.clone());
        f(self);
        if self.layout != before.0 {
            self.undo.push(before);
            if self.undo.len() > UNDO_LIMIT {
                self.undo.remove(0);
            }
            self.redo.clear();
        }
    }

    fn single(&self) -> Result<usize, EditError> {
        match self.selection.as_slice() {
            [i] => Ok(*i),
            _ => Err(EditError::NeedOneKey),
        }
    }

    fn selected_keys(&self) -> impl Iterator<Item = &Key> {
        self.selection.iter().map(|&i| &self.layout.keys[i])
    }

    // ── selection (no undo steps) ──

    pub fn select(&mut self, indices: &[usize], mode: Mode) {
        let n = self.layout.keys.len();
        if mode == Mode::Replace {
            self.selection.clear();
        }
        for &i in indices.iter().filter(|&&i| i < n) {
            match self.selection.iter().position(|&s| s == i) {
                Some(p) if mode == Mode::Toggle => {
                    self.selection.remove(p);
                }
                Some(_) => {}
                None => self.selection.push(i),
            }
        }
    }

    pub fn select_all(&mut self) {
        self.selection = (0..self.layout.keys.len()).collect();
    }

    pub fn select_none(&mut self) {
        self.selection.clear();
    }

    /// Keys whose centre lies in the rectangle.
    pub fn select_rect(&mut self, x: f64, y: f64, w: f64, h: f64, mode: Mode) {
        let (x0, x1) = (x.min(x + w), x.max(x + w));
        let (y0, y1) = (y.min(y + h), y.max(y + h));
        let inside: Vec<usize> = (0..self.layout.keys.len())
            .filter(|&i| {
                let (cx, cy) = centre(&self.layout.keys[i]);
                (x0..=x1).contains(&cx) && (y0..=y1).contains(&cy)
            })
            .collect();
        self.select(&inside, mode);
    }

    // ── geometry ──

    /// Move the selection; with snapping its top-left lands on the grid.
    pub fn move_by(&mut self, dx: f64, dy: f64) {
        let Some(((x0, y0), _)) = bounds(self.selected_keys()) else {
            return;
        };
        let (dx, dy) = if self.snap {
            (snap(x0 + dx, SNAP_UNIT) - x0, snap(y0 + dy, SNAP_UNIT) - y0)
        } else {
            (dx, dy)
        };
        self.commit(|e| {
            for &i in &e.selection {
                translate(&mut e.layout.keys[i], dx, dy);
            }
        });
    }

    /// Rotate the selection around its centre.
    pub fn rotate_by(&mut self, d: f64) {
        let Some(((x0, y0), (x1, y1))) = bounds(self.selected_keys()) else {
            return;
        };
        let pivot = if let [i] = self.selection.as_slice() {
            centre(&self.layout.keys[*i])
        } else {
            ((x0 + x1) / 2.0, (y0 + y1) / 2.0)
        };
        let d = if self.snap { snap(d, SNAP_ANGLE) } else { d };
        self.commit(|e| {
            for &i in &e.selection {
                rotate_about(&mut e.layout.keys[i], pivot, d);
            }
        });
    }

    /// Drag one edge of the single selected key by `delta` units, in the
    /// key's own (rotated) frame; the opposite edge stays put.
    pub fn resize(&mut self, edge: Edge, delta: f64) -> Result<(), EditError> {
        let i = self.single()?;
        let snap_on = self.snap;
        self.commit(|e| {
            let k = &mut e.layout.keys[i];
            let size = |v: f64| {
                let v = if snap_on { snap(v, SNAP_UNIT) } else { v };
                v.clamp(MIN_SIZE, MAX_SIZE)
            };
            let c = centre(k);
            let (s, co) = k.rotation.to_radians().sin_cos();
            let (ux, uy) = ((co, s), (-s, co));
            let (along, change) = match edge {
                Edge::Left | Edge::Right => {
                    let w = size(k.width + delta);
                    let dw = w - k.width;
                    k.width = w;
                    (ux, if edge == Edge::Right { dw } else { -dw })
                }
                Edge::Top | Edge::Bottom => {
                    let h = size(k.height + delta);
                    let dh = h - k.height;
                    k.height = h;
                    (uy, if edge == Edge::Bottom { dh } else { -dh })
                }
            };
            let c = (c.0 + along.0 * change / 2.0, c.1 + along.1 * change / 2.0);
            if k.rotation == 0.0 {
                k.x = c.0 - k.width / 2.0;
                k.y = c.1 - k.height / 2.0;
            } else {
                place(k, c);
            }
        });
        Ok(())
    }

    /// Set a value exactly. For several keys, `x`/`y` move the selection's
    /// bounding box; size and rotation apply to each key around its centre.
    pub fn set_geometry(&mut self, field: Field, value: f64) {
        let Some(((x0, y0), _)) = bounds(self.selected_keys()) else {
            return;
        };
        let single = self.selection.len() == 1;
        self.commit(|e| {
            for &i in &e.selection {
                let k = &mut e.layout.keys[i];
                match field {
                    Field::X if single => {
                        normalise(k);
                        let d = value - k.x;
                        translate(k, d, 0.0);
                    }
                    Field::Y if single => {
                        normalise(k);
                        let d = value - k.y;
                        translate(k, 0.0, d);
                    }
                    Field::X => translate(k, value - x0, 0.0),
                    Field::Y => translate(k, 0.0, value - y0),
                    Field::Width | Field::Height => {
                        let c = centre(k);
                        let v = value.clamp(MIN_SIZE, MAX_SIZE);
                        if field == Field::Width {
                            k.width = v;
                        } else {
                            k.height = v;
                        }
                        if k.rotation == 0.0 {
                            k.x = c.0 - k.width / 2.0;
                            k.y = c.1 - k.height / 2.0;
                        } else {
                            place(k, c);
                        }
                    }
                    Field::Rotation => {
                        let c = centre(k);
                        rotate_about(k, c, value - k.rotation);
                    }
                }
            }
        });
    }

    // ── content ──

    /// A new 1u unbound key at (`x`, `y`), selected alone.
    pub fn add_key(&mut self, x: f64, y: f64) {
        let (x, y) = if self.snap {
            (snap(x, SNAP_UNIT), snap(y, SNAP_UNIT))
        } else {
            (x, y)
        };
        self.commit(|e| {
            e.layout.keys.push(Key {
                x,
                y,
                width: 1.0,
                height: 1.0,
                rotation: 0.0,
                rx: 0.0,
                ry: 0.0,
                kanata: None,
                legend: String::new(),
            });
            e.selection = vec![e.layout.keys.len() - 1];
        });
    }

    /// Copies of the selection, 0.5u right and down, selected.
    pub fn duplicate(&mut self) {
        self.commit(|e| {
            let first = e.layout.keys.len();
            let copies: Vec<Key> = e
                .selection
                .iter()
                .map(|&i| {
                    let mut k = e.layout.keys[i].clone();
                    translate(&mut k, 0.5, 0.5);
                    k
                })
                .collect();
            e.layout.keys.extend(copies);
            e.selection = (first..e.layout.keys.len()).collect();
        });
    }

    pub fn delete(&mut self) {
        self.commit(|e| {
            let mut sel = std::mem::take(&mut e.selection);
            sel.sort_unstable();
            for i in sel.into_iter().rev() {
                e.layout.keys.remove(i);
            }
        });
    }

    /// Bind the single selected key to a Kanata key name; `None` or an empty
    /// name clears the binding.
    pub fn bind(&mut self, name: Option<&str>) -> Result<(), EditError> {
        let i = self.single()?;
        let name = name.map(str::trim).filter(|n| !n.is_empty());
        if let Some(n) = name
            && !kbd::is_key_name(n)
        {
            return Err(EditError::UnknownName(n.to_owned()));
        }
        self.commit(|e| e.layout.keys[i].kanata = name.map(str::to_owned));
        Ok(())
    }

    /// Set the single selected key's printed legend (cut to [`LEGEND_MAX`]).
    pub fn set_legend(&mut self, text: &str) -> Result<(), EditError> {
        let i = self.single()?;
        let text: String = text.trim().chars().take(LEGEND_MAX).collect();
        self.commit(|e| e.layout.keys[i].legend = text);
        Ok(())
    }

    /// Set, change or remove the reference image (one undo step). Width and
    /// opacity are clamped to [`BACKGROUND_WIDTH`] and [`BACKGROUND_OPACITY`].
    pub fn set_background(&mut self, background: Option<Background>) {
        let background = background.map(|b| Background {
            width: b.width.clamp(BACKGROUND_WIDTH.0, BACKGROUND_WIDTH.1),
            opacity: b.opacity.clamp(BACKGROUND_OPACITY.0, BACKGROUND_OPACITY.1),
            ..b
        });
        self.commit(|e| e.layout.background = background);
    }

    /// Shift the layout to start at (0, 0), as one undo step (before saving).
    pub fn normalise_origin(&mut self) {
        self.commit(|e| normalise_origin(&mut e.layout));
    }
}

// ── diagnostics ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyFlags {
    pub unbound: bool,
    pub duplicate: bool,
    pub not_in_config: bool,
    pub overlapping: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Counts {
    pub unbound: usize,
    pub duplicate: usize,
    pub not_in_config: usize,
    pub overlapping: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Diagnostics {
    pub keys: Vec<KeyFlags>,
    pub counts: Counts,
}

/// Mark problem keys. `config_keys` are the canonical names in the config's
/// `defsrc`; empty when there is no config (then nothing is "not in config").
pub fn diagnose(layout: &Layout, config_keys: &HashSet<String>) -> Diagnostics {
    let canon: Vec<Option<String>> = layout
        .keys
        .iter()
        .map(|k| k.kanata.as_deref().map(kbd::canonical))
        .collect();
    let shapes: Vec<Option<[Point; 4]>> = layout
        .keys
        .iter()
        .map(|k| {
            (k.width > 2.0 * OVERLAP_SHRINK && k.height > 2.0 * OVERLAP_SHRINK)
                .then(|| rect_corners(k, OVERLAP_SHRINK))
        })
        .collect();
    let mut keys: Vec<KeyFlags> = canon
        .iter()
        .map(|c| KeyFlags {
            unbound: c.is_none(),
            duplicate: c.is_some() && canon.iter().filter(|o| *o == c).count() > 1,
            not_in_config: c
                .as_ref()
                .is_some_and(|c| !config_keys.is_empty() && !config_keys.contains(c)),
            overlapping: false,
        })
        .collect();
    for i in 0..shapes.len() {
        for j in i + 1..shapes.len() {
            if let (Some(a), Some(b)) = (&shapes[i], &shapes[j])
                && intersects(a, b)
            {
                keys[i].overlapping = true;
                keys[j].overlapping = true;
            }
        }
    }
    let count = |f: fn(&KeyFlags) -> bool| keys.iter().filter(|k| f(k)).count();
    let counts = Counts {
        unbound: count(|k| k.unbound),
        duplicate: count(|k| k.duplicate),
        not_in_config: count(|k| k.not_in_config),
        overlapping: count(|k| k.overlapping),
    };
    Diagnostics { keys, counts }
}

/// Separating-axis test for two convex quadrilaterals.
fn intersects(a: &[Point; 4], b: &[Point; 4]) -> bool {
    let axes =
        |q: &[Point; 4]| [(q[1], q[0]), (q[3], q[0])].map(|(p, o)| (-(p.1 - o.1), p.0 - o.0));
    let project = |q: &[Point; 4], ax: Point| {
        q.iter()
            .map(|p| p.0 * ax.0 + p.1 * ax.1)
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
                (lo.min(v), hi.max(v))
            })
    };
    axes(a).into_iter().chain(axes(b)).all(|ax| {
        let (a0, a1) = project(a, ax);
        let (b0, b1) = project(b, ax);
        a1 > b0 + 1e-9 && b1 > a0 + 1e-9
    })
}

// ── saving ──────────────────────────────────────────────────────────────

/// Where a save writes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// The device's own-format layout file, updated in place.
    InPlace(PathBuf),
    /// A new file in the layouts folder; the device is then pointed at it.
    New(PathBuf),
}

impl Target {
    pub fn path(&self) -> &Path {
        match self {
            Target::InPlace(p) | Target::New(p) => p,
        }
    }
}

/// Own-format files are updated in place; anything else (no layout, the grid,
/// a KLE export) gets `<dir>/<stem>.json`, or `<stem>-2.json`… if taken.
pub fn save_target(
    current: Option<&Path>,
    current_is_own: bool,
    dir: &Path,
    stem: &str,
    exists: impl Fn(&Path) -> bool,
) -> Target {
    if let Some(p) = current
        && current_is_own
    {
        return Target::InPlace(p.to_path_buf());
    }
    let mut n = 1;
    loop {
        let name = if n == 1 {
            format!("{stem}.json")
        } else {
            format!("{stem}-{n}.json")
        };
        let path = dir.join(name);
        if !exists(&path) {
            return Target::New(path);
        }
        n += 1;
    }
}

/// Write via a temporary file in the same folder and a rename, so a failed
/// write leaves the old file intact. Creates the folder.
pub fn write_atomic(path: &Path, text: &str) -> io::Result<()> {
    let dir = path
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    std::fs::create_dir_all(dir)?;
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy())
        .unwrap_or_default();
    let tmp = dir.join(format!(".{name}.tmp-{}", std::process::id()));
    let result = (|| {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(text.as_bytes())?;
        f.sync_all()?;
        std::fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

/// What a file looked like when the editor opened it, to notice changes made
/// elsewhere before overwriting them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileStamp {
    pub modified: Option<SystemTime>,
    pub len: u64,
}

impl FileStamp {
    pub fn of(path: &Path) -> Option<Self> {
        let m = std::fs::metadata(path).ok()?;
        Some(Self {
            modified: m.modified().ok(),
            len: m.len(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FORMAT_VERSION;

    fn key(x: f64, y: f64) -> Key {
        Key {
            x,
            y,
            width: 1.0,
            height: 1.0,
            rotation: 0.0,
            rx: 0.0,
            ry: 0.0,
            kanata: None,
            legend: String::new(),
        }
    }

    fn bound(x: f64, y: f64, name: &str) -> Key {
        Key {
            kanata: Some(name.into()),
            ..key(x, y)
        }
    }

    fn layout(keys: Vec<Key>) -> Layout {
        Layout {
            version: FORMAT_VERSION,
            name: "t".into(),
            keys,
            background: None,
        }
    }

    fn close(a: Point, b: Point) -> bool {
        (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9
    }

    fn same_corners(a: &Key, b: &Key) -> bool {
        corners(a).iter().zip(corners(b)).all(|(p, q)| close(*p, q))
    }

    #[test]
    fn normalise_keeps_the_key_in_place() {
        let mut k = Key {
            width: 1.5,
            rotation: 30.0,
            rx: -2.0,
            ry: 4.0,
            ..key(3.0, 1.0)
        };
        let before = k.clone();
        normalise(&mut k);
        assert!(same_corners(&before, &k));
        assert!(close((k.rx, k.ry), centre(&k)));
        let mut plain = key(1.0, 2.0);
        normalise(&mut plain);
        assert_eq!(plain, key(1.0, 2.0));
    }

    #[test]
    fn rotations_compose_and_moves_follow() {
        let mut a = Key {
            rotation: 10.0,
            rx: 5.0,
            ry: 5.0,
            ..key(1.0, 1.0)
        };
        let mut b = a.clone();
        rotate_about(&mut a, (0.0, 0.0), 20.0);
        rotate_about(&mut a, (0.0, 0.0), 15.0);
        rotate_about(&mut b, (0.0, 0.0), 35.0);
        assert!(same_corners(&a, &b));
        assert!((a.rotation - 45.0).abs() < 1e-9);
        let c = centre(&a);
        translate(&mut a, 1.0, -2.0);
        assert!(close(centre(&a), (c.0 + 1.0, c.1 - 2.0)));
        let c = centre(&a);
        rotate_about(&mut a, c, 170.0);
        assert!((a.rotation - -145.0).abs() < 1e-9);
    }

    #[test]
    fn undo_redo_and_dirty() {
        let mut e = Editor::new(layout(vec![key(0.0, 0.0)]));
        assert!(!e.dirty() && !e.can_undo());
        e.select(&[0], Mode::Replace);
        e.move_by(1.0, 0.0);
        assert!(e.dirty() && e.can_undo());
        e.undo();
        assert!(!e.dirty() && e.can_redo());
        assert_eq!(e.selection(), [0]);
        e.redo();
        assert_eq!(e.layout().keys[0].x, 1.0);
        e.undo();
        e.move_by(0.0, 1.0);
        assert!(!e.can_redo(), "a new change clears redo");
        e.mark_saved();
        assert!(!e.dirty());
        // Selection alone is not an undo step; neither is a no-op.
        let steps = e.undo.len();
        e.select_none();
        e.move_by(1.0, 1.0);
        assert_eq!(e.undo.len(), steps);
    }

    #[test]
    fn undo_is_capped() {
        let mut e = Editor::new(layout(vec![key(0.0, 0.0)]));
        e.select(&[0], Mode::Replace);
        for _ in 0..250 {
            e.move_by(1.0, 0.0);
        }
        assert_eq!(e.undo.len(), UNDO_LIMIT);
    }

    #[test]
    fn selection_modes() {
        let mut e = Editor::new(layout(vec![key(0.0, 0.0), key(1.0, 0.0), key(5.0, 5.0)]));
        e.select(&[0, 9], Mode::Replace);
        assert_eq!(e.selection(), [0]);
        e.select(&[1], Mode::Add);
        assert_eq!(e.selection(), [0, 1]);
        e.select(&[0, 2], Mode::Toggle);
        assert_eq!(e.selection(), [1, 2]);
        e.select_rect(-0.5, -0.5, 2.5, 1.5, Mode::Replace);
        assert_eq!(e.selection(), [0, 1]);
        e.select_all();
        assert_eq!(e.selection(), [0, 1, 2]);
        e.select_none();
        assert!(e.selection().is_empty());
    }

    #[test]
    fn move_snaps_the_selection_corner() {
        let mut e = Editor::new(layout(vec![key(0.1, 0.0), key(1.1, 0.0)]));
        e.select_all();
        e.move_by(0.6, 0.3);
        assert!((e.layout().keys[0].x - 0.75).abs() < 1e-9);
        assert!((e.layout().keys[1].x - 1.75).abs() < 1e-9);
        assert!((e.layout().keys[0].y - 0.25).abs() < 1e-9);
        e.snap = false;
        e.move_by(0.01, 0.0);
        assert!((e.layout().keys[0].x - 0.76).abs() < 1e-9);
    }

    #[test]
    fn rotate_snaps_and_uses_the_selection_centre() {
        let mut e = Editor::new(layout(vec![key(0.0, 0.0), key(1.0, 0.0)]));
        e.select_all();
        e.rotate_by(88.0);
        let k = &e.layout().keys;
        assert!((k[0].rotation - 90.0).abs() < 1e-9);
        // Rotated 90° around (1, 0.5): the row becomes a column.
        assert!(close(centre(&k[0]), (1.0, 0.0)));
        assert!(close(centre(&k[1]), (1.0, 1.0)));
    }

    #[test]
    fn resize_moves_one_edge_in_the_keys_frame() {
        let mut e = Editor::new(layout(vec![key(0.0, 0.0)]));
        e.select(&[0], Mode::Replace);
        e.resize(Edge::Right, 0.5).unwrap();
        let k = e.layout().keys[0].clone();
        assert_eq!((k.x, k.width), (0.0, 1.5));
        e.resize(Edge::Left, 0.25).unwrap();
        assert_eq!(
            (e.layout().keys[0].x, e.layout().keys[0].width),
            (-0.25, 1.75)
        );
        e.resize(Edge::Top, -5.0).unwrap();
        assert_eq!(e.layout().keys[0].height, MIN_SIZE);

        // Rotated 90°: widening to the "right" grows downwards on screen.
        let mut e = Editor::new(layout(vec![key(0.0, 0.0)]));
        e.select(&[0], Mode::Replace);
        e.set_geometry(Field::Rotation, 90.0);
        let left_before = corners(&e.layout().keys[0])[0];
        e.resize(Edge::Right, 1.0).unwrap();
        let k = &e.layout().keys[0];
        assert_eq!(k.width, 2.0);
        assert!(close(corners(k)[0], left_before), "the left edge stays put");
        assert!(close(centre(k), (0.5, 1.0)));
        // Two keys: resizing needs exactly one.
        e.add_key(3.0, 3.0);
        e.select_all();
        assert_eq!(e.resize(Edge::Right, 1.0), Err(EditError::NeedOneKey));
    }

    #[test]
    fn exact_geometry() {
        let mut e = Editor::new(layout(vec![key(0.0, 0.0), key(1.0, 1.0)]));
        e.select_all();
        e.set_geometry(Field::X, 3.0);
        assert_eq!((e.layout().keys[0].x, e.layout().keys[1].x), (3.0, 4.0));
        e.set_geometry(Field::Width, 2.0);
        assert_eq!(e.layout().keys[0].width, 2.0);
        assert!(close(centre(&e.layout().keys[0]), (3.5, 0.5)));
        e.set_geometry(Field::Height, 99.0);
        assert_eq!(e.layout().keys[1].height, MAX_SIZE);
        e.select(&[0], Mode::Replace);
        e.set_geometry(Field::Rotation, -30.0);
        assert!((e.layout().keys[0].rotation - -30.0).abs() < 1e-9);
        assert!(close(centre(&e.layout().keys[0]), (3.5, 0.5)));
        e.set_geometry(Field::Y, 2.0);
        assert!((e.layout().keys[0].y - 2.0).abs() < 1e-9);
    }

    #[test]
    fn add_duplicate_delete() {
        let mut e = Editor::new(layout(vec![bound(0.0, 0.0, "a")]));
        e.add_key(2.1, 0.0);
        assert_eq!(e.selection(), [1]);
        assert_eq!(e.layout().keys[1].x, 2.0);
        assert_eq!(e.layout().keys[1].kanata, None);
        e.select_all();
        e.duplicate();
        assert_eq!(e.layout().keys.len(), 4);
        assert_eq!(e.selection(), [2, 3]);
        assert_eq!(e.layout().keys[2].x, 0.5);
        assert_eq!(e.layout().keys[2].kanata.as_deref(), Some("a"));
        e.select(&[0, 2], Mode::Replace);
        e.delete();
        assert_eq!(e.layout().keys.len(), 2);
        assert!(e.selection().is_empty());
        assert_eq!(e.layout().keys[0].x, 2.0);
    }

    #[test]
    fn bind_and_legend() {
        let mut e = Editor::new(layout(vec![key(0.0, 0.0), key(1.0, 0.0)]));
        assert_eq!(e.bind(Some("caps")), Err(EditError::NeedOneKey));
        e.select(&[0], Mode::Replace);
        e.bind(Some(" caps ")).unwrap();
        assert_eq!(e.layout().keys[0].kanata.as_deref(), Some("caps"));
        assert_eq!(
            e.bind(Some("nonsense")),
            Err(EditError::UnknownName("nonsense".into()))
        );
        assert_eq!(e.layout().keys[0].kanata.as_deref(), Some("caps"));
        e.bind(Some("")).unwrap();
        assert_eq!(e.layout().keys[0].kanata, None);
        e.set_legend(&"x".repeat(40)).unwrap();
        assert_eq!(e.layout().keys[0].legend.chars().count(), LEGEND_MAX);
    }

    #[test]
    fn diagnostics() {
        let mut thumb = bound(2.3, 0.0, "spc");
        thumb.rotation = 15.0;
        thumb.rx = 2.8;
        thumb.ry = 0.5;
        let l = layout(vec![
            bound(0.0, 0.0, "esc"),
            bound(1.0, 0.0, "esc"),
            thumb,
            bound(0.0, 1.0, "f13"),
            key(0.5, 1.0),
        ]);
        let config: HashSet<String> = ["esc", "spc"].iter().map(|n| kbd::canonical(n)).collect();
        let d = diagnose(&l, &config);
        assert!(d.keys[0].duplicate && d.keys[1].duplicate);
        assert!(
            !d.keys[0].overlapping && !d.keys[1].overlapping,
            "touching keys"
        );
        assert!(
            !d.keys[2].overlapping,
            "a 15° thumb key next to its neighbour"
        );
        assert!(d.keys[3].not_in_config);
        assert!(d.keys[4].unbound);
        assert!(
            d.keys[3].overlapping && d.keys[4].overlapping,
            "half over another"
        );
        assert_eq!(
            d.counts,
            Counts {
                unbound: 1,
                duplicate: 2,
                not_in_config: 1,
                overlapping: 2
            }
        );
        // Without a config nothing is "not in config".
        assert!(!diagnose(&l, &HashSet::new()).keys[3].not_in_config);
    }

    #[test]
    fn origin_normalisation() {
        let mut e = Editor::new(layout(vec![key(-1.0, 2.0), key(1.0, 3.0)]));
        e.normalise_origin();
        assert_eq!((e.layout().keys[0].x, e.layout().keys[0].y), (0.0, 0.0));
        assert_eq!((e.layout().keys[1].x, e.layout().keys[1].y), (2.0, 1.0));
        let steps = e.undo.len();
        e.normalise_origin();
        assert_eq!(e.undo.len(), steps, "already at the origin: no step");
        let mut empty = layout(vec![]);
        normalise_origin(&mut empty);
        assert!(empty.keys.is_empty());
    }

    #[test]
    fn save_targets() {
        let dir = Path::new("/l");
        let own = Path::new("/x/mine.json");
        assert_eq!(
            save_target(Some(own), true, dir, "defy", |_| false),
            Target::InPlace(own.into())
        );
        assert_eq!(
            save_target(Some(own), false, dir, "defy", |_| false),
            Target::New("/l/defy.json".into())
        );
        let taken = |p: &Path| p.ends_with("defy.json") || p.ends_with("defy-2.json");
        assert_eq!(
            save_target(None, false, dir, "defy", taken),
            Target::New("/l/defy-3.json".into())
        );
    }

    #[test]
    fn atomic_write_and_stamps() {
        let dir = std::env::temp_dir().join(format!("layout-edit-{}", std::process::id()));
        let path = dir.join("sub/a.json");
        write_atomic(&path, "one").unwrap();
        let stamp = FileStamp::of(&path).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "one");
        write_atomic(&path, "longer").unwrap();
        assert_ne!(FileStamp::of(&path), Some(stamp));
        let leftovers = std::fs::read_dir(dir.join("sub")).unwrap().count();
        assert_eq!(leftovers, 1, "no temp file left behind");
        // A failed write (target is a folder) leaves no temp file either.
        assert!(write_atomic(&dir.join("sub"), "x").is_err());
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(FileStamp::of(&path), None);
    }

    #[test]
    fn saved_equals_edited() {
        let mut e = Editor::new(layout(vec![key(2.0, 2.0)]));
        e.normalise_origin();
        let json = e.layout().to_json();
        e.mark_saved();
        assert_eq!(Layout::from_json(&json).unwrap(), *e.layout());
        assert!(!e.dirty());
    }

    #[test]
    fn background_follows_the_origin_and_is_clamped() {
        let mut e = Editor::new(layout(vec![key(2.0, 3.0)]));
        e.set_background(Some(Background {
            path: "/p.png".into(),
            x: 1.0,
            y: 1.0,
            width: 9999.0,
            opacity: 0.0,
        }));
        let b = e.layout().background.clone().unwrap();
        assert_eq!(
            (b.width, b.opacity),
            (BACKGROUND_WIDTH.1, BACKGROUND_OPACITY.0)
        );
        e.normalise_origin();
        let b = e.layout().background.clone().unwrap();
        assert_eq!((b.x, b.y), (-1.0, -2.0), "moved with the keys");
        let json = e.layout().to_json();
        assert_eq!(Layout::from_json(&json).unwrap(), *e.layout());
        e.set_background(None);
        assert!(!e.layout().to_json().contains("background"));
        e.undo();
        assert!(e.layout().background.is_some());
    }
}
