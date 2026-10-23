# Research: Layout Editor

## R1. Where the editing logic lives

- **Decision**: a new `edit` module in the `layout` crate: an `Editor` that owns one `Layout`, the selection, the undo history and the dirty flag, with one method per operation (select, move, resize, rotate, add, delete, duplicate, bind, set legend, undo, redo). Diagnostics (unbound, duplicate, not in config, overlapping) are a pure function of the layout and the config's key names. The app wraps it in one QObject, `LayoutEditor`.
- **Rationale**: constitution I (Rust owns state, QML renders) and V (all of it testable with `cargo test`). The `layout` crate already owns the format and has no Qt.
- **Alternatives**: editing a JS copy of the layout in QML (untestable, breaks I); a generic command framework (more than this needs, VII).

## R2. Undo: snapshots, not commands

- **Decision**: every committed change pushes a copy of the whole layout onto the undo stack, capped at 200 entries; redo is cleared by a new change. "Dirty" is "differs from the last saved snapshot".
- **Rationale**: a 110-key layout is about 10 KB; 200 copies are 2 MB at most. One rule covers every operation, and undo can't drift out of sync with what an operation did (VII).
- **Alternatives**: inverse commands per operation (more code, more bugs, no benefit at this size).

## R3. Rotation: normalise to "around the key's own centre"

- **Decision**: the format keeps KLE's convention (rotate by `rotation` around the absolute point `rx, ry`), but every key the editor touches is first rewritten into an equivalent form whose `rx, ry` is the key's own centre (same position on screen). Then:
  - moving adds to `x, y, rx, ry`;
  - rotating a selection by `d` around a pivot (the selection's centre, or the key's centre for one key) moves each centre around the pivot and adds `d` to `rotation`, normalised to (−180°, 180°];
  - resizing works in the key's own frame: dragging an edge moves that edge only, so the centre shifts by half the change along the rotated axis.
- **Rationale**: KLE rotations around a shared, arbitrary point don't compose simply; rotations around each key's centre do. The rewrite is exact and the files stay readable by any KLE-convention reader (ours included).
- **Alternatives**: a new format version with centre-based keys (breaks old files for no gain); keeping arbitrary pivots (the composition maths is error-prone).

## R4. Drag performance: transient offsets in QML, one commit in Rust

- **Decision**: during a drag (move, resize handle, rotate handle) QML only applies a temporary visual offset, size or angle to the selected keys' delegates, snapped with the step Rust reports. On release QML calls one invokable (`moveSelection`, `resizeSelection`, `rotateSelection`) with the final delta; Rust applies, snaps, records one undo step and publishes the new layout. Exact fields and arrow-key nudges call the same invokables.
- **Rationale**: republishing the layout JSON rebuilds every key delegate; doing that 60 times a second would lag on 110 keys (SC-005). A transient offset is rendering, which QML may own (I); the layout itself changes only in Rust.
- **Alternatives**: a per-key list model updated incrementally (fast, but much more bridge code, VII); publishing on every move (simple, laggy).

## R5. Live preview on the board

- **Decision**: `DeviceState` gains `previewLayout(json)` (show this layout without touching files) and `endPreview()` (reload the saved one). The editor calls `previewLayout` after every commit and `endPreview` on discard or close. Labels are recomputed for the previewed layout as for a loaded one.
- **Rationale**: FR-013 and SC-003 (under 0.5 s; a commit happens on release, so the board follows each finished gesture). No file is written until Save.
- **Alternatives**: writing a temp file and pointing the device at it (touches settings, risks leaving it pointed there after a crash, SC-006).

## R6. Changing a device's layout must not restart its Kanata

- **Decision**: in `DeviceState::apply`, a change of the layout path alone reloads the layout (`load_layout`) and relabels; only a change of config, address or check result restarts the session.
- **Rationale**: FR-015. Today the layout is part of the setup that triggers a restart (feature 004), which would make every first save interrupt the keyboard for a moment.
- **Alternatives**: none worth keeping.

## R7. Where and how a layout is saved

- **Decision**:
  - own-format file (it parses with `Layout::from_json`) → updated in place;
  - no layout, the grid, or a KLE export → new file `layouts/<slug of the device name>.json` (falling back to the device id if the name gives an empty slug) in the overlay's config folder (`$XDG_CONFIG_HOME/kanata-overlay/`), with `-2`, `-3`… if taken; the device's `layout` setting is then pointed at it and settings are saved. Ids are fixed when a device is added (e.g. `new-device`) and don't follow renames, so the name gives the file the user expects (`defy.json`);
  - writes are atomic (temp file in the same folder, then rename), so a failed save leaves the old file intact (FR-017, SC-006);
  - on save, the editor's own layout is first shifted so its top-left corner is at (0, 0) (one undo step, skipped if already there), then written and marked saved, so the editor, the board and the file always agree.
- **External changes**: the editor remembers the file's modification time and size when it opens; if either differs at save time, the user chooses overwrite or reload.
- **Shared files**: after a save, every device whose layout is that path reloads it (a `layoutSaved(path)` signal that each `DeviceView` handles).
- **Rationale**: FR-014 to FR-017; never overwrites a file the user may still need (the KLE export, someone else's file name).
- **Alternatives**: always asking for a file name (slower; the user said the point is a board for each device); saving next to the Kanata config (mixes the user's Kanata folder with app data).

## R8. Choosing a key name

- **Decision**: the `kbd` crate exposes every key name Kanata accepts (`key_names()`, from the generated table, each with its canonical form) and `is_key_name(name)`. The picker lists:
  1. the config's `defsrc` keys that no layout key is bound to yet;
  2. the rest of the config's keys;
  3. every other Kanata key name, one entry per canonical key, its other spellings shown as hints.
  Typing filters all three. A name that `is_key_name` rejects is refused (FR-007).
- **Rationale**: FR-006 (Q1 = C). The generated table already lists exactly what Kanata accepts, and stays right when Kanata is bumped (the table is regenerated then).
- **Alternatives**: free text only (typos would silently unbind); only the config's keys (spec allows names not in the config).

## R9. Overlap detection

- **Decision**: two keys overlap when their rotated rectangles, each shrunk by 0.05u per side, intersect (separating-axis test on the two rectangles' four axes). All pairs are checked: about 6,000 pairs for 110 keys, recomputed on each commit.
- **Rationale**: the edge case "overlapping keys are marked". The shrink lets keys that only touch (normal on a keyboard) pass.
- **Alternatives**: axis-aligned bounding boxes (false overlaps for every rotated thumb key).

## R10. Board bounds with keys left of or above the origin

- **Decision**: `Board.qml`'s bounds use the minimum corner too, not just the maximum, and offset the keys by it. A layout with no keys has bounds (0, 0, 0, 0), so the board is the empty frame with its header. Save still normalises to (0, 0) (R7), so saved files are unaffected.
- **Rationale**: while editing (preview), keys may be dragged to negative coordinates; today they would be cut off.

## R11. The editor window

- **Decision**: `LayoutEditorWindow.qml`, an ordinary `ApplicationWindow` opened from the settings window's **Edit layout…** button, editing the device selected there. It holds a toolbar (device selector, add, duplicate, delete, undo, redo, snapping, save), a canvas with the keys at a zoomable scale, and an inspector for the selection (legend, binding picker, x, y, width, height, rotation). Only one editor exists; switching device in it asks to save or discard first.
- **Canvas**: click selects, Shift/Ctrl+click adds or removes, drag on the background draws a selection rectangle, Ctrl+A selects all, drag on a selected key moves the selection, handles on a single selection resize (edges) and rotate (a handle above the key), arrow keys nudge by the snap step (Shift: 1u), Delete deletes, Ctrl+Z/Ctrl+Shift+Z undo/redo, Ctrl+S saves, Ctrl+D duplicates.
- **Rationale**: FR-001, FR-002, FR-008 to FR-012; conventions any drawing tool uses.
