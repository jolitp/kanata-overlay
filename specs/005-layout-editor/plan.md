# Implementation Plan: Layout Editor

**Branch**: `005-layout-editor` (stacked on `004-multi-device`) | **Date**: 2026-10-23 | **Spec**: [spec.md](spec.md)

## Summary

The settings window gets **Edit layout…**, which opens an editor window for the selected
device. It starts from the device's current board (its layout file, or the grid of its
config's keys). The user selects keys on a canvas, drags them, resizes and rotates them
with handles or exact fields, adds, duplicates and deletes keys, sets legends, and binds
keys to Kanata names picked from a list (the config's unbound keys first). Every change
can be undone and previews on the device's board. Save writes the app's own format: in
place for an own-format file, otherwise a new file in the overlay's `layouts/` folder
that the device then uses, without restarting its Kanata.

The editing model (operations, undo, rotation maths, diagnostics, save rules) is a pure
Rust module in the `layout` crate; one new QObject bridges it; QML draws the canvas and
the inspector and only shows transient offsets while a drag is in progress.

## Technical Context

**Language/Version**: Rust stable (edition 2024); QML (Qt 6.11)

**Primary Dependencies**: none new. `kbd` exposes the generated key-name table (`key_names`, `is_key_name`); the app already depends on `layout`, `kbd` and `settings`.

**Storage**: layout files, format version 1 (unchanged), under `$XDG_CONFIG_HOME/kanata-overlay/layouts/`; settings file v2 (the device's `layout` path).

**Testing**:
- `cargo test`: `layout::edit` (every operation with undo/redo, rotation normalisation and composition, resize in a rotated frame, snapping, diagnostics, overlap vs touching, save target, atomic write, changed-on-disk check); `kbd` key names.
- Manual: quickstart §2 with the fake Kanata (agent), §3 with the user's devices.

**Target Platform**: Linux, Plasma Wayland first; the editor is an ordinary window, so X11 is the same.

**Performance Goals**:
- board updated within 0.5 s of each finished gesture (SC-003);
- drag of 110 keys follows the pointer without visible lag (SC-005): transient offsets in QML during the drag, one commit on release (research R4).

**Constraints**:
- never restart a device's Kanata because its layout changed (FR-015, research R6);
- never write the Kanata config (FR-018); read no key presses (FR-019);
- a failed or discarded edit leaves the last saved layout on the board (SC-006).

**Scale/Scope**:
- layout: `edit.rs` about 450 lines plus tests;
- kbd: about 20 lines;
- app: 1 new QObject (`layout_editor.rs`, about 300 lines); `DeviceState` gains preview and reload (about 60 lines);
- QML: 1 new window with a canvas key delegate (about 500 lines); `SettingsWindow`, `DeviceView`, `Board` changed slightly.

## Constitution Check

| Principle | Status | How |
|-----------|--------|-----|
| I. Rust owns state, QML renders | ✅ | The layout being edited, the selection, undo, snapping, diagnostics and save rules live in `layout::edit`. QML renders and, during a drag only, shows a transient offset that Rust then commits (research R4). |
| II. Stable Kanata interfaces | ✅ | Nothing new talks to Kanata. Key names come from the table generated from Kanata's source. |
| III. Never strand the keyboard | ✅ | A layout change no longer restarts Kanata (R6); saving never touches the Kanata config or the session. |
| IV. Plasma Wayland first | ✅ | The editor is an ordinary window; the overlay boards stay layer-shell, click-through. |
| V. Testable without hardware | ✅ | All editing logic is pure and unit-tested; the app flow is checked with the fake Kanata (quickstart §2). |
| VI. Key events are private | ✅ | The editor reads no key presses at all; press-to-bind waits for roadmap item 6 (clarification Q1). |
| VII. Simple first | ✅ | Snapshot undo (R2), one format, no new dependencies, a JSON-property bridge like the existing QObjects. |

**Gate**: PASS. **Post-design**: PASS.

## Project Structure

```text
crates/layout/src/
├── lib.rs          # + pub mod edit; normalise_origin()
└── edit.rs         # NEW: Editor, operations, undo, rotation maths, diagnose(), save_target(), write_atomic()
crates/kbd/src/lib.rs           # + key_names(), is_key_name()
crates/settings/src/file.rs     # + layouts_dir()
crates/app/
├── build.rs                    # + layout_editor.rs, LayoutEditorWindow.qml, EditorKey.qml
├── src/layout_editor.rs        # NEW: LayoutEditor QObject
├── src/device_state.rs         # + previewLayout, endPreview, reloadLayout, configKeysJson; layout-only apply
├── src/main.rs                 # + mod layout_editor
└── qml/
    ├── LayoutEditorWindow.qml  # NEW: toolbar, canvas, inspector, key-name picker, save/discard dialog
    ├── EditorKey.qml           # NEW: a key on the canvas: flags, selection outline, handles
    ├── SettingsWindow.qml      # + Edit layout…
    ├── Main.qml                # + the editor window, layoutSaved fan-out
    ├── DeviceView.qml          # + reload on layoutSaved
    └── Board.qml               # bounds from the minimum corner too (R10)
```

## Implementation Notes

- **Order**:
  1. Foundational: `kbd` key names; `layout::edit` with all operations and tests; `DeviceState` layout-only apply (R6) and preview; Board bounds (R10).
  2. US1 + US3 (both P1, the MVP): editor window with canvas selection, inspector with legend and the key-name picker, flags; save, save target, atomic write, setting update, discard/close dialog, shared-file reload.
  3. US2: drag to move, selection rectangle, resize and rotate handles, exact geometry fields, nudges, add/duplicate/delete, snapping toggle, undo/redo buttons and shortcuts.
  4. Polish: README section, CLAUDE.md, quickstart log.
- **MVP check** after step 2: quickstart §2 items 1, 2, 4, 5, 6, 7.
- **Device removed while editing** (edge case): the editor offers a file dialog to save elsewhere, then closes.
- **Changed on disk**: compared by modification time and size recorded at open (R7).

## Complexity Tracking

None. No principle needs an exception.
