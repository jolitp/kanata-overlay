# Research: Several Devices

**Feature**: [spec.md](spec.md) | **Plan**: [plan.md](plan.md) | **Date**: 2026-10-23

Kanata facts come from the 1.12.0 docs (`docs/config.adoc`, sections linux-dev,
linux-dev-names-include/exclude and linux-device-detect-mode) and from the user's
hardware, probed with `evtest`.

## R1. One app object per device

- **Decision**: Split today's `OverlayState` and `KanataState` along the global/per-device line.
  - **Global (`OverlayState`)**: platform capabilities, global notices, show/hide plus preview, and the shortcuts.
  - **Per device (new `DeviceState` QObject)**: the device's Kanata session (managed or attached, feature 003), its TCP connection, current layer, layout JSON, parsed `.kbd` config, labels (feature 002) and device notices. The code moves from `overlay_state.rs` and `kanata_state.rs`, it isn't rewritten. Each `DeviceState` owns its own address, so the `ADDRESS` static goes away.
- **Rationale**: Every device-specific thing already exists for one device. Moving it under one object per device makes N devices mostly a matter of instantiation (constitution VII). Each `DeviceState` owns one `Supervisor` on its own thread, so the parent-death guarantee holds per device (constitution III).
- **Alternatives considered**: One QObject holding vectors of everything. That would mean index-based QML access, a custom list model in cxx-qt, and more code.

## R2. Creating and removing devices live (FR-001a)

- **Decision**: `Main.qml` uses an `Instantiator` whose model is `settingsState.deviceIds`, a JSON list of ids. Its delegate is a new `DeviceView.qml`: a `QtObject` that owns one `DeviceState` and creates that device's overlay window with `createObject`. That is the same pattern as today, so the window stays top-level.
  - Adding an id creates a view, whose `Component.onCompleted` starts the session.
  - Removing an id destroys the view. The Rust `DeviceState` is dropped, its `Supervisor` is dropped, and a managed Kanata stops (feature 003's `Drop`).
- **Rationale**: Qt manages the lifetimes; the Rust side needs no registry.

## R3. Settings file version 2

- **Decision**: The `settings` crate gains a `devices` list. Each entry holds the id, the name, the Kanata config path, the layout path, an optional fixed address, and its own placement (edge, margin, opacity, scale, screen). The file version becomes `2`.
  - **v1 files**: read as one device, `"main"`, using the v1 placement fields. Its config and layout come from `--config`/`--layout`, because v1 didn't store them (FR-002, FR-008a).
  - **No devices but `--config` given** (today's usage): one implicit device, not written to the file until the user edits devices.
  - **CLI placement overrides** (`--edge` etc.): apply only in that single-device case.
  - **Leniency**: each device is validated field by field, as in feature 001. An invalid device is skipped with a warning. Duplicate ids get a suffix.
- **Rationale**: Settings stay in one file with one save path, and existing users and commands keep working.

## R4. Ports for managed Kanatas (FR-006)

- **Decision**:
  - **Default address**: device *i* (0-based, in list order) uses `127.0.0.1:7070+i` unless its `address` is set.
  - **Startup**: feature 003's logic runs per device: probe, then attach or manage, with a free-port fallback.
- **Rationale**: Fixed default ports keep attach mode predictable. `just kanata` (port 7070) still attaches to the first device, and a second Kanata the user starts by hand can use 7071.

## R5. Which devices a config grabs (FR-011)

Kanata 1.12, Linux:

- `linux-dev`: explicit device **paths** (`:`-separated, or a list). If present, only these are grabbed.
- Otherwise `linux-dev-names-include`: exact **names**. Only those are grabbed. "Partial matches and regexes are not supported."
- `linux-dev-names-exclude`: exact names to skip.
- With none of these, Kanata auto-detects, guided by `linux-device-detect-mode`. The default grabs keyboards. It grabs mice too once mouse buttons appear in `defsrc` (the `any` mode).

**Decision**:
- **Parsing**: the `kbd` crate reads these three `defcfg` options into `DeviceSelection { paths, include, exclude, mouse_buttons_in_defsrc }`.
- **Resolving**: the app resolves each selection against the devices present, by reading `/proc/bus/input/devices` (names and `eventN` handlers; no permissions needed). The result is the set of device names each config would grab:
  - **paths**: map each `eventN` to its device name;
  - **include**: the names that are present;
  - **auto**: every keyboard-like device (it has `EV_KEY` with letter keys), plus mouse-like ones if `defsrc` has mouse buttons, minus the excludes.
- **Overlap**: a non-empty intersection between two configs means a conflict, and the warning names both configs and the shared device names. A config that resolves through *auto* gets the "grabs every keyboard" warning, and the laptop keyboard is named.
- **Rationale**: It is exact for names, uses no extra permissions, and needs only parsing plus small pure functions. Resolution happens just before starting managed Kanatas.
- **Devices, not names (checked on the user's machine)**: `/proc/bus/input/devices` lists **three** devices named exactly `Compx 2.4G Wireless Receiver` (`event3` is the mouse; `event6` and `event7` are other receiver interfaces), plus `DYGMA DEFY Mouse` next to `DYGMA DEFY Keyboard`. Kanata's name include therefore grabs every device with that name. So `grabs` returns **sets of `eventN` devices** (each with its name, for messages), and overlap is computed on those.
- **Missing devices (Kanata 1.12, `src/oskbd/linux.rs`)**: if none of a config's selected devices is present, Kanata exits with "No keyboard devices were found…", unless `defcfg` has `linux-continue-if-no-devs-found yes`. This is a new failure kind, `NoDevice`: retried every 10 s or when `/dev/input` changes, and never counted towards give-up. The example configs set the option anyway.
- **Pure check**: the cross-device rules (unlimited, blocked by an earlier device) live in `kanata_client::check_devices`. The app only publishes its verdicts (constitution I).
- **Limit**: devices plugged in later are not re-checked. The check runs again when the device list changes or on **Restart**.

## R6. Mouse button labels (FR-010)

- **Decision**: Extend the `kbd` glyph table:
  - `BTN_SIDE` (Kanata `mbck`, Mouse 4) → `🖱←`;
  - `BTN_EXTRA` (`mfwd`, Mouse 5) → `🖱→`;
  - `BTN_LEFT`, `BTN_RIGHT`, `BTN_MIDDLE` → `🖱L`, `🖱R`, `🖱M`.

  The generated name table already maps `mbck`/`mfwd`/`🖰4`/`🖰5` to these codes.
- **Note**: The user's receiver exposes the keypad as "Compx 2.4G Wireless Receiver Keyboard" and the buttons as "Compx 2.4G Wireless Receiver". A mouse config needs both names in `linux-dev-names-include` (documented in the quickstart).

## R7. A missing layout (spec edge case)

- **Decision**: `layout::Layout::grid(names, columns)` builds a plain grid of 1u keys from a config's `defsrc`, 12 columns wide. Each key's Kanata name is set, so labels work.

## R8. Settings window

- **Decision**: A device selector at the top: a `ComboBox` of device names, plus **Add** and **Remove**. Below it, for the selected device:
  - name, config and layout fields, each with a **Browse…** button that opens `QtQuick.Dialogs` `FileDialog`. `QtQuick.Dialogs` is part of qtdeclarative, already in the dev shell, and on Plasma it shows KDE's own file dialog;
  - an optional address;
  - the existing placement controls, now per device;
  - the Kanata row (Restart, Start Kanata, Show log, Messages), bound to that device's `DeviceState`.
- **Overlapping boards**: if two boards have the same position and monitor, the window shows a hint (spec edge case).
