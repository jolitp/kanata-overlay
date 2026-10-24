# Implementation Plan: Live Key Highlighting

**Branch**: `006-live-key-highlighting` (stacked on `005-layout-editor`) | **Date**: 2026-10-23 | **Spec**: [spec.md](spec.md)

## Summary

A managed Kanata whose device has highlighting on is started with `--debug`. The
supervisor offers each output line to an optional tap. The tap swallows every debug
line, so none ever reaches the log, and passes key events to a filter on the reader
thread. The filter delivers a press to bind, drops events while the board is hidden,
drops typing keys under "no typing keys", and queues the rest to the device's
`DeviceState`. Tap-hold results come from Kanata's TCP messages for every Kanata.
`DeviceState` publishes the held keys and tap-hold results as two small JSON
properties; the boards and the layout editor's canvas light the matching keys. The
layout editor gains Press to bind.

## Technical Context

**Language/Version**: Rust stable (edition 2024); QML (Qt 6.11)

**Primary Dependencies**: none new. Kanata 1.12's `--debug` line format (research R1) and TCP `TapActivated`/`HoldActivated` (protocol 1.11+, already in `kanata-tcp-protocol`).

**Storage**: settings file v2 gains an optional per-device `highlight`; no key data is stored anywhere.

**Testing**:
- `cargo test`: parser, tap, filter, typing keys, preferred names, settings field, supervisor log without debug lines (fake Kanata);
- manual: quickstart §2 with the extended fake Kanata (agent), §3 with real Kanata (user).

**Target Platform**: Linux, Plasma Wayland first.

**Performance Goals**: a highlight within 50 ms of the event reaching the overlay (FR-002). One small property update per event, no delegate rebuilds. Debug output adds a few lines per key press to a reader thread that already exists.

**Constraints**:
- key data only in memory, never logged, including the log window (VI, FR-009);
- the debug-output dependency optional and behind a trait (II);
- filtering before the Qt thread (FR-005).

**Scale/Scope**:
- kanata-client: `output_tap.rs` and `live.rs` (about 200 lines plus tests), supervisor tap hook (about 30);
- kbd: about 40; settings: about 60;
- app: `DeviceState` live state, capture and TCP tap-hold (about 200);
- QML: KeyCap, EditorKey, Board, SettingsWindow and LayoutEditorWindow changes.

## Constitution Check

| Principle | Status | How |
|-----------|--------|-----|
| I. Rust owns state, QML renders | ✅ | Held keys, tap-hold results, expiry, filtering and capture live in Rust. QML only draws the two properties and calls `tick()`. |
| II. Stable Kanata interfaces | ✅ (justified use of an unstable one) | Tap-hold results use the TCP protocol. Presses need Kanata's debug output, which isn't stable. It is used only for managed Kanatas, behind the `OutputTap` trait, parsed by one tested function, and optional: highlighting "off" doesn't use it, and a format change only disables highlighting. |
| III. Never strand the keyboard | ✅ | No change to process handling. `--debug` is an extra argument; the restart rules reuse feature 003's path. |
| IV. Plasma Wayland first | ✅ | No windowing change. |
| V. Testable without hardware | ✅ | The fake Kanata emits debug key lines and TCP tap-hold messages; parser, tap and filter are pure. |
| VI. Key events are private | ✅ | Every debug line is swallowed before the log. Events live only in the filter and in two in-memory sets, cleared on hide, stop and "off". No history. `tracing` never sees key names (message kinds only, as today). Typing keys under "no typing keys" are dropped on arrival. |
| VII. Simple first | ✅ | No new dependency, no evdev access, one trait with one implementation. |

**Gate**: PASS. **Post-design**: PASS.

## Project Structure

```text
crates/kanata-client/src/
├── output_tap.rs   # NEW: OutputTap trait, DebugKeyTap, parse_key_event, KeyEvent
├── live.rs         # NEW: route() filter (pure)
├── supervisor.rs   # + optional tap in capture(); start() takes it
├── lib.rs          # + modules, re-exports
crates/kanata-client/tests/
├── fake-kanata.sh  # + FAKE_KEYS (debug lines), FAKE_TAPHOLD (TCP)
└── supervisor.rs   # + a tap test: key events reach the sink, the log has no debug line
crates/kbd/src/lib.rs         # + is_typing_key, preferred_name
crates/settings/src/          # + Highlight on Device, file parse/save, device JSON
crates/app/
├── src/device_state.rs       # live state, tap, TCP tap-hold, watching, tick, capture
├── src/settings_state.rs     # + "highlight" in device()/update_device
└── qml/
    ├── KeyCap.qml, EditorKey.qml       # pressed, tapHold
    ├── Board.qml, DeviceView.qml       # pass live state; watching
    ├── SettingsWindow.qml              # Highlight combo, source note
    └── LayoutEditorWindow.qml          # highlights on the canvas, Press to bind
```

## Implementation Notes

- **Order**:
  1. Foundational: kbd helpers; `output_tap` and `live` with tests; supervisor tap with a test against the fake Kanata; settings `highlight`.
  2. US1: DeviceState live state, `--debug` and the tap for managed devices, watching; drawing on the boards.
  3. US2: TCP tap-hold, expiry, badges.
  4. US3: capture, Press to bind in the editor; the editor canvas highlights.
  5. Polish: README, CLAUDE.md, quickstart log.
- **MVP**: US1 + US2 (both P1).
- **Attached Kanata**: `pressSource = "tcp"`; the settings window explains why presses don't show.

## Complexity Tracking

| Violation | Why needed | Simpler alternative rejected because |
|-----------|------------|--------------------------------------|
| Unstable interface (Kanata's debug output) | The only source of physical key presses while Kanata holds the device | The TCP protocol has no press events; reading the device is impossible while it is grabbed; Kanata's output device shows results, not keys (clarification Q1) |
