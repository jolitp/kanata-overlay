# Data Model: Several Devices

**Feature**: [spec.md](spec.md) | **Research**: [research.md](research.md)

## Device *(settings crate)*

| Field | Type | Rule |
|-------|------|------|
| `id` | string | Unique, `[a-z0-9-]{1,32}`. Generated from the name on Add. Duplicates get `-2`, `-3`, … |
| `name` | string | 1–40 characters, shown on the board header and in settings |
| `config` | path or null | The Kanata `.kbd` file. Without it there is no managed Kanata and no labels |
| `layout` | path or null | KLE or own-format JSON. Missing → a grid built from `defsrc` (R7) |
| `address` | `host:port` or null | Null → `127.0.0.1:7070+i` (R4) |
| `edge`, `margin`, `opacity`, `scale`, `screen` | as in feature 001 | Same ranges: margin 0–500, opacity 10–100, scale 50–300, edge one of the 8 positions |

## Settings, version 2

```json
{
  "version": 2,
  "devices": [
    { "id": "defy", "name": "Defy", "config": "~/kanata/defy.kbd", "layout": "~/kanata/defy.json",
      "address": null, "edge": "bottom", "margin": 16, "opacity": 85, "scale": 100, "screen": null },
    { "id": "mouse", "name": "Mouse keypad", "config": "~/kanata/mouse.kbd", "layout": "~/kanata/mouse.json",
      "address": null, "edge": "bottom-right", "margin": 16, "opacity": 85, "scale": 80, "screen": null }
  ]
}
```

| Input | Result |
|-------|--------|
| v2 file | devices as written. Invalid devices are skipped with a warning. Invalid fields fall back to their defaults, per field |
| v1 file | one device, `{id: "main", name: "Keyboard"}`, with the v1 placement. Config and layout come from the CLI. Saved as v2 on the next edit |
| `version` > 2 | defaults, a warning, and the file is not writable (as in feature 001) |
| no devices, `--config` given | one implicit device, `main`, built from `--config`/`--layout` and the CLI placement overrides. Not saved until the user edits devices |
| no devices, no `--config` | one implicit device without a config: today's legends-only board |

## DeviceSelection *(kbd crate)*

| Field | From `defcfg` |
|-------|---------------|
| `paths` | `linux-dev` (`a:b` or a list; `\:` escapes a colon) |
| `include` | `linux-dev-names-include` (list) |
| `exclude` | `linux-dev-names-exclude` (list) |
| `mouse_buttons` | `defsrc` contains any `BTN_*` key |

## InputDevice *(kanata-client, from `/proc/bus/input/devices`)*

| Field | Meaning |
|-------|---------|
| `name` | The quoted `N: Name=` value |
| `events` | `eventN` handlers (`H: Handlers=`) |
| `keyboard` | `EV=` has KEY, and the `KEY=` bitmap contains `KEY_A`…`KEY_Z` |
| `mouse` | `EV=` has REL, and `KEY=` contains `BTN_LEFT` |

## Grab resolution and overlap

```
grabs(sel, present) → set of devices (eventN + name); several devices may share a name
    sel.paths non-empty   → present devices whose eventN (or /dev/input path) is in paths
    sel.include non-empty → every present device whose name is in sel.include
    otherwise (auto)      → {d | d.keyboard || (sel.mouse_buttons && d.mouse)}, minus devices whose name is in sel.exclude

conflict(a, b)  = grabs(a) ∩ grabs(b) ≠ ∅ (by eventN)   → warning naming the shared devices, only a (the first) is started
unlimited(sel)  = sel.paths and sel.include both empty  → warning "grabs every keyboard: …names…"
```

## DeviceState *(app)*

Per device; it replaces the per-device half of `OverlayState` and all of `KanataState`.

| Property | From |
|----------|------|
| `deviceId`, `name` | settings |
| `connected`, `currentLayer`, `status` | its connection loop (was in `OverlayState`) |
| `layoutJson`, `labelsJson` | its layout and config (feature 002) |
| `mode`, `stateText`, `logText`, `canRestart`, `canStart` | its Kanata session (was `KanataState`) |
| `notices` | device notices (config errors, overlap warnings) |

| Lifetime | |
|----------|---|
| created | by `DeviceView.qml` when the id appears in `settingsState.deviceIds` |
| started | `start()`: load settings, layout and config, probe, then attach or manage |
| destroyed | when the id disappears or the app quits; `Drop` stops a managed Kanata |
