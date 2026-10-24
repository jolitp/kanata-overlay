---
description: "Task list for 006-live-key-highlighting"
---

# Tasks: Live Key Highlighting

**Input**: Design documents from `/specs/006-live-key-highlighting/`

**Prerequisites**: [plan.md](plan.md), [spec.md](spec.md), [research.md](research.md), [data-model.md](data-model.md), [contracts/interfaces.md](contracts/interfaces.md), [quickstart.md](quickstart.md)

**Tests**: Included for the Qt-free crates (parser, tap, filter, kbd helpers, settings field, supervisor with the fake Kanata), as the plan and constitution V require. The boards and the editor are checked by hand: quickstart §2 with the fake Kanata (agent), §3 with real Kanata (user).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependency on unfinished tasks).
- **[Story]**: The user story the task belongs to (US1–US3 from spec.md).

## Conventions for every task

- Work inside `nix develop`. `just check` must pass before each commit.
- **Constitution VI**: key names from presses or tap-hold results are never passed to `tracing`, `eprintln!`, the supervisor log or any file. Log message *kinds* only. Every `[DEBUG]`/`[TRACE]` line from a tapped Kanata is swallowed before the log.
- **Constitution II**: the debug output is read only through `OutputTap`; nothing else parses Kanata's log for keys.
- **Agent smoke tests use the fake Kanata** and a separate `--settings` file, never real Kanata or the user's settings. Build the binary before a smoke test.

---

## Phase 1: Setup

- [X] T001 Create `crates/kanata-client/src/output_tap.rs` and `crates/kanata-client/src/live.rs` with module doc comments (research R1–R3) and declare them in `crates/kanata-client/src/lib.rs`.

---

## Phase 2: Foundational (Blocking Prerequisites)

**⚠️ CRITICAL**: No user-story work can begin until this phase is complete.

- [X] T002 [P] In `crates/kbd/src/lib.rs` add `pub fn is_typing_key(canonical: &str) -> bool` with exactly the keys of research R8 ("`KEY_A`–`KEY_Z`, `KEY_0`–`KEY_9`, `KEY_MINUS`, `KEY_EQUAL`, `KEY_LEFTBRACE`, `KEY_RIGHTBRACE`, `KEY_BACKSLASH`, `KEY_SEMICOLON`, `KEY_APOSTROPHE`, `KEY_GRAVE`, `KEY_COMMA`, `KEY_DOT`, `KEY_SLASH`, `KEY_SPACE`, `KEY_102ND`, `KEY_KP0`–`KEY_KP9`, `KEY_KPDOT`") and `pub fn preferred_name(canonical: &str) -> Option<&'static str>` (shortest plain-text spelling, ties alphabetical; the rule `spelling_order` uses in `crates/app/src/layout_editor.rs`, which then calls this instead). Tests: letters, digits, `KEY_SPACE`, `KEY_KP5` are typing keys; `KEY_LEFTSHIFT`, `KEY_ESC`, `KEY_UP`, `KEY_F1`, `BTN_SIDE` are not; `preferred_name("KEY_ENTER") == Some("ent")`, `preferred_name("KEY_A") == Some("a")`, unknown → `None`.
- [X] T003 [P] `crates/kanata-client/src/output_tap.rs`: `pub enum KeyAction { Press, Release, Repeat }`, `pub struct KeyEvent { pub key: String, pub action: KeyAction }`, `pub fn parse_key_event(line: &str) -> Option<KeyEvent>` (finds `process recv ev KeyEvent { code: NAME (N), value: VALUE }` anywhere in the line; NAME is kept as is, it is already canonical), `pub trait OutputTap: Send + Sync + std::fmt::Debug { fn line(&self, line: &str) -> bool; }` and `pub struct DebugKeyTap` (holds `Box<dyn Fn(KeyEvent) + Send + Sync>`, manual `Debug` that prints no events) whose `line` returns `true` for every line containing `[DEBUG]` or `[TRACE]` and calls the sink for key events among them. Tests with real-format lines: `08:00:03.7792 [DEBUG] process recv ev KeyEvent { code: KEY_A (30), value: Press }`, `Release`, `Repeat`, `BTN_SIDE (275)`, a line without the timestamp; a non-key debug line is consumed and not passed on; `[INFO]` lines are not consumed; malformed key lines are consumed and ignored.
- [X] T004 `crates/kanata-client/src/live.rs`: `pub enum Mode { All, NoTyping, Off }`, `pub enum Route { Capture(String), Show(KeyEvent), Drop }`, `pub fn route(event: KeyEvent, armed: bool, watching: bool, mode: Mode) -> Route` implementing data-model.md › Filter in that row order. Tests: one per row, plus "armed beats not watching" and "armed accepts a typing key under NoTyping".
- [X] T005 In `crates/kanata-client/src/supervisor.rs` add `pub tap: Option<Arc<dyn OutputTap>>` to `SupervisorOptions` (default `None`) and in `capture()` offer each stripped line to the tap first; only lines it doesn't consume go to the log (and `LogChanged` is emitted only for those). Re-export `OutputTap`, `DebugKeyTap`, `KeyEvent`, `KeyAction`, `parse_key_event`, `live` items from `crates/kanata-client/src/lib.rs`.
- [X] T006 Extend `crates/kanata-client/tests/fake-kanata.sh`: with `--debug` and `FAKE_KEYS="NAME:Value …"`, a background loop prints each event as `HH:MM:SS.ffff [DEBUG] process recv ev KeyEvent { code: NAME (0), value: Value }` (with ANSI colour around `[DEBUG]`, like Kanata) every 100 ms, plus one non-key `[DEBUG] layer stack …` line per round; with `FAKE_TAPHOLD="caps:tap caps:hold"`, every connected client receives `{"TapActivated":{"key":"caps"}}` / `{"HoldActivated":{"key":"caps"}}` in turn, one per second. Without `--debug` it prints no debug lines.
- [X] T007 Test in `crates/kanata-client/tests/supervisor.rs`: a supervisor with a `DebugKeyTap` whose sink collects events, `--debug` in `extra_args`, `FAKE_KEYS` set: events arrive in order, and `log_text()` contains no `[DEBUG]` line and no `KEY_` name.
- [X] T008 [P] Settings: `pub enum Highlight { All, NoTyping, Off }` on `Device` in `crates/settings/src/lib.rs` ("missing or invalid → `"all"` (with a settings warning for invalid)"), parsed and saved as `"all"`, `"no-typing"`, `"off"` in `crates/settings/src/file.rs` (always written), settings version stays 2. Tests: round trip, missing → All, `"bogus"` → All plus `InvalidField`.
- [X] T009 In `crates/app/src/settings_state.rs`: `device(id)` JSON gains `"highlight"`; `update_device(id, "highlight", v)` accepts the three values.

**Checkpoint**: `just check` passes; the app behaves exactly as before (no tap set anywhere yet).

---

## Phase 3: User Story 1 - See which key I'm pressing (Priority: P1) 🎯 MVP

**Goal**: held keys light on their device's board; "no typing keys" and "off" respected; nothing kept while hidden.

**Independent Test**: quickstart §2 items 1, 3, 4, 5, 6.

- [X] T010 [US1] `crates/app/src/device_state.rs` live state (data-model.md › Live state): properties `pressed_json` (`"[]"` default), `tap_hold_json` (`"{}"`), `press_source` (`"debug"` | `"tcp"` | `"off"`), `watching` (bool); shared `Arc<AtomicU8>` mode, `Arc<AtomicBool>` watching and armed; `held: BTreeSet<String>`. `Setup` gains `detailed: bool` (`highlight != Off`, so switching off ↔ on restarts, research R5); `All` ↔ `NoTyping` only updates the atomic in `apply()`.
- [X] T011 [US1] In `crates/app/src/device_state.rs` `manage()`: when `detailed`, push `--debug` onto `LaunchOptions.extra_args` and set `SupervisorOptions.tap` to a `DebugKeyTap` whose sink calls `live::route` with the atomics and, for `Show`, queues (`qt_thread().queue`) a held-set update and publishes `pressed_json`; set `press_source` to `"debug"`, attached sessions `"tcp"`, `Off` `"off"`.
- [X] T012 [US1] Clearing in `crates/app/src/device_state.rs` (FR-004): empty `held` and `tap_hold` and republish on disconnect, every supervisor state other than Running, `shutdown()`, `watching = false` (a `watching` setter that also stores the atomic), and mode `Off`.
- [X] T013 [P] [US1] `crates/app/qml/KeyCap.qml`: `property bool pressed`, drawn as a lighter face (`#3d4048`) with an accent border (`#6aa0ff`, 2 px); `crates/app/qml/Board.qml` passes `pressed: board.pressed.includes(modelData.kanataCanonical)` from `JSON.parse(device.pressedJson)`.
- [X] T014 [US1] `crates/app/qml/DeviceView.qml` binds `deviceState.watching` to `overlay.overlayShown || <editor open on this device>` (Main.qml passes the editor's `deviceId`).
- [X] T015 [US1] `crates/app/qml/SettingsWindow.qml`: a **Highlight** combo (All keys / No typing keys / Off → `"all"`, `"no-typing"`, `"off"`) per device, saved like the other fields; under it, when `kanata.pressSource === "tcp"`, the note "pressed keys: only for a Kanata started by the overlay" (FR-010).
- [X] T016 [US1] Validate quickstart §2 items 1, 3, 4, 5, 6 with the fake Kanata (agent), with screenshots and a check of the log window text.

**Checkpoint**: US1 works.

---

## Phase 4: User Story 2 - See whether a tap-hold key tapped or held (Priority: P1) 🎯 MVP

**Goal**: "tap" for 1 s, "hold" until release (or 1.5 s without a debug source), for every Kanata.

**Independent Test**: quickstart §2 item 2.

- [X] T017 [US2] In the connection thread of `crates/app/src/device_state.rs`, handle `ServerMessage::TapActivated { key }` / `HoldActivated { key }`: canonicalise with `kbd::canonical`, filter with `live::route` (as a `Press`, `armed = false`), queue a tap-hold update `(key, Tap|Hold, Instant::now())`; keep logging only the message kind.
- [X] T018 [US2] `tick()` invokable in `crates/app/src/device_state.rs`: drop `Tap` older than 1 s, `Hold` older than 1.5 s when `press_source != "debug"`; a `Release` of a key ends its `Hold`; republish `tap_hold_json` only when it changed.
- [X] T019 [P] [US2] `crates/app/qml/KeyCap.qml`: `property string tapHold` drawn as a small badge ("tap" / "hold", accent colours) in the key's bottom-right; `crates/app/qml/Board.qml` passes it from `JSON.parse(device.tapHoldJson)`; a `Timer` (100 ms, running while `tapHoldJson !== "{}"`) calls `device.tick()`.
- [X] T020 [US2] Validate quickstart §2 item 2 with the fake Kanata (agent), including an attached-style run (no `--debug`): results still show and expire.

**Checkpoint**: MVP (US1 + US2). Commit point.

---

## Phase 5: User Story 3 - Bind a key by pressing it (Priority: P2)

**Goal**: Press to bind in the layout editor; the editor's canvas highlights held keys.

**Independent Test**: quickstart §2 item 7.

- [X] T021 [US3] `crates/app/src/device_state.rs`: invokables `capture()` (sets `armed`; only when `press_source == "debug"`) and `cancel_capture()`; signal `captured(key: QString)` emitted for `Route::Capture`, which also clears `armed`.
- [X] T022 [US3] `crates/app/qml/LayoutEditorWindow.qml`: **Press to bind** beside **Clear** (single selection; disabled with the reason "needs a Kanata started by the overlay, with highlighting on" otherwise); while waiting, the button reads "Press a key… (Ns)" with a 10 s countdown; Esc or a click on the canvas cancels (`cancelCapture`). On `captured(key)` bind the selected key to the config's own spelling (the first `configKeysJson` entry whose `kbd::canonical` matches, via a new `LayoutEditor.nameFor(canonical)` invokable in `crates/app/src/layout_editor.rs`), else `kbd::preferred_name`.
- [X] T023 [P] [US3] `crates/app/qml/EditorKey.qml` and `crates/app/qml/LayoutEditorWindow.qml`: `pressed` and `tapHold` on canvas keys from the edited device's `pressedJson` / `tapHoldJson` (FR-008), same look as the board.
- [X] T024 [US3] Validate quickstart §2 item 7 with the fake Kanata (agent).

**Checkpoint**: all stories work.

---

## Phase 6: Polish & Cross-Cutting Concerns

- [X] T025 Privacy check (SC-005, quickstart §2 item 8): after a fake-Kanata run with `FAKE_KEYS`, search the scratch settings folder, its `layouts/`, the app's stderr and the log window text for `KEY_` names and the fake key names; record the result.
- [X] T026 [P] README.md: a "Live key highlighting" section (what lights, tap/hold badges, the Highlight setting and its privacy meaning, attached Kanatas show tap/hold only, Press to bind) and mark roadmap item 6 done.
- [X] T027 [P] CLAUDE.md: `output_tap.rs` / `live.rs`, the rule that debug lines never reach the log, where filtering happens.
- [X] T028 User validation with real devices: quickstart §3.
- [X] T029 Fill in the quickstart.md Validation log; `just check`; commit.

---

## Dependencies & Execution Order

- **Setup (T001)** → **Foundational (T002–T009)** → **US1 (T010–T016)** → **US2 (T017–T020)** → **US3 (T021–T024)** → **Polish**.
- Within Foundational: T003 before T005 and T007; T006 before T007; T004 after T003 (uses `KeyEvent`); T008 before T009.
- US2's hold expiry uses US1's releases; US3 uses US1's tap and US2's badges on the canvas.

### Parallel opportunities

- T002, T003, T008 (different crates); T004 once T003 exists.
- T013 with T011–T012; T019 with T017–T018; T023 with T021–T022.
- T026, T027.

## Implementation Strategy

1. **MVP = US1 + US2** (both P1): highlights and tap/hold badges on the boards. Commit after T020.
2. **Then US3**: Press to bind, on top of the same tap.
3. Privacy check (T025) before the final commit.
