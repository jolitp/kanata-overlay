# Implementation Plan: Several Devices

**Branch**: `004-multi-device` (stacked on `003-managed-kanata`) | **Date**: 2026-10-23 | **Spec**: [spec.md](spec.md)

## Summary

The user describes devices in the settings window: a name, a Kanata config, a layout,
and optionally an address. The overlay creates a `DeviceState` for each one. Each
`DeviceState` owns that device's Kanata session (managed or attached, with feature 003's
supervisor), its connection, layer and labels, and its own overlay window placed by its
own settings.

Before starting managed Kanatas, the overlay works out which input devices each config
would grab. It reads `/proc/bus/input/devices` and the configs' `linux-dev*` options, and
warns about overlaps or configs without a device limit. Mouse buttons get glyphs. A device
without a layout gets a grid. Existing single-device use and v1 settings files keep
working.

## Technical Context

**Language/Version**: Rust stable (edition 2024); QML (Qt 6.11)

**Primary Dependencies**: none new. `QtQuick.Dialogs` comes with qtdeclarative, which the dev shell already has. `kanata-client` gains a dependency on the workspace's own `kbd` crate.

**Storage**: settings file v2, with v1 read compatibly

**Testing**:
- `cargo test`: settings v2 and migration; kbd device selection and glyphs; `/proc` parsing with a fixture of the user's devices; `grabs` and overlap.
- Manual: quickstart §2 with two fake servers, §3 with real Kanata.

**Target Platform**: Linux (Plasma Wayland first)

**Performance Goals**:
- both boards up within 6 s (SC-001);
- relabel per device under 100 ms (SC-002).

**Constraints**:
- per-device isolation (FR-005);
- never strand the keyboard, per device;
- `just run` unchanged.

**Scale/Scope**:
- settings: about 250 lines plus tests;
- kbd: about 80 lines;
- kanata-client: about 200 lines;
- app: 1 new QObject, made by moving code out of `OverlayState` and `KanataState`;
- QML: 1 new file and 4 changed.

## Constitution Check

| Principle | Status | How |
|-----------|--------|-----|
| I. Rust owns state, QML renders | ✅ | Devices, migration, selection and overlap are in the Rust crates. `DeviceState` bridges per device. QML only instantiates one view per id, and creating and destroying views is Qt's job. |
| II. Stable Kanata interfaces | ✅ | The TCP protocol per device, as before. `linux-dev*` are documented `defcfg` options read by our subset parser. `/proc/bus/input/devices` is a stable kernel interface. |
| III. Never strand the keyboard | ✅ | One `Supervisor` per device, each on its own thread with the parent-death signal. Removing a device drops its supervisor, which stops its Kanata. The overlap check prevents two Kanatas grabbing one device, and a conflicting second Kanata is never started. |
| IV. Plasma Wayland first | ✅ | One layer-shell window per board, the pattern feature 001 already uses. |
| V. Testable without hardware | ✅ | Selection and overlap are pure, with a `/proc` fixture. Multi-device behaviour is tested with two fake servers (quickstart §2). Real-Kanata checks have manual notes. |
| VI. Key events are private | ✅ | Nothing new is logged. Device names appear in warnings; they aren't key data. |
| VII. Simple first | ✅ | Code moves, nothing is rewritten. No new dependencies. Device-specific behaviour stays in the user's Kanata configs, and the overlay only reads them. |

**Gate**: PASS. **Post-design**: PASS.

## Project Structure

```text
crates/settings/src/
├── lib.rs           # Device, Settings v2, add/remove/device_mut, default_address
├── file.rs          # v2 load/save; v1 → one device; per-device leniency
└── devices.rs       # NEW: effective_devices (implicit and migrated), id rules
crates/kbd/src/
├── reader.rs        # + defcfg linux-dev / -names-include / -names-exclude
├── lib.rs           # + DeviceSelection on Config
└── label.rs         # + BTN_* glyphs
crates/layout/src/lib.rs        # + Layout::grid(names, columns)
crates/kanata-client/src/
├── input_devices.rs # NEW: parse /proc/bus/input/devices, grabs(), overlap
└── tests/fixtures/proc-input-devices.txt   # NEW: the user's devices (names only)
crates/app/
├── src/device_state.rs        # NEW: per-device session, connection, labels (moved code)
├── src/overlay_state.rs       # global only: platform, notices, visibility
├── src/kanata_state.rs        # REMOVED (moved into device_state.rs)
├── src/settings_state.rs      # + devices API, revision, per-device placement
└── qml/
    ├── Main.qml               # Instantiator over device ids
    ├── DeviceView.qml         # NEW
    ├── LayerShellOverlay.qml, FallbackOverlay.qml, Board.qml   # take a DeviceState
    └── SettingsWindow.qml     # device selector, fields, Browse…, per-device rows
```

## Implementation Notes

- **Order**:
  1. Foundational: settings v2 and migration, `DeviceState` extracted with one device and no behaviour change, then `just check` and a manual single-device check.
  2. US1: the Instantiator, per-device windows and the device editor.
  3. US2: mouse glyphs and the grid.
  4. US3: selection, `/proc` and overlap.
- **The extraction step is a pure refactor**: feature 003's quickstart §3 must still pass afterwards, before any multi-device work starts.
- **Global notices** (platform, shortcuts) show on the first board.

## Complexity Tracking

None. No principle needs an exception.
