# Data Model: Live Key Highlighting

All key data here lives in memory only (constitution VI, FR-009). Nothing in this file
is written to disk except the per-device **highlight** setting.

## Highlight setting (settings file, per device)

| Field | Values | Rules |
|-------|--------|-------|
| `highlight` | `"all"`, `"no-typing"`, `"off"` | missing or invalid → `"all"` (with a settings warning for invalid); settings file stays version 2 |

Switching between `"off"` and either other value restarts that device's managed Kanata
(with or without `--debug`); `"all"` ↔ `"no-typing"` does not (research R5).

## KeyEvent (new, `kanata_client::output_tap`)

| Field | Meaning |
|-------|---------|
| `key` | canonical key name from Kanata's `OsCode` (`KEY_CAPSLOCK`, `BTN_SIDE`) |
| `action` | `Press`, `Release` or `Repeat` |

Parsed from a debug line; never stored, logged or kept beyond the filter (research R3).

## Live state (new, `DeviceState`, per device)

| Field | Meaning | Rules |
|-------|---------|-------|
| `held` | set of canonical keys currently held | added on Press, removed on Release; emptied on disconnect, restart, stop, `watching = false` and on any highlight mode change (a typing key held across a switch to "no typing keys" would otherwise never see its release) |
| `tap_hold` | map canonical key → (`Tap` \| `Hold`, since) | Tap expires after 1 s; Hold on that key's Release, or after 1.5 s without a debug source; emptied with `held` |
| `watching` | bool | set from QML: overlay shown or the editor open on this device |
| `mode` | `All` \| `NoTyping` \| `Off` | from the highlight setting |
| `armed` | bool | press-to-bind waiting; disarmed by the first Press, `cancelCapture()` or a stop |
| `source` | `Debug` \| `TcpOnly` | `Debug` when the overlay started this Kanata with `--debug` |

## Filter (pure, research R3)

`route(event, armed, watching, mode) -> Route`:

| Condition (first match) | Route |
|-------------------------|-------|
| `armed` and `Press` | `Capture(key)` |
| `Repeat` | `Drop` |
| not `watching` | `Drop` |
| `mode == Off` | `Drop` |
| `mode == NoTyping` and `is_typing_key(key)` | `Drop` |
| otherwise | `Show(event)` |

Tap-hold results use the same table with `armed = false` and `action = Press`.
