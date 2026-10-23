# Feature Specification: Layout Editor

**Feature Branch**: `005-layout-editor`

**Created**: 2026-10-23

**Status**: Draft

**Input**: User description: "Layout editor: edit a device's physical layout in the overlay app — bind unbound keys to Kanata key names, move/resize/rotate keys, add and delete keys, and save the layout in the app's own format, so a device without a usable layout (e.g. the Defy grid or the mouse keypad) can get a board that matches the real device"

## Context

Each device's board is drawn from a *layout*: where each physical key sits, how big
it is, how it is rotated, and which Kanata key name it stands for. Today a layout
comes from a file made elsewhere (a Keyboard Layout Editor export or the app's own
format, written by hand). A device without one gets a plain grid of its config's
keys (feature 004), and an imported layout can have keys whose name could not be
guessed: those show no action at all.

The user's Dygma Defy is a split keyboard with thumb clusters, shown today as a
12-wide grid; the Compx mouse keypad got a hand-written layout. This feature lets
the user shape a board to match the real device inside the app.

## Clarifications

### Session 2026-10-23

- Q: How does the user choose which Kanata key a physical key is? → A: From a list now (the config's keys first, every Kanata key name searchable); "press the key to bind it" comes with live key highlighting (roadmap item 6), not in this feature.
- Q: How are keys moved, resized and rotated? → A: Drag on a canvas (move; handles to resize and rotate), plus exact fields for position, size and rotation of the selection.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Bind keys the layout doesn't know (Priority: P1)

The user opens the editor for a device whose board has unbound keys (keys that show
no action because the layout doesn't say which Kanata key they are). Unbound keys
are marked. The user selects one and chooses its Kanata key; the board shows that
key's action from then on. They save, and the binding is still there after a
restart.

**Why this priority**: an unbound key is a key the board can't explain at all. This
is the smallest change that makes an imported layout fully useful, and it needs no
geometry editing.

**Independent Test**: open an imported layout with unbound keys, bind one, save,
restart the overlay: the key shows its action.

**Acceptance Scenarios**:

1. **Given** a device whose layout has unbound keys, **When** the user opens the editor, **Then** every unbound key is visibly marked and a count of unbound keys is shown.
2. **Given** a selected unbound key, **When** the user binds it to a key name, **Then** the key shows the action that key has in the device's current layer, in the editor and on the board.
3. **Given** the user binds a key to a name another key already uses, **When** the binding is made, **Then** both keys are marked as duplicates (allowed, e.g. two physical Esc keys) and nothing is silently unbound.
4. **Given** a bound key, **When** the user clears its binding, **Then** it becomes unbound again.

---

### User Story 2 - Arrange keys to match the device (Priority: P2)

The user opens the editor on the Defy's grid and rearranges it into the Defy's shape:
two halves, thumb keys rotated, wider keys where the device has them. They can add
keys the config doesn't list and delete keys the device doesn't have.

**Why this priority**: this is what turns "a grid of names" into "a picture of my
keyboard", the main point of the board for split and odd devices. It builds on
binding (US1): new keys need a binding.

**Independent Test**: starting from the grid of a config, move two keys, rotate one,
widen one, add one and delete one, save: the board matches what the editor showed.

**Acceptance Scenarios**:

1. **Given** a device with no layout (grid board), **When** the user opens the editor, **Then** the editor starts from that grid with every key already bound.
2. **Given** selected keys, **When** the user moves them, **Then** they move together and keep their relative positions.
3. **Given** a selected key, **When** the user changes its width, height or rotation, **Then** the key changes accordingly and its legend stays readable.
4. **Given** the editor, **When** the user adds a key, **Then** a new unbound key appears, selected, ready to bind.
5. **Given** selected keys, **When** the user deletes them, **Then** they disappear; the config is not changed.
6. **Given** any change, **When** the user undoes it, **Then** the layout returns to its state before that change; redo reapplies it.

---

### User Story 3 - Save and use the layout (Priority: P1)

When the user saves, the layout is written in the app's own format and the device
uses it from then on. A device that had no layout gets a new file; a device whose
layout came from another tool's export gets a new file in the app's format, and the
original is left untouched.

**Why this priority**: without saving, the other stories produce nothing that lasts.
It ships with US1.

**Independent Test**: edit the mouse keypad's layout, save, restart the overlay: the
board is the saved one, and the settings window shows the device's layout is the
saved file.

**Acceptance Scenarios**:

1. **Given** a device with no layout, **When** the user saves, **Then** a layout file is created in the app's layouts folder, named after the device, and the device's layout setting points to it.
2. **Given** a device whose layout is in the app's own format, **When** the user saves, **Then** that file is updated in place.
3. **Given** a device whose layout is another tool's export, **When** the user saves, **Then** a new file in the app's format is created and selected; the export is not modified.
4. **Given** unsaved changes, **When** the user closes the editor or switches device, **Then** they are asked to save, discard or cancel.
5. **Given** the save fails (folder not writable, disk full), **Then** the user sees why, the edits stay in the editor, and the device keeps its previous layout.

---

### Edge Cases

- **Binding to a name the config doesn't list**: allowed (the device may send it, or the config may add it later); the key is marked "not in config" and shows its printed legend instead of an action.
- **Config changes after the layout is made**: keys whose name left `defsrc` show "not in config"; nothing is unbound or deleted automatically.
- **Overlapping keys**: allowed (they can't be prevented while dragging), but marked, so the user can notice them.
- **Very many keys** (full-size keyboards, ~110 keys): editing stays responsive.
- **Empty layout** (every key deleted): saving is allowed; the board shows an empty frame with the device name, never a crash.
- **Device removed while its editor is open**: the editor closes after asking to save the layout to a file of the user's choice.
- **Two devices sharing one layout file**: saving updates both boards.
- **Layout file changed outside the app while the editor is open**: on save the user is told it changed and chooses to overwrite or reload.
- **Overlay hidden or not shown on the current monitor**: the editor works on its own; the board just doesn't preview.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: Users MUST be able to open the layout editor for the selected device from the settings window.
- **FR-002**: The editor MUST open in an ordinary window that takes mouse and keyboard input (the overlay itself stays click-through).
- **FR-003**: The editor MUST start from the device's current board: its layout file if usable, otherwise the grid of its config's keys, otherwise an empty layout.
- **FR-004**: The editor MUST show each key with its printed legend, its bound Kanata key name and, if bound and in the config, its action in the device's current layer.
- **FR-005**: The editor MUST mark unbound keys, duplicate bindings, keys bound to names not in the config, and overlapping keys, and show how many of each there are.
- **FR-006**: Users MUST be able to bind a key to a Kanata key name, change it and clear it, by picking from a list: the config's keys first (those not yet bound on top), then every Kanata key name, searchable by typing.
- **FR-007**: Only names Kanata accepts as key names MUST be accepted as bindings; an unknown name is refused with a message.
- **FR-008**: Users MUST be able to select one key, several keys, or all keys.
- **FR-009**: Users MUST be able to move the selected keys by dragging, resize and rotate them with handles, and set position, size and rotation exactly in fields; arrow keys nudge the selection. Positions and sizes snap to quarter key units and rotation to 5° by default, and snapping can be turned off.
- **FR-010**: Users MUST be able to add keys and delete selected keys, and duplicate selected keys.
- **FR-011**: Users MUST be able to edit a key's printed legend.
- **FR-012**: Users MUST be able to undo and redo every change made in the editor since it opened.
- **FR-013**: The device's board on the overlay MUST show the edited layout while editing (live preview), and return to the saved layout if the edits are discarded.
- **FR-014**: Saving MUST write the layout in the app's own format: a device with no layout or with another tool's export gets a new file in the app's layouts folder named after the device (never overwriting an existing file); a device whose layout is already in the app's format updates that file.
- **FR-015**: After saving to a new file, the device's layout setting MUST point to it, without restarting that device's Kanata.
- **FR-016**: Users MUST be asked to save, discard or cancel when closing the editor or switching device with unsaved changes.
- **FR-017**: A failed save MUST be reported with its reason, keep the edits in the editor, and leave the device's previous layout in use.
- **FR-018**: The editor MUST never change the device's Kanata config.
- **FR-019**: The editor MUST NOT read pressed keys (constitution VI; press-to-bind comes with roadmap item 6).

### Key Entities

- **Layout**: a named set of keys for one physical device, in the app's own format (already exists, feature 001). Gains nothing new except being written by the app.
- **Key**: position, size, rotation (angle and centre), Kanata key name (may be absent: unbound), printed legend.
- **Editing session**: the layout being edited for one device, its selection, its undo history and whether it has unsaved changes. Lives only while the editor is open.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Starting from an imported layout with 10 unbound keys, a user binds all of them and saves in under 3 minutes.
- **SC-002**: Starting from the Defy's grid, a user produces a board the user judges as "matches my keyboard" (two halves, thumb clusters, rotations) in under 20 minutes, without editing any file by hand.
- **SC-003**: Every edit shows on the overlay's board within half a second.
- **SC-004**: A saved layout, reopened after restarting the overlay, is identical to what was saved: same keys, positions, sizes, rotations, bindings and legends.
- **SC-005**: Editing a 110-key layout stays smooth: dragging keys follows the pointer without visible lag.
- **SC-006**: No editing operation leaves a device without a working board: after a discarded edit, a failed save or a crash of the editor, the board shows the last saved layout.

## Assumptions

- The app's own layout format (feature 001) already holds everything needed (position, size, rotation with centre, binding, legend); no format version change is expected. If one is needed, older files keep loading.
- The app's layouts folder is `layouts/` inside the overlay's config folder (next to `settings.json`).
- Keyboard Layout Editor import stays as it is; exporting back to that format is out of scope.
- Editing the Kanata config (adding keys to `defsrc`, changing actions) is out of scope; the editor only reads it.
- Non-rectangular keys (ISO Enter, big-ass Enter) are drawn as rectangles, as today.
- One editor window at a time, for one device; switching device is done inside it.
- Binding by pressing the key on the device is out of scope; it comes with live key highlighting (roadmap item 6), which will read key presses anyway. This feature reads no key presses at all.
