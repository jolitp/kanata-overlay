# Interfaces: Layout Editor

## `LayoutEditor` QObject (new, `crates/app/src/layout_editor.rs`)

Exposed to QML as `LayoutEditor` (camelCase names via `#[auto_cxx_name]`).

**Properties**

| Name | Type | Meaning |
|------|------|---------|
| `deviceId` | string | device being edited, `""` when closed |
| `layoutJson` | string | the layout being edited, same shape as `DeviceState.layoutJson` (keys carry `kanataCanonical`) |
| `flagsJson` | string | `[{unbound, duplicate, notInConfig, overlapping}]`, one per key, plus `counts` |
| `selectionJson` | string | `[index, …]` |
| `canUndo`, `canRedo`, `dirty` | bool | |
| `snap` | bool | read/write |
| `snapUnit`, `snapAngle` | real | 0.25, 5 when snapping, else 0 |
| `error` | string | last save or bind error, `""` if none |

**Invokables**

| Call | Effect |
|------|--------|
| `open(deviceJson, layoutJson, configKeysJson)` | start a session from the device's current board (`DeviceState.layoutJson`) and its `defsrc` names |
| `select(indicesJson, mode)` | mode: `"replace"`, `"add"`, `"toggle"`; `selectAll()`, `selectNone()` |
| `selectRect(x, y, w, h, mode)` | keys whose centre lies in the rectangle (units) |
| `moveSelection(dx, dy)`, `rotateSelection(d)`, `resizeSelection(edge, delta)` | edge: `"left"`, `"right"`, `"top"`, `"bottom"` |
| `setGeometry(field, value)` | field: `x`, `y`, `width`, `height`, `rotation` |
| `addKey(x, y)`, `duplicateSelection()`, `deleteSelection()` | |
| `bind(name)` | `""` clears; returns false and sets `error` if not a key name |
| `setLegend(text)` | |
| `keyNames(filter) -> string` | picker entries as JSON `[{name, canonical, aliases, group}]`; group: `"unbound"`, `"config"`, `"other"` (research R8) |
| `undo()`, `redo()` | |
| `saveTarget() -> string` | the path Save would write, for the window title and messages |
| `save() -> string` | writes; returns the saved path, or `""` and sets `error`; `"changed"` in `error` means the file changed on disk (then `save(true)` overwrites, `reload()` rereads) |
| `close()` | ends the session (the caller has already asked about unsaved changes) |

**Signals**

| Signal | When |
|--------|------|
| `saved(deviceId, path, isNew)` | after a successful save; QML then sets the device's layout setting if `isNew` and emits it to every `DeviceView` |

## `DeviceState` additions

| Call | Effect |
|------|--------|
| `previewLayout(json)` | show this layout (labels recomputed), no file access |
| `endPreview()` | reload the device's saved layout (or grid) |
| `reloadLayout(path)` | if `path` is this device's layout file, reread it (shared files, research R7) |
| `configKeysJson()` | the canonical `defsrc` names of the loaded config, `[]` without one |

`apply()` changes: a change of layout path alone reloads the layout without restarting the session (research R6).

## `SettingsWindow.qml`

- New **Edit layout…** button beside the layout field's **Browse…**; opens the editor for the selected device.

## Files

- New layouts are written to `$XDG_CONFIG_HOME/kanata-overlay/layouts/<slug of the device name, or its id>.json` (fallback `~/.config/...`), format version 1, keys normalised to start at (0, 0), written atomically.
- The Kanata config is never written (FR-018).

## `kbd` additions

| Item | Meaning |
|------|---------|
| `key_names() -> impl Iterator<Item = (&str, &str)>` | every name Kanata accepts with its canonical key |
| `is_key_name(name) -> bool` | the name is in that table |
