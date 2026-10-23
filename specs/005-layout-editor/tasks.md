---
description: "Task list for 005-layout-editor"
---

# Tasks: Layout Editor

**Input**: Design documents from `/specs/005-layout-editor/`

**Prerequisites**: [plan.md](plan.md), [spec.md](spec.md), [research.md](research.md), [data-model.md](data-model.md), [contracts/interfaces.md](contracts/interfaces.md), [quickstart.md](quickstart.md)

**Tests**: Included for the Qt-free crates (`layout::edit`, `kbd` key names), as the plan and constitution V require. The editor window is checked by hand: quickstart §2 with the fake Kanata (agent), §3 with the user's devices.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependency on unfinished tasks).
- **[Story]**: The user story the task belongs to (US1–US3 from spec.md).

## Conventions for every task

- Work inside `nix develop`. `just check` must pass before each commit.
- **Rust owns state (I)**: the edited layout, selection, undo and diagnostics live in `layout::edit`. QML may only show transient drag offsets that it then commits through one invokable (research R4).
- **No key presses (VI, FR-019)**: nothing in this feature reads input events.
- **Never write the Kanata config (FR-018)**; **never restart Kanata for a layout change (FR-015)**.
- **Agent smoke tests use the fake Kanata** (`--kanata crates/kanata-client/tests/fake-kanata.sh`) and a separate `--settings` file, never real Kanata or the user's settings.
- Build the binary (`cargo build -p kanata-overlay`) before a smoke test: `just check` doesn't rebuild it.

---

## Phase 1: Setup

- [ ] T001 Create `crates/layout/src/edit.rs` with a module doc comment (research R1) and declare `pub mod edit;` in `crates/layout/src/lib.rs`.
- [ ] T002 [P] Create empty `crates/app/src/layout_editor.rs` (bridge module skeleton with `#[auto_cxx_name]`, `QObject` `LayoutEditor`), `crates/app/qml/LayoutEditorWindow.qml` and `crates/app/qml/EditorKey.qml`; register all three in `crates/app/build.rs` and add `mod layout_editor;` to `crates/app/src/main.rs`.

---

## Phase 2: Foundational (Blocking Prerequisites)

**⚠️ CRITICAL**: No user-story work can begin until this phase is complete.

### Key names

- [ ] T003 [P] In `crates/kbd/src/lib.rs` add `pub fn key_names() -> impl Iterator<Item = (&'static str, &'static str)>` (every `KEY_ALIASES` entry: name, canonical) and `pub fn is_key_name(name: &str) -> bool` (binary search in `KEY_ALIASES`). Tests: `is_key_name("caps")`, `("ret")`, `("f13")`, `("mbck")` true; `("nonsense")`, `("")`, `("KEY_ENTER")` false unless the table lists it; `key_names()` count equals `KEY_ALIASES.len()`.

### Editing model (`crates/layout/src/edit.rs`)

- [ ] T004 Geometry helpers (research R3), with tests:
  - `fn centre(&Key) -> (f64, f64)` (the rotated rectangle's centre, absolute) and `fn corners(&Key) -> [(f64, f64); 4]`;
  - `fn normalise(&mut Key)`: rewrite to the equivalent form whose `rx, ry` is the key's own centre; test that all four corners stay within 1e-9 for rotated keys with arbitrary pivots, and that unrotated keys are unchanged;
  - `fn rotate_about(&mut Key, pivot, d)`: centre moves around the pivot, `rotation += d`, normalised to "(−180, 180]";
  - `fn translate(&mut Key, dx, dy)` (moves `x, y, rx, ry`).
- [ ] T005 `pub struct Editor` per data-model.md › Editor: `layout`, `selection` (ordered, "always valid indices"), `undo`/`redo` stacks of `(Layout, selection)` snapshots with undo "capped at 200", `saved` snapshot, `snap: bool`; `Editor::new(layout)`, `dirty()`, `can_undo()`, `can_redo()`, `undo()`, `redo()`, `mark_saved()`; one private `commit(f)` that snapshots, applies, clears redo. Constants `SNAP_UNIT = 0.25`, `SNAP_ANGLE = 5.0`. Tests: undo/redo round trip, cap at 200, redo cleared by a new change, dirty after undo back to saved is false.
- [ ] T006 Selection operations in `crates/layout/src/edit.rs`: `select(indices, Mode::{Replace, Add, Toggle})` (out-of-range indices ignored), `select_all`, `select_none`, `select_rect(x, y, w, h, mode)` (keys whose centre is inside). None records an undo step. Tests for each mode.
- [ ] T007 Geometry operations, each one undo step, with tests (data-model.md › Operations):
  - `move_by(dx, dy)` (snapped to `SNAP_UNIT` when snapping: the selection's bounding-box corner lands on the grid);
  - `rotate_by(d)` around the selection centre (snapped to `SNAP_ANGLE`);
  - `resize(Edge, delta)` on a single selected key in its own frame: "that edge moves", centre shifts by half the change along the rotated axis; width/height clamped to "≥ 0.25u, ≤ 20u";
  - `set_geometry(field, value)` for `x`, `y`, `width`, `height`, `rotation`; for several keys `x`/`y` move the selection's bounding box.
- [ ] T008 Content operations, each one undo step, with tests: `add_key(x, y)` ("new 1u unbound key, empty legend, selected alone"), `duplicate()` ("offset 0.5u right and down, selected"), `delete()` ("selection becomes empty"), `bind(Option<&str>) -> Result<(), BindError>` (single selection; rejected with `BindError::UnknownName` unless `kbd::is_key_name`; add `kbd.workspace = true` to `[dependencies]` in `crates/layout/Cargo.toml`), `set_legend(text)` (trimmed, and "longer input is cut to 32 characters").
- [ ] T009 `pub fn diagnose(layout, config_keys: &HashSet<String>) -> Diagnostics` (data-model.md › Diagnostics) with per-key `KeyFlags { unbound, duplicate, not_in_config, overlapping }` and counts. Overlap per research R9: rotated rectangles shrunk "0.05u per side", separating-axis test. Tests: two Esc keys are duplicates; `f13` with a config without it is not in config, and not flagged without a config; touching 1u keys don't overlap; a key half over another does; a 15°-rotated thumb key next to its neighbour doesn't.
- [ ] T010 Saving helpers, with tests using a temp dir: `pub fn normalise_origin(&mut Layout)` in `crates/layout/src/lib.rs` (shift so the minimum corner is (0, 0)); `pub fn save_target(current, current_is_own, layouts_dir, file_stem, exists: impl Fn(&Path) -> bool) -> Target { InPlace(PathBuf) | New(PathBuf) }` (data-model.md › Save target: "`-2`, `-3`… if it exists"); `Editor::normalise_origin()` (one undo step, none if already at (0, 0)), with a test that the layout written equals the editor's layout after saving; `pub fn write_atomic(path, text) -> io::Result<()>` (temp file in the same folder, then rename; creates the folder); `pub struct FileStamp { modified, len }` and `FileStamp::of(path) -> Option<FileStamp>` for the changed-on-disk check.

### App foundations

- [ ] T011 In `crates/app/src/device_state.rs`, make `apply()` reload only the layout when only `setup.layout` changed (research R6): compare config, address and check result for a restart; otherwise call `load_layout` and `relabel`. Verify with the fake Kanata that pointing a device at another layout file keeps the session (no "starting" state).
- [ ] T012 In `crates/app/src/device_state.rs`, add invokables `preview_layout(json)` (parse with `Layout::from_json`, annotate with `kanataCanonical`, set `layout_json`, relabel; no file access), `end_preview()` (re-run `load_layout` for the current setup), `reload_layout(path)` (reload if it equals `setup.layout`) and `config_keys_json()` (canonical `defsrc` names, `[]` without a config), per contracts/interfaces.md.
- [ ] T013 [P] In `crates/app/qml/Board.qml`, compute bounds from the minimum corner as well as the maximum and offset the key container by the minimum (research R10), so keys at negative coordinates show while previewing. With no keys the bounds are (0, 0, 0, 0): the board is its empty frame and header.

**Checkpoint**: `just check` passes; the app behaves as before (single and two-device smoke test with the fake Kanata).

---

## Phase 3: User Story 1 - Bind keys the layout doesn't know (Priority: P1) 🎯 MVP

**Goal**: open the editor for a device, see unbound/duplicate/not-in-config keys marked, select a key, bind it from the list, clear it, edit its legend.

**Independent Test**: quickstart §2 items 1 and 2.

- [ ] T014 [US1] Implement the `LayoutEditor` QObject in `crates/app/src/layout_editor.rs` per contracts/interfaces.md: properties `deviceId`, `layoutJson` (with `kanataCanonical`), `flagsJson`, `selectionJson`, `canUndo`, `canRedo`, `dirty`, `snap`, `snapUnit`, `snapAngle`, `error`; invokables `open`, `select`, `selectAll`, `selectNone`, `bind`, `setLegend`, `undo`, `redo`, `close`, `keyNames(filter)`. One private `publish()` that refreshes every property after each operation.
- [ ] T015 [US1] `keyNames(filter)` in `crates/app/src/layout_editor.rs` (research R8): groups `"unbound"` (config keys no layout key is bound to), `"config"` (the rest of the config's keys), `"other"` (one entry per remaining canonical key, other spellings in `aliases`); case-insensitive substring filter over name and aliases; at most 200 entries.
- [ ] T016 [US1] `crates/app/qml/EditorKey.qml`: draws a key like `KeyCap.qml` (same rotation transform) plus: a selection outline; flag marks (unbound: orange border; duplicate: "2×" badge; not in config: dashed border; overlapping: red tint); its bound name under the legend. Takes `key`, `flags`, `selected`, `unit`, `label`.
- [ ] T017 [US1] `crates/app/qml/LayoutEditorWindow.qml`, first version: an `ApplicationWindow` with a canvas (`Flickable` + zoom slider, keys via `Repeater` of `EditorKey` over `JSON.parse(editor.layoutJson).keys`), a device selector in the toolbar (until T022 adds the unsaved-changes dialog, switching discards), click to select, Shift/Ctrl+click to add/toggle, Ctrl+A, Undo/Redo buttons with Ctrl+Z / Ctrl+Shift+Z; a status line with the flag counts (FR-005); an inspector with the legend field and a binding field whose popup lists `editor.keyNames(text)` by group, with "Clear binding". Labels come from the device's `labelsJson`.
- [ ] T018 [US1] Wire it up: `crates/app/qml/Main.qml` creates one `LayoutEditor` and one `LayoutEditorWindow`; `crates/app/qml/SettingsWindow.qml` gets **Edit layout…** beside the layout's **Browse…**, which calls `editor.open(settings.device(id), deviceState.layoutJson, deviceState.configKeysJson())` and shows the window; after every operation the window calls `deviceState.previewLayout(editor.layoutJson)` (FR-013).
- [ ] T019 [US1] Validate quickstart §2 items 1–2 with the fake Kanata (agent), with a screenshot of the editor and the board.

**Checkpoint**: US1 works, without saving yet.

---

## Phase 4: User Story 3 - Save and use the layout (Priority: P1) 🎯 MVP

**Goal**: save in the app's format (in place or a new file the device then uses), without restarting Kanata; ask on close with unsaved changes; report failures.

**Independent Test**: quickstart §2 items 4–8.

- [ ] T020 [US3] In `crates/app/src/layout_editor.rs` add `saveTarget()` and `save(overwrite)` (contracts/interfaces.md): target from `save_target` with the device's layout path, `settings::layouts_dir()` (add `pub fn layouts_dir()` beside the settings path helper in `crates/settings/src/file.rs`) and the file stem `settings::slug(name)`, or the id if that is empty; `Editor::normalise_origin()` on the editor itself, then `write_atomic` of its layout; `FileStamp` check against the one taken at `open` (`error = "changed"` unless `overwrite`); `mark_saved`; emit `saved(deviceId, path, isNew)`. Add `reload()` (reread the target file into a fresh `Editor`).
- [ ] T021 [US3] In `crates/app/qml/Main.qml`, on `editor.saved`: if `isNew`, `settings.updateDevice(id, "layout", path)` and `settings.save()`; then call `reloadLayout(path)` on every device state (shared files, research R7). Thanks to T011 the device's Kanata is not restarted (FR-015).
- [ ] T022 [US3] In `crates/app/qml/LayoutEditorWindow.qml`: **Save** button and Ctrl+S; window title `Layout — <device> — <target file>` with `•` when dirty; on close or device switch with `editor.dirty`, a Save / Discard / Cancel dialog (FR-016); Discard calls `deviceState.endPreview()`; `error` shown in a banner, with **Overwrite** / **Reload** when it is `"changed"` (FR-017).
- [ ] T023 [US3] Device removed while editing (edge case): in `crates/app/qml/LayoutEditorWindow.qml`, when the device id disappears from `settings.deviceIds` and the editor is dirty, offer a `FileDialog` to save elsewhere (add `saveAs(path)` to `crates/app/src/layout_editor.rs`), then close.
- [ ] T024 [US3] Validate quickstart §2 items 4–8 with the fake Kanata (agent): new file created and selected, session not restarted, persistence after restart, discard, KLE source untouched (`git diff --exit-code examples/`), failed save with a read-only folder.

**Checkpoint**: MVP: bind, save and use. Commit point.

---

## Phase 5: User Story 2 - Arrange keys to match the device (Priority: P2)

**Goal**: drag to move, rectangle selection, resize and rotate handles, exact fields, nudges, add/duplicate/delete, snapping toggle, undo/redo controls.

**Independent Test**: quickstart §2 item 3.

- [ ] T025 [US2] Add invokables `selectRect`, `moveSelection`, `rotateSelection`, `resizeSelection`, `setGeometry`, `addKey`, `duplicateSelection`, `deleteSelection` and the `snap` setter to `crates/app/src/layout_editor.rs`, each calling the T006–T008 operations and `publish()`.
- [ ] T026 [US2] Dragging in `crates/app/qml/LayoutEditorWindow.qml` and `crates/app/qml/EditorKey.qml` (research R4): dragging a selected key applies a transient offset to all selected delegates, rounded to `editor.snapUnit` when non-zero; on release one `moveSelection(dx, dy)` call. Dragging on the background draws a rectangle; on release `selectRect` (Shift: add).
- [ ] T027 [US2] Handles on a single selection in `crates/app/qml/EditorKey.qml`: four edge handles (transient size, then `resizeSelection(edge, delta)`) and a rotate handle above the key (transient angle around the key centre, rounded to `editor.snapAngle`, then `rotateSelection(d)`); for several selected keys, one rotate handle above the selection's bounding box rotating around its centre.
- [ ] T028 [US2] Inspector and toolbar in `crates/app/qml/LayoutEditorWindow.qml`: exact fields x, y, width, height, rotation (`setGeometry`; empty or mixed shown as "—" for several keys); toolbar Add, Duplicate, Delete, Snap toggle; shortcuts per research R11 (arrows nudge by the snap step or 0.25u, Shift+arrows by 1u, Delete, Ctrl+D; undo/redo are already in T017); "Add" places the key at the canvas centre.
- [ ] T029 [US2] Validate quickstart §2 item 3 with the fake Kanata (agent), including a 110-key KLE layout for drag smoothness (SC-005) and the board updating after each release (SC-003).

**Checkpoint**: all stories work.

---

## Phase 6: Polish & Cross-Cutting Concerns

- [ ] T030 [P] README.md: a "Layout editor" section (opening it, binding, arranging, where layouts are saved, that KLE exports are never overwritten) and mark roadmap item 5 done.
- [ ] T031 [P] CLAUDE.md: describe `layout::edit` and `crates/app/src/layout_editor.rs`, and the transient-drag rule.
- [ ] T032 User validation with real devices: quickstart §3 (Defy shape and SC-002 timing, mouse in-place save, binding timing SC-001, no interruption while saving, large layout).
- [ ] T033 Fill in the quickstart.md Validation log; `just check`; commit.

---

## Dependencies & Execution Order

- **Setup (T001–T002)** → **Foundational (T003–T013)** → **US1 (T014–T019)** → **US3 (T020–T024)** → **US2 (T025–T029)** → **Polish**.
- Within Foundational: T003 before T008 (bind validation); T004 before T005–T009; T010 independent of T005–T009; T011–T013 independent of the layout crate work.
- US3 needs US1's editor and window; US2 needs the window and the T006–T008 operations.

### Parallel opportunities

- T002 with T001.
- T003, T004, T010, T011, T013 touch different files and can proceed together.
- T030 and T031.

## Implementation Strategy

1. **MVP = US1 + US3** (both P1): bind and save. After T024 the Defy/mouse layouts can already be fixed up by binding and saving, and imported KLE layouts become fully bound.
2. **Then US2**: arranging, the bigger UI piece, on top of a working save path.
3. Commit after each checkpoint.
