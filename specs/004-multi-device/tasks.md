---
description: "Task list for 004-multi-device"
---

# Tasks: Several Devices

**Input**: Design documents from `/specs/004-multi-device/`

**Prerequisites**: [plan.md](plan.md), [spec.md](spec.md), [research.md](research.md), [data-model.md](data-model.md), [contracts/interfaces.md](contracts/interfaces.md), [quickstart.md](quickstart.md)

**Tests**: Included for the Qt-free crates (settings v2 and migration, device selection, `/proc` parsing, grabs and overlap, glyphs, grid), as the plan and constitution V require. The multi-device UI is checked by hand: quickstart §2 with fake servers, §3 with real Kanata.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependency on unfinished tasks).
- **[Story]**: The user story the task belongs to (US1–US3 from spec.md).

## Conventions for every task

- Work inside `nix develop`. `just check` must pass before each commit.
- **Constitution III**: each device keeps its own `Supervisor`, and therefore its own thread with the parent-death signal. Dropping a `DeviceState` must stop its managed Kanata.
- **Agent smoke tests use fake Kanatas** (`just fake-kanata`, or `--kanata crates/kanata-client/tests/fake-kanata.sh`), never real Kanata.
- **Moved code keeps its behaviour.** Phase 2 is a refactor: feature 003's checks must still pass after it.

---

## Phase 1: Setup

- [X] T001 Add `kbd.workspace = true` to `[dependencies]` in `crates/kanata-client/Cargo.toml`. It is needed for `grabs()` in US3; check that nothing cyclic appears (`kbd` has no dependencies).
- [X] T002 [P] Create `crates/kanata-client/tests/fixtures/proc-input-devices.txt`. It is a trimmed copy of the user's `/proc/bus/input/devices` with only these device blocks, keeping their `N:`, `H:`, `B: EV=` and `B: KEY=` lines:
  - "AT Translated Set 2 keyboard" (`event0`);
  - "Compx 2.4G Wireless Receiver" (`event3` mouse, `event6`, `event7`);
  - "Compx 2.4G Wireless Receiver Keyboard" (`event4`);
  - "DYGMA DEFY Keyboard" (`event13`);
  - "DYGMA DEFY Mouse" (`event15`).

  Get them with `awk 'BEGIN{RS=""} /DYGMA|Compx|AT Translated/' /proc/bus/input/devices`. Drop the `P:`, `S:` and `U:` lines (paths and uniq ids).
- [X] T003 [P] Create `crates/settings/src/devices.rs`, an empty module with a doc comment, and declare `mod devices;` in `crates/settings/src/lib.rs`.

---

## Phase 2: Foundational (Blocking Prerequisites)

**⚠️ CRITICAL**: No user-story work can begin until this phase is complete. It ends with today's behaviour unchanged.

### Settings v2

- [X] T004 In `crates/settings/src/lib.rs`, add the device types (data-model.md › Device):
  - `pub struct Placement { edge, margin, opacity, scale, screen }`, moving those fields out of `Settings`;
  - `pub struct Device { id: String, name: String, config: Option<PathBuf>, layout: Option<PathBuf>, address: Option<String>, placement: Placement }`;
  - `pub struct Settings { version: u32, devices: Vec<Device> }`, with `VERSION = 2`.

  Rules, quoted from data-model.md:
  - id: "Unique, `[a-z0-9-]{1,32}`. Generated from the name on Add. Duplicates get `-2`, `-3`, …";
  - name: "1–40 characters";
  - placement ranges unchanged: "margin 0–500, opacity 10–100, scale 50–300".

  Keep `Overrides::effective` working on a `Placement`.
- [X] T005 Rewrite load and save in `crates/settings/src/file.rs` for v2 (data-model.md › Settings, version 2):
  - per-device, per-field leniency; an invalid device is skipped with `SettingsWarning::InvalidDevice { index, reason }`;
  - a **v1 file** becomes one device `{id: "main", name: "Keyboard"}` with the v1 placement fields, still writable, and saved as v2 on the next save;
  - `version` > 2 gives defaults, a warning, and `writable: false`;
  - save writes v2.
- [X] T006 [P] Update and extend the tests in `crates/settings/src/file.rs`:
  - a v2 round trip with two devices;
  - a v1 file migrates to `main` with its placement;
  - an invalid device among valid ones;
  - duplicate ids;
  - version 3 is not writable.

  The existing v1 tests become migration tests.
- [X] T007 Implement `crates/settings/src/devices.rs` (contracts/interfaces.md):
  - `Settings::effective_devices(&self, cli: &Overrides, cli_config, cli_layout) -> Vec<Device>`. With no devices and `--config` given, return one implicit `main` device built from the CLI, with the CLI placement overrides. With no devices and no config, return one implicit device with no config. With a single device, apply the CLI overrides to it, and fill in its config and layout from the CLI if they are missing (the v1 case). With several devices, ignore the CLI placement;
  - `add_device(name) -> id`, `remove_device(id)` and `device_mut(id)`;
  - `slug(name)`, following the id rule;
  - `default_address(index)` = `"127.0.0.1:{7070+index}"`.
- [X] T008 [P] Write tests in `crates/settings/src/devices.rs` covering each `effective_devices` case above, id generation (`"Mouse keypad"` → `mouse-keypad`, then `mouse-keypad-2`) and `default_address`.

### App: extract DeviceState (pure refactor)

- [X] T009 Create `crates/app/src/device_state.rs`, holding the `DeviceState` QObject (data-model.md › DeviceState). Do it in two steps, each followed by `just check` and a fake-Kanata smoke run: **(a)** move the connection and labels code from `overlay_state.rs`, while `KanataState` still exists; **(b)** merge `KanataState` into it and delete `kanata_state.rs`. Move everything per device into it:
  - **from `overlay_state.rs`**: the connection loop (`spawn_connection`), `current_layer`, `connected`, `status`, `layout_json`, `labels_json`, `config` and its relabel/reload logic, `with_canonical_names`, `reload_config` and `message_kind`;
  - **from `kanata_state.rs`**: the whole thing (mode, state text, log, can-restart/start, supervisor, probe, manage/attach, events, log throttling).

  Each `DeviceState` owns its address in an `Arc<Mutex<String>>`, which replaces the `ADDRESS` static. Add `#[qproperty(QString, device_id)]` and `#[qproperty(QString, name)]` properties; `start()` reads its device from `SettingsState` (T010) by id. `OverlayState` keeps only the global parts: `use_layer_shell`, `notices`, `overlay_shown`, `overlay_visible`, `previewing`, `toggle_overlay`, `set_previewing`, `add_notice` and the platform detection.

  Delete `crates/app/src/kanata_state.rs`, and update `build.rs` and the `main.rs` module list.
- [X] T010 Extend `crates/app/src/settings_state.rs` (contracts/interfaces.md › SettingsState):
  - **Properties**: `device_ids: QString` (a JSON array) and `revision: i32`;
  - **Invokables**: `device(id) -> QString` (JSON of id, name, config, layout, address and placement), `add_device(name) -> QString` (the new id), `remove_device(id)`, `update_device(id, field, value: QString)`, and `placement(id, screenX, screenY, screenW, screenH, baseW, baseH)`;
  - **Every change** saves on commit (as in feature 001), bumps `revision` and updates `device_ids`;
  - **On load**, compute `effective_devices` using `Args` (`--config`, `--layout`).

  Keep the single-device placement properties (`edge`, `margin`, `opacity`, `scale`, `screen_name`) and the `update*` invokables working as aliases for the first device.
- [X] T011 Create `crates/app/qml/DeviceView.qml`: a `QtObject` with `required property string deviceId`, `required property var settings`, `required property var overlay` and `property bool first`. It holds `property list<QtObject> objects: [DeviceState { id: device; deviceId: view.deviceId }]`. On `Component.onCompleted` it calls `device.start()` and creates the overlay window with `Qt.createComponent(overlay.useLayerShell ? "LayerShellOverlay.qml" : "FallbackOverlay.qml").createObject(view, { device, settings, overlay, first })`. On `Component.onDestruction` it calls `device.shutdown()` and destroys the window. It **also** connects `Qt.application.aboutToQuit` to `device.shutdown()`: `onDestruction` may only run after the event loop has ended, and the orderly SIGTERM stop of feature 003 must still happen on a normal quit (constitution III).

  In `crates/app/qml/Main.qml`, replace the single `KanataState`, window and connection wiring with `Instantiator { model: JSON.parse(settingsState.deviceIds); delegate: DeviceView { required property string modelData; required property int index; deviceId: modelData; first: index === 0; settings: settingsState; overlay: overlayState } }`.

  Update `LayerShellOverlay.qml`, `FallbackOverlay.qml` and `Board.qml`:
  - they take `required property var device` (labels, layer, mode, state text and notices come from it) plus `overlay` for visibility and global notices;
  - global notices show only on the board where `first` is true;
  - the header reads `device.name · layer · stateText · notices`.

  Add `DeviceView.qml` to `build.rs`.
- [X] T012 Refactor checkpoint: run `just check`, then the agent smoke tests from feature 003 against the fake Kanata (managed, `kill -9` of the overlay → no Kanata left, attached, `--kanata /nope`), and add the results to a new "Validation log" in `specs/004-multi-device/quickstart.md`. **Ask the user** to rerun feature 003's quickstart §3 with real Kanata before continuing.

**Checkpoint**: The app behaves exactly as in feature 003, now through one `DeviceState`.

---

## Phase 3: User Story 1 - Two Kanatas, two boards, one overlay (Priority: P1) 🎯 MVP

**Goal**: Several devices, described in the settings window. Each gets its own Kanata (managed or attached), its own board at its own position, and live add/remove.

**Independent Test**: quickstart.md §2 with two fake servers on 7070 and 7071: two boards, each labelled from its own config, a layer change on one changes only that board, and removing a device removes its board at once.

- [X] T013 [US1] In `crates/app/src/device_state.rs`, take the address from the device's `address`, or from `settings::default_address(index)`, where the index is the device's position in `device_ids`. Run feature 003's probe-then-attach-or-manage per device. Each managed device uses its own port (FR-006).
- [X] T014 [US1] Handle a missing device (research R5, spec edge case). Kanata 1.12 exits with "No keyboard devices were found…" when its selected devices are absent, unless `linux-continue-if-no-devs-found yes` is set (`src/oskbd/linux.rs`). In `crates/kanata-client/src/failure.rs`:
  - add `Failure::NoDevice`, detected by that message in `classify_exit`, with the notice "waiting for its device (not connected)";
  - make it retryable **without** counting towards the give-up limit.

  In `crates/kanata-client/src/supervisor.rs`, retry `NoDevice` every 10 s, or earlier if a `/dev/input/event*` file appears (poll the directory listing each second), and emit `State(WaitingForKeyboard)`. Add a unit test for the classification and a process test: a fake that prints the message and exits 1 must never reach `GaveUp`.
- [X] T015 [US1] Make placement per device in `crates/app/qml/LayerShellOverlay.qml` and `FallbackOverlay.qml`: the `place` binding calls `settings.placement(device.deviceId, …)`, touching `settings.revision` for the dependency (feature 001's binding lesson), and reads edge, margin, opacity and screen from `JSON.parse(settings.device(device.deviceId))`.
- [X] T016 [US1] Rework `crates/app/qml/SettingsWindow.qml` (research R8):
  - a device `ComboBox` (names from `settings.device(id)` for each id in `deviceIds`, re-read on `revision`), with **Add** (`settings.addDevice("New device")`, then select it) and **Remove** (confirm, then `settings.removeDevice(id)`);
  - for the selected device: a name `TextField`, then config and layout `TextField`s each with a **Browse…** button opening a `QtQuick.Dialogs` `FileDialog` (filters `*.kbd`; `*.json`), then an optional address `TextField`, all committed with `settings.updateDevice(id, field, value)`;
  - the existing placement controls, bound to that device;
  - the Kanata row and Messages for that device's `DeviceState`, which `Main.qml` passes in as `deviceStates`, a JS map of id → `DeviceState` built from the `Instantiator`'s `objectAdded`/`objectRemoved`;
  - a hint "overlaps <name>" when another device has the same edge and monitor.
- [X] T017 [US1] Verify live changes (FR-001a) with fake servers: adding a device creates its board and starts its session, and removing one destroys the board. For a managed device, its Kanata process (the fake) must be gone within 2 s. If destroying the QML object doesn't drop the Rust struct promptly, call `device.shutdown()` in `DeviceView`'s `onDestruction` (T011) before the object is released.
- [X] T018 [US1] Manually validate quickstart.md §2 (fake servers, agent) and §3 (real Kanata, user: SC-001, including the time from launch until both boards show "managed" against the 6 s budget, plus SC-002, SC-003 and SC-004), and add the results to the Validation log in `specs/004-multi-device/quickstart.md`.

**Checkpoint**: The MVP. Defy and mouse each have their own Kanata and board.

---

## Phase 4: User Story 2 - Mouse buttons and the keypad look right (Priority: P2)

**Goal**: Side buttons show their action, or `🖱←`/`🖱→`. A device without a layout gets a grid of its keys.

**Independent Test**: A config that maps `mbck` and leaves `mfwd` alone shows the action on one button and `🖱→` on the other. A device without a layout shows a grid labelled from its config.

- [X] T019 [P] [US2] In `crates/kbd/src/label.rs`'s glyph table, add `BTN_SIDE` → `🖱←`, `BTN_EXTRA` → `🖱→`, `BTN_LEFT` → `🖱L`, `BTN_RIGHT` → `🖱R` and `BTN_MIDDLE` → `🖱M` (research R6). Tests: `mbck`, `🖰4` and `mousebackward` all give `🖱←`, and `(defsrc mbck) (deflayer l C-c)` gives `⌃C`.
- [X] T020 [P] [US2] Add `pub fn grid(names: &[String], columns: usize) -> Layout` to `crates/layout/src/lib.rs`. It lays out 1u keys row by row, 12 columns by default, with `kanata` set to each name and `legend` set to the name. Tests: 13 names give 2 rows, and every key's `kanata` matches its name.
- [X] T021 [US2] In `crates/app/src/device_state.rs`, when the device has no layout, or the layout fails to load while a config is present, build the layout with `layout::grid` from the config's `defsrc` names (`kbd::Config::source[i].written`), and add a device notice: "no layout for <name>: showing a grid".
- [X] T022 [US2] Manually validate with the user's mouse config (quickstart §3, side-buttons row) and add the result to the Validation log in `specs/004-multi-device/quickstart.md`.

---

## Phase 5: User Story 3 - Devices never fight (Priority: P3)

**Goal**: Before starting managed Kanatas, warn about overlapping device selections and about configs without a device limit. Only the first of two conflicting Kanatas is started.

**Independent Test**: quickstart.md §4. A `mouse.kbd` without `defcfg` gives "grabs every keyboard: … AT Translated Set 2 keyboard …", and only the Defy's Kanata starts. "DYGMA DEFY Keyboard" in both configs gives a warning naming both configs. The real configs give no warning.

### Tests for User Story 3

- [X] T023 [P] [US3] Write tests in `crates/kbd/src/reader.rs` for `defcfg` device selection:
  - `linux-dev /dev/input/a:/dev/input/b`;
  - `linux-dev` with `\:` escapes;
  - `linux-dev (a "b c")`;
  - `linux-dev-names-include ("X" "Y")`;
  - `linux-dev-names-exclude (…)`;
  - `mouse_buttons` true when `defsrc` contains `mbck`.
- [X] T024 [P] [US3] Write tests in `crates/kanata-client/src/input_devices.rs` using the T002 fixture:
  - **parse**: 7 devices, and three named "Compx 2.4G Wireless Receiver" with `event3`, `event6` and `event7`;
  - **classify**: `keyboard` is set for the AT keyboard, the Defy keyboard and the Compx keyboard; `mouse` is set for `event3` and the Defy mouse;
  - **`grabs`**:
    - the include of both Compx names gives `event3`, `event4`, `event6` and `event7`;
    - the include of "DYGMA DEFY Keyboard" gives `event13` only;
    - auto gives every keyboard;
    - auto with mouse buttons adds the mice;
    - an exclude removes;
    - `linux-dev /dev/input/event13` gives the Defy;
  - **overlap**: the user's two configs don't overlap, and "auto + anything" does.

### Implementation for User Story 3

- [X] T025 [P] [US3] Add `pub struct DeviceSelection { paths, include, exclude, mouse_buttons }` to `crates/kbd/src/lib.rs` as a field of `Config`. In `crates/kbd/src/reader.rs`, parse it from `defcfg`: `linux-dev` as an atom split on unescaped `:`, or as a list; `linux-dev-names-include` and `linux-dev-names-exclude` as lists. Set `mouse_buttons` if any canonical `defsrc` name starts with `BTN_`.
- [X] T026 [P] [US3] Implement `crates/kanata-client/src/input_devices.rs` (contracts/interfaces.md), and re-export it from `lib.rs`:
  - `InputDevice`;
  - `parse_input_devices(text)`: blocks separated by blank lines; `N: Name="…"`; `H: Handlers=…`, keeping the `eventN` names; and the `B: EV=` / `B: KEY=` hex bitmaps. **Bitmap rule**: the space-separated hex words are 64-bit, most significant first. `keyboard` means the EV bit 1 (KEY) is set and the KEY bits 16–25, 30–38 and 44–50 (letters) are all set. `mouse` means the EV bit 2 (REL) is set and the KEY bit 272 (BTN_LEFT) is set;
  - `input_devices()`, which reads `/proc/bus/input/devices`;
  - `grabs(sel, present)`, following data-model.md › Grab resolution;
  - `pub fn conflicts(a, b) -> Vec<&InputDevice>`, the intersection by event.
- [X] T027 [US3] Add the pure device check to `crates/kanata-client/src/input_devices.rs` (constitution I: logic in library crates): `pub struct DeviceVerdict { pub id: String, pub unlimited: Option<String>, pub blocked_by: Option<String>, pub warnings: Vec<String> }` and `pub fn check_devices(devices: &[(String /*id*/, String /*config name*/, kbd::DeviceSelection)], present: &[InputDevice]) -> Vec<DeviceVerdict>`. Its rules:
  - `unlimited`: paths and include both empty → the warning "<config> would grab every keyboard: <names>";
  - `blocked_by`: the first earlier device whose grabs overlap, with the warning "<config> and <other config> both grab <names>; <this> was not started".

  Unit-test it with the T002 fixture: the user's configs (no verdicts), an auto config (unlimited), and a shared Defy (the second one blocked).
- [X] T028 [US3] Wire up the device check in `crates/app/src/settings_state.rs`. Whenever the device list loads or changes, read each device's config with `kbd::read` (only for `DeviceSelection`) and call `kanata_client::check_devices` against `input_devices()`. The bridge only publishes the result, and contains no rules of its own. Expose it through an invokable `device_check(id) -> QString` (JSON `{blocked, warnings}`), and bump `revision`. In `crates/app/src/device_state.rs`, `start()` reads the check: warnings become device notices, and a `blocked` device starts no managed Kanata. It may still attach if one already answers, because then the user started it. The state text is "not started: conflicts with <name>".
- [X] T029 [US3] Validate quickstart.md §4 (agent: with fake configs that carry the `defcfg` variants, using the user's real device list via `/proc` and no grabbing; user: with real Kanata) and add the result to the Validation log in `specs/004-multi-device/quickstart.md`.

---

## Phase 6: Polish & Cross-Cutting Concerns

- [X] T030 [P] Update `README.md`:
  - **New "Several devices" section**: the settings-window device editor, one Kanata per device, the `linux-dev-names-include` examples for the Defy and the mouse (including the three same-named Compx devices), per-device placement, and the overlap warnings;
  - **Untouched devices**: devices left out (like the laptop keyboard) are never grabbed or shown;
  - **Roadmap**: add the entry.
- [X] T031 [P] Update `CLAUDE.md` › Layout and Conventions: `DeviceState` (per device) vs `OverlayState` (global); `DeviceView.qml` and the `Instantiator`; settings v2.
- [X] T032 Run `just check` and fix every finding.
- [X] T033 Review spec.md FR-001 – FR-012 and SC-001 – SC-006 against the implementation and the Validation log, and note any gaps in `specs/004-multi-device/quickstart.md`.

---

## Dependencies & Execution Order

- **Setup (T001–T003)** → **Foundational (T004–T012)**. T004 → T005 → T007. T009 → T010 → T011 → T012.
- **US1 (T013–T018)**: needs the Foundational phase.
- **US2 (T019–T022)**: T019 and T020 are independent of everything except Setup. T021 needs T009.
- **US3 (T023–T029)**: T023–T027 need only Setup (T027 after T026). T028 needs T027, T010 and T009.
- **Polish**: last.

### Parallel Opportunities

- Setup: T002 and T003.
- Foundational: the tests T006 and T008, alongside the app refactor T009 (different crates).
- **Pure-crate work in parallel with the app work**: US2's T019 and T020, and US3's T023–T027, can all be done while Phase 2's app refactor is in progress.

## Parallel Example

```bash
Task: "T019 mouse glyphs in crates/kbd/src/label.rs"
Task: "T020 Layout::grid in crates/layout/src/lib.rs"
Task: "T025 DeviceSelection in crates/kbd/src/{lib,reader}.rs"
Task: "T026 input_devices in crates/kanata-client/src/input_devices.rs"
```

---

## Implementation Strategy

### MVP First

1. Setup → Foundational. The refactor checkpoint (T012) must show unchanged behaviour, **and the user reruns feature 003 §3**.
2. US1 → two boards with two fake servers → the user's real devices → commit.

### Incremental Delivery

1. US2 (mouse glyphs, grid) → commit.
2. US3 (overlap check) → commit.
3. Polish → PR, stacked on #3 or rebased once #3 is merged.

## Notes

- T012, T018, T022 and T029 are the manual test notes constitution V requires.
- Feature 003's tests (`crates/kanata-client/tests/supervisor.rs`) must keep passing unchanged. The supervisor itself isn't modified.
