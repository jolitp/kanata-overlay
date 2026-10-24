# Quickstart: Layout Editor

How to check the feature works. §1 needs nothing; §2 uses the fake Kanata (agents can
run it); §3 is for the user, with their devices and real Kanata.

## 1. Automated

```sh
just check
```

Covers (see [data-model.md](data-model.md)): every editor operation and its undo/redo;
rotation normalisation keeps every corner in place; rotate-then-move composes; resize in
a rotated key's frame; snapping; diagnostics (unbound, duplicate, not in config,
overlapping vs touching); save target rules; atomic save and the "changed on disk"
check; `kbd::is_key_name` and `key_names`.

## 2. With the fake Kanata

Use a separate settings file so the real one isn't touched:

```sh
SP=$(mktemp -d)
cp examples/kanata/minimal.kbd "$SP/a.kbd"
# settings.json with one device "a": config $SP/a.kbd, no layout
cargo run -p kanata-overlay -- --settings "$SP/settings.json" \
  --kanata crates/kanata-client/tests/fake-kanata.sh
```

1. **Open**: settings → **Edit layout…**. The editor shows the grid of `a.kbd`'s keys, all bound; the count of unbound keys is 0.
2. **Bind (US1)**: add a key → it is unbound and marked; bind it to `caps` from the list (the config's keys come first) → marked duplicate (Caps is already on the grid). Bind it to `f13` → marked "not in config". Type `nonsense` → refused.
3. **Arrange (US2)**: drag a key, rotate one by 15° with the handle, widen one to 1.5u, select a row with a rectangle and move it, delete a key; after each release the board on the overlay updates within half a second (SC-003). Undo all, redo all.
4. **Save (US3)**: Ctrl+S → `layouts/a.json` appears in `$XDG_CONFIG_HOME/kanata-overlay/`, the settings window's layout field shows it, and the fake Kanata was **not** restarted (its board stays "managed · connected" without a gap).
5. **Persistence (SC-004)**: quit, start again: the board is identical to the saved one; reopen the editor: no unsaved changes.
6. **Discard (SC-006)**: move a key, close the editor, choose **Discard** → the board returns to the saved layout.
7. **KLE source**: point the device at `examples/layouts/sample.kle.json`, edit, save → a new `layouts/a-2.json` (or `a.json` if free); the KLE file is unchanged (`git diff --exit-code examples/`).
8. **Failed save**: `chmod a-w` the layouts folder, edit, save → the reason is shown, the edits stay, the board keeps the previous layout. `chmod u+w` afterwards.

## 3. With the user's devices (user)

1. **Defy (SC-002)**: open the editor on the Defy's grid and shape it like the Defy: two halves, the thumb clusters rotated. Time it. Save, restart the overlay, compare with the keyboard.
2. **Mouse**: open the editor on `mouse-layout.json` (own format) and save → that file is updated in place.
3. **Bind (SC-001)**: import a KLE layout with unbound keys (or delete bindings in the editor), bind ten keys from the list, save; time it.
4. **No interruption**: while Kanata runs the Defy, save a new layout: typing on the Defy never stops.
5. **Large layout (SC-005)**: open a full-size (~110-key) KLE layout, select all, drag: keys follow the pointer smoothly.

## Validation log

**Automated (2026-10-23)**: `just check` passes (fmt, clippy `-D warnings`, all
tests). New tests: `layout::edit` (16: rotation normalisation and composition,
undo/redo and its cap, selection modes, snapped move and rotate, resize in a rotated
frame, exact geometry, add/duplicate/delete, bind and legend, diagnostics including
touching vs overlapping and a rotated thumb key, origin normalisation, save targets,
atomic write and stamps, saved = edited), `kbd` key names, `settings::layouts_dir`,
the key-name picker.

**Agent, fake Kanata (2026-10-23)**: driven by a temporary script in `Main.qml`
(removed afterwards), with a separate `--settings` file and screenshots:
- §2.1–2.2: the editor opened on the grid of `minimal.kbd`; `nonsense` refused with a
  message; an added key bound to `caps` → "2×" on both Caps keys; `f13` → "∉"; the
  rotated new key unbound; counts 1 unbound · 2 duplicate · 1 not in config ·
  2 overlapping (a widened Q over W). The board previewed every change.
- §2.3: a row selected by box (12 keys), moved, undone, redone. Transient states
  rendered by setting them from the script: a 20°-rotated key widening along its own
  axis, two keys rotating around their shared centre with the group handle. Real
  pointer drags (and SC-005 smoothness) are left to the user (T032).
- §2.4: Save wrote `layouts/alpha.json` (named after the device, next to the
  settings file), the device's layout setting pointed to it, the title lost its "•",
  the board dropped "showing a grid", and the fake Kanata was **not** restarted (no
  state change logged).
- §2.5: after a restart the editor reopened clean with the same 41 keys.
- Changed on disk: appending to the file made Save answer "changed"; Overwrite saved.
- §2.8: with the layouts folder read-only, Save reported "Permission denied" and kept
  the previous layout.
- §2.6: discard (end preview) put the saved layout back on the board.
- §2.7: with `examples/layouts/sample.kle.json` as the layout, Save created
  `alpha-2.json` and switched the device to it; `git diff --exit-code examples/` clean.

**Fixed on the way**: the editor's properties are empty strings before the first
`open()`, so QML now guards the JSON it parses; the picker showed `↩` for Enter and
missed matches past the first six spellings, so it now prefers plain-text spellings
and matches every spelling.

**Design notes**: the layouts folder is next to the settings file in use (so
`--settings` runs never touch the real one); `DeviceState.configKeysJson()` returns
the config's names as written (the picker shows your spelling); moving every key and
saving is not a change, because saving shifts the layout back to the origin.

**Added on request (2026-10-23)**: wheel zoom around the pointer (checked by script:
zooming 54 → 108 kept the layout point under the pointer at the same place, the
slider followed) and right/middle-drag panning; a reference image under the keys
(checked by script and screenshot: chosen, placed at 60 % opacity, saved in the layout
file, undoable; the board ignores it). Real wheel and drag input are left to the user.

**User, real devices (2026-10-23), T032**:
- §3.1 Defy: the user shaped the grid into the Defy with a Bazecor screenshot as the
  reference image: 70 keys, 12 rotated thumb keys, saved as
  `~/.config/kanata-overlay/layouts/defy.json` and selected. Took about 30 minutes, so
  **SC-002 (under 20 minutes) is missed** on this first use; the reference image
  arrived midway. Candidates if it matters: mirror a half, set one angle for several
  keys, faster binding.
- §3.2 Mouse: saving updated `~/.config/kanata/mouse-layout.json` in place (the edit
  was in the file, no new file). Passed.
- §3.4 Typing on the Defy while saving: no interruption noticed. Passed.
- §3.5 Dragging keys followed the pointer; wheel zoom and right-drag pan worked.
  Passed. Moving the reference image "changed direction by itself": the canvas was
  sized from the image's position *during* the drag and grew under the pointer.
  Fixed: the canvas uses the image's committed position, and when it grows left or up
  the view scrolls by as much (checked by script: no origin change during a drag, the
  point under the pointer unchanged after the commit).
- §3.3 binding timing (SC-001) was not timed separately; the Defy needed binding only
  for 4 keys outside `defy.kbd`.

Untested: removing a device while its editor has unsaved changes (T023, opens a file
dialog).

