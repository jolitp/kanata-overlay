# Data Model: Layout Editor

## Layout and Key (existing, `layout` crate)

Unchanged on disk (format version 1). See `crates/layout/src/lib.rs`.

| Field | Meaning | Rules added by this feature |
|-------|---------|-----------------------------|
| `x`, `y` | top-left before rotation, in key units (1u) | editor snaps to 0.25u unless snapping is off; saved layouts start at (0, 0) |
| `width`, `height` | size in units | ≥ 0.25u, ≤ 20u |
| `rotation` | degrees clockwise around (`rx`, `ry`) | kept in (−180, 180]; editor snaps to 5° |
| `rx`, `ry` | rotation centre, absolute units | keys the editor touches get their own centre here (research R3) |
| `kanata` | Kanata key name, or absent (unbound) | must pass `kbd::is_key_name` |
| `legend` | printed text, may be empty | free text; longer input is cut to 32 characters |

### Background (new, optional on Layout)

The editor's reference image: `path` (absolute), `x`, `y` (top-left, units), `width`
(units; height from the image's aspect ratio), `opacity`. Rules: width 0.5–200,
opacity 0.05–1; omitted from the file when absent (older files load unchanged);
shifted with the keys by `normalise_origin`; boards ignore it (FR-020).

## Editor (new, `layout::edit`)

One editing session for one device's layout.

| Field | Meaning |
|-------|---------|
| `layout` | the layout being edited |
| `selection` | ordered set of key indices; always valid indices |
| `undo`, `redo` | stacks of layout snapshots (selection included), undo capped at 200 |
| `saved` | snapshot at the last save or at open; `dirty` = `layout != saved` |
| `snap` | on/off; steps 0.25u and 5° when on |

Operations (each one that changes the layout records exactly one undo step):

| Operation | Effect |
|-----------|--------|
| `select(indices, mode)` | replace, add or toggle; `select_all`, `select_none`. No undo step. |
| `move_by(dx, dy)` | selected keys move together |
| `resize(edge, delta)` | single selected key; that edge moves in the key's own frame |
| `set_geometry(x?, y?, w?, h?, rotation?)` | exact values from the inspector; for several keys, `x`/`y` move the selection's bounding box |
| `rotate_by(d)` | around the selection centre |
| `add_key(at)` | new 1u unbound key, empty legend, selected alone |
| `duplicate()` | copies of the selection, offset 0.5u right and down, selected |
| `delete()` | removes the selection; selection becomes empty |
| `bind(name?)` | sets or clears `kanata` on the single selected key; rejected if not a key name |
| `set_legend(text)` | single selected key |
| `undo()`, `redo()` | swap snapshots |
| `mark_saved()` | `saved = layout` |

## Diagnostics (new, pure)

`diagnose(layout, config_keys) -> Vec<KeyFlags>` plus counts, where `config_keys` is the
set of canonical names in the config's `defsrc` (empty when there is no config).

| Flag | When |
|------|------|
| `unbound` | no `kanata` |
| `duplicate` | another key has the same canonical name |
| `not_in_config` | bound, a config exists, and its canonical name isn't in `defsrc` |
| `overlapping` | its shrunk rotated rectangle intersects another key's (research R9) |

## Save target (new, pure)

`save_target(current_path, current_is_own_format, layouts_dir, file_stem, exists) -> Target`, where `file_stem` is `slug(device name)`, or the device id if that slug is empty

| Current layout | Target |
|----------------|--------|
| own-format file | `InPlace(current_path)` |
| none, grid, KLE export, unreadable | `New(layouts_dir/<file_stem>.json)`, or `-2`, `-3`… if it exists |

## State transitions of an editing session

```text
closed ──open(device)──▶ clean ──edit──▶ dirty ──save ok──▶ clean
                           ▲                │  └─save fails─▶ dirty (+ error shown)
                           └──undo to saved─┘
dirty ──close / switch device──▶ ask: Save → (save ok) closed | Discard → closed (board reloads saved) | Cancel → dirty
```
