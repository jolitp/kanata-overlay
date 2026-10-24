---
description: "Task list for 007-desktop-integration"
---

# Tasks: Desktop Integration

**Input**: Design documents from `/specs/007-desktop-integration/`

**Prerequisites**: [plan.md](plan.md), [spec.md](spec.md), [research.md](research.md), [data-model.md](data-model.md), [contracts/interfaces.md](contracts/interfaces.md), [quickstart.md](quickstart.md)

**Tests**: Included for the pure helpers (labels, tooltip/menu model, `style` setting, entry contents, single-instance protocol, permission report), as the plan and constitution V require. The tray, the compact board and the entries are checked by hand: quickstart §2 (agent, scratch homes, fake Kanata), §3 (user).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependency on unfinished tasks).
- **[Story]**: The user story the task belongs to (US1–US4 from spec.md).

## Conventions for every task

- Work inside `nix develop`. `just check` must pass before each commit.
- **No root, no administrator prompts** (FR-012): the app writes only in the user's home and never runs `sudo`/`pkexec`.
- **Agent smoke tests** use the fake Kanata, a separate `--settings` file and scratch `HOME`, `XDG_DATA_HOME`, `XDG_CONFIG_HOME`, `XDG_RUNTIME_DIR` where entries or sockets are involved, never the user's real ones. Build the binary before a smoke test.
- **Constitution VI**: the tray shows layers and states only.

---

## Phase 1: Setup

- [ ] T001 Add `ksni = { version = "0.3", default-features = false, features = ["blocking"] }` to `[dependencies]` in `crates/app/Cargo.toml` (and the workspace `Cargo.toml` if dependencies are declared there); `cargo build -p kanata-overlay` succeeds.
- [ ] T002 [P] Create `assets/io.github.jolitp.kanataoverlay.svg`: a simple keyboard glyph (rounded rectangle with three rows of keys) in a neutral grey that reads on light and dark panels, 64×64 viewBox.
- [ ] T003 [P] Create empty modules `crates/app/src/tray_state.rs`, `crates/app/src/desktop_state.rs`, `crates/app/src/single_instance.rs`, `crates/app/src/desktop_entry.rs`, `crates/kanata-client/src/permissions.rs`; register the QObject files in `crates/app/build.rs`, the modules in `crates/app/src/main.rs` and `crates/kanata-client/src/lib.rs`.

---

## Phase 2: Foundational (Blocking Prerequisites)

- [ ] T004 [P] Settings: `pub enum Style { Full, Compact }` on `Device` in `crates/settings/src/lib.rs` ("missing → `"full"`; invalid → `"full"` with a settings warning; settings stay version 2"), parsed and saved as `"full"` / `"compact"` in `crates/settings/src/file.rs`, every `Device` literal updated; `device(id)` JSON gains `"style"` and `update_device(id, "style", v)` accepts both values in `crates/app/src/settings_state.rs`. Tests: missing, invalid with warning, round trip.
- [ ] T005 [P] Pure tray helpers in `crates/app/src/tray_state.rs` (a plain `mod model` with tests, no Qt): `short_label(layer: &str, connected: bool) -> String` (research R2: "first 4 characters, uppercase first letter"; `–` when not connected or empty; digits unchanged), `tooltip(name, layer, state, connected) -> (String, String)` (title = name, text = "layer · state" or "not connected · state"). Tests: `base`→`Base`, `nav`→`Nav`, `symbols`→`Symb`, `12`→`12`, `é`-initial names, not connected → `–`.
- [ ] T006 [P] Pure entry contents in `crates/app/src/desktop_entry.rs`: `desktop_entry(exec: &Path) -> String` (`[Desktop Entry]`, `Type=Application`, `Name=Kanata Overlay`, `Comment=On-screen layers for the Kanata key remapper`, `Exec=<quoted exec>`, `Icon=io.github.jolitp.kanataoverlay`, `Categories=Utility;`, `StartupNotify=false`), `autostart_entry(exec) -> String` (the same plus `X-KDE-autostart-after=panel`), and `install(data_home, exec)` writing the desktop file and the SVG (from `include_str!("../../../assets/io.github.jolitp.kanataoverlay.svg")`) only when contents differ, atomically. Tests: contents, `Exec` quoting of a path with spaces, unchanged file not rewritten (modification time kept).
- [ ] T007 [P] Single-instance protocol in `crates/app/src/single_instance.rs`: `socket_path(settings_path: &Path) -> PathBuf` (`$XDG_RUNTIME_DIR/kanata-overlay-<hash>.sock`, else `/tmp/kanata-overlay-<uid>-<hash>.sock`; `<hash>` = 8 hex digits of a stable hash (FNV-1a) of the settings path, so each settings file has its own instance), `enum Request { Settings }`, `parse(line) -> Option<Request>`, `fn claim(path) -> Claim { Primary(UnixListener), Secondary }` (connect first: if it answers, send `settings\n` and report Secondary; otherwise remove a stale file and bind). Tests: parse; two settings paths give two socket paths, the same path the same one; with a temp dir: first claim is Primary, a second is Secondary and the first's listener receives `settings`; a stale socket file is replaced.
- [ ] T008 [P] Permissions in `crates/kanata-client/src/permissions.rs`: `struct Facts { uinput_writable, input_readable, uinput_group_exists, db_groups: Vec<String>, session_groups: Vec<String>, nixos }`, `fn gather() -> Facts` (`libc::access`, `/dev/input/event*`, `/etc/group` for the user name from `$USER`/`getpwuid`, `getgroups` mapped to names, `/etc/NIXOS`), `fn report(&Facts) -> Report { items: Vec<(Item, Status)>, fix: String, nixos: bool }` with `Status { Ok, Missing, NeedsLogin }` per data-model.md › Permission report; the fix text is the README's NixOS block or its command block, only for missing parts, empty when all ok. Tests: all ok; uinput missing (NixOS and not); groups in the database but not the session → NeedsLogin and the fix says to log out and back in; input missing.

**Checkpoint**: `just check` passes; the app behaves as before.

---

## Phase 3: User Story 1 - See and control the overlay from the panel (Priority: P1) 🎯 MVP

**Goal**: one tray icon per device with its layer label, tooltip and the shared menu.

**Independent Test**: quickstart §2 item 1.

- [ ] T009 [US1] `TrayState` QObject in `crates/app/src/tray_state.rs` per contracts/interfaces.md: property `available`; invokable `update(devicesJson)` (`[{id, name, layer, state, connected, managed}]`); signals `toggleRequested`, `settingsRequested(deviceId)`, `restartRequested(deviceId)`, `quitRequested`. Keeps a map device id → (`ksni::blocking::Handle`, last label, last connected); creates a tray for a new id, drops it for a missing one, and on change calls `handle.update(...)`.
- [ ] T010 [US1] The `ksni::Tray` impl in `crates/app/src/tray_state.rs`: `id()` = `kanata-overlay-<device id>`, `title()` = device name, `tool_tip()` from `model::tooltip`, `icon_pixmap()` from the last drawn icon, `activate()` and menu items queue the signals through `qt_thread().queue` (Show/Hide boards, Settings…, Restart Kanata submenu with one entry per managed device, separator, Quit). On spawn failure (no StatusNotifierWatcher), set `available = false` once (FR-004).
- [ ] T011 [US1] Icon drawing in `crates/app/src/tray_state.rs` on the Qt thread: `QImage` 32×32 `Format_ARGB32_Premultiplied`, `QPainter` with antialiasing, rounded rect in `#6aa0ff` (connected) or `#8a8a8a`, `short_label` in bold white, font size stepped down until it fits; read pixels into ARGB32 big-endian bytes for `ksni::Icon`. Redraw only when label or connected changed.
- [ ] T012 [US1] Wire it in `crates/app/qml/Main.qml`: a `TrayState`; `root.updateTray()` building the devices JSON from `root.deviceStates` (name, `currentLayer`, `stateText` or `status`, `connected`, `mode === "managed"`), called from `updateDeviceStates()` (devices added/removed) and from each `DeviceView`'s `onNameChanged`, `onCurrentLayerChanged`, `onStateTextChanged`, `onStatusChanged`, `onConnectedChanged`, `onModeChanged` handlers on its `DeviceState` (a `Connections` in `crates/app/qml/DeviceView.qml` calling a `stateChanged` signal Main listens to); `onToggleRequested` → `overlayState.toggleOverlay()`, `onSettingsRequested(id)` → open the settings window on that device (`settingsWindow.selectedId = id; settingsWindow.open()`), `onRestartRequested(id)` → `deviceStates[id].restart()`, `onQuitRequested` → `Qt.quit()`; when `available` is false, add the global notice "no system tray: use the shortcuts".
- [ ] T013 [US1] Validate quickstart §2 item 1 with the fake Kanata (agent): two icons registered (check with `busctl --user` / `qdbus org.kde.StatusNotifierWatcher` for two `kanata-overlay-*` items), a layer change via `request_layer` updates only its icon (tooltip text via D-Bus), menu activation by script, and a no-tray session.

**Checkpoint**: US1 works.

---

## Phase 4: User Story 2 - A small board that only shows the layer (Priority: P1) 🎯 MVP

**Goal**: per-device compact board.

**Independent Test**: quickstart §2 item 2.

- [ ] T014 [P] [US2] `crates/app/qml/Board.qml`: `property bool compact`; compact shows only the header row (dot, "name · layer"), no keys and no notices line; `baseSize` from the header's width and height plus padding.
- [ ] T015 [US2] `crates/app/qml/DeviceView.qml` and the overlay windows: pass `compact: cfg.style === "compact"` to the board; `watched` also requires `!compact` (presses not read while compact, FR-006).
- [ ] T016 [US2] `crates/app/qml/SettingsWindow.qml`: a **Board style** combo (Full / Compact) per device, with `focusPolicy: Qt.TabFocus` like the others, saved like the other fields; loaded in `refresh()`.
- [ ] T017 [US2] Validate quickstart §2 item 2 with the fake Kanata (agent): screenshots full vs compact at the same corner, area ratio under 1/10 (SC-002), no highlights with `FAKE_KEYS` while compact.

**Checkpoint**: MVP (US1 + US2). Commit point.

---

## Phase 5: User Story 3 - Start at login and be known to the desktop (Priority: P2)

**Goal**: single instance, desktop entry and icon, Start at login.

**Independent Test**: quickstart §2 items 3–5.

- [ ] T018 [US3] In `crates/app/src/main.rs`, before creating the application: `single_instance::claim(socket_path(<settings path in use>))` → Secondary: print "kanata-overlay is already running; opened its settings" and exit 0; Primary: keep the listener for `DesktopState`. Then `desktop_entry::install` (errors logged, never fatal) with the launch target of T029.
- [ ] T019 [US3] `DesktopState` QObject in `crates/app/src/desktop_state.rs`: takes the listener (a `OnceLock` set by `main`), spawns a thread accepting connections and queuing `openSettings` per `Request::Settings`; property `startAtLogin` (autostart file exists) and invokable `setStartAtLogin(on)` (write `autostart_entry` atomically or remove it) under `$XDG_CONFIG_HOME/autostart` (fallback `~/.config/autostart`); at startup, an existing login entry is rewritten when its `Exec` differs from the current launch target (U2).
- [ ] T020 [US3] `crates/app/qml/Main.qml`: a `DesktopState`; `onOpenSettings` opens the settings window. `crates/app/qml/SettingsWindow.qml`: a **Start at login** checkbox bound to `desktop.startAtLogin`.
- [ ] T029 [US3] Launch target (research R9) in `crates/app/src/desktop_entry.rs`: `launch_target() -> PathBuf`: the value of `KANATA_OVERLAY_WRAPPER` when set and the executable is under `/nix/store`; else write `~/.local/share/kanata-overlay/launch.sh` (mode 755, atomically, only when different) exporting the current `QT_PLUGIN_PATH`, `QML2_IMPORT_PATH`, `QML_IMPORT_PATH` and (if set) `QT_QPA_PLATFORMTHEME`, then `exec <current exe> --kanata <absolute kanata path> "$@"` (the `--kanata` given on the command line, else `kanata` resolved on `PATH`; omitted if not found). Pure script builder tested (quoting, missing variables left out).
- [ ] T030 [US3] `flake.nix`: `packages.default` building the workspace's `kanata-overlay` binary (`rustPlatform.buildRustPackage` with the dev shell's Qt/KF6 inputs) and wrapping it with `makeWrapper`: `--set QT_PLUGIN_PATH`, `--set QML2_IMPORT_PATH`, `--set QML_IMPORT_PATH` from the same `qtEnv`, `--prefix PATH : ${kanata}/bin`, `--set KANATA_OVERLAY_WRAPPER $out/bin/kanata-overlay`; `nix build` succeeds and `result/bin/kanata-overlay --help`-style start works outside the dev shell (`env -i` with only `HOME`, `XDG_RUNTIME_DIR`, `WAYLAND_DISPLAY`, `DBUS_SESSION_BUS_ADDRESS`).
- [ ] T021 [US3] Validate quickstart §2 items 3–5 with scratch `HOME`/`XDG_*` (agent): second start exits and the first opens settings; desktop file and icon written; the "App info not found" warning gone on the next start; the autostart file follows the checkbox; the generated `launch.sh` starts the overlay from a clean environment (`env -i` as in T030) with its tray icons.

---

## Phase 6: User Story 4 - Get the permissions right (Priority: P2)

**Goal**: the permission check in the settings window.

**Independent Test**: quickstart §2 item 6 plus T008's tests.

- [ ] T022 [US4] `crates/app/src/desktop_state.rs`: property `permissionsJson` (`{items: [{name, status}], fix, nixos}`) and invokable `checkPermissions()` using `kanata_client::permissions::{gather, report}`.
- [ ] T023 [US4] `crates/app/qml/SettingsWindow.qml`: a **Permissions** row: "✓ Kanata can run without root" or each missing item with its status ("missing" / "log out and back in"), a read-only selectable box with the fix and a **Copy** button (clipboard via a `TextEdit.selectAll(); copy()`); `checkPermissions()` on every open. `crates/kanata-client/src/failure.rs`: the permission failure notice ends with "see Settings › Permissions".
- [ ] T024 [US4] Validate quickstart §2 item 6 (agent): the row shows this machine's state.

---

## Phase 7: Polish & Cross-Cutting Concerns

- [ ] T025 [P] README.md: "Tray, compact boards and login" section (icons per device, labels, menu, compact style, Start at login, single instance, permission check) and roadmap item 7 done; mention the "App info not found" warning is gone.
- [ ] T026 [P] CLAUDE.md: `tray_state.rs`, `desktop_state.rs`, `single_instance.rs`, `desktop_entry.rs`, `permissions.rs`; the no-widgets rule (tray through `ksni`).
- [ ] T027 User validation: quickstart §3.
- [ ] T028 Fill in the quickstart.md Validation log; `just check`; commit.

---

## Dependencies & Execution Order

- **Setup (T001–T003)** → **Foundational (T004–T008)** → **US1 (T009–T013)** → **US2 (T014–T017)** → **US3 (T018–T020, T029, T030, T021)** → **US4 (T022–T024)** → **Polish**.
- US1 needs T001, T005; US2 needs T004; US3 needs T006, T007 (T029 before T018's install call; T030 independent of the Rust work); US4 needs T008. US2 is independent of US1 and can go first.

### Parallel opportunities

- T002, T003; all of T004–T008 (different files and crates).
- T014 alongside T009–T011.
- T025, T026.

## Implementation Strategy

1. **MVP = US1 + US2**: tray icons per device and compact boards. Commit after T017.
2. **Then US3** (start at login, desktop entry, single instance) and **US4** (permission check).
