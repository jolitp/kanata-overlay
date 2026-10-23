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

*(filled in during implementation)*
