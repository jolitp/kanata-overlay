# Research: Live Key Highlighting

## R1. Where presses come from: Kanata's debug output

- **Finding** (Kanata 1.12 source, `src/kanata/mod.rs`): every input event is logged at debug level as
  `process recv ev KeyEvent { code: KEY_A (30), value: Press }`
  (`value`: `Press`, `Release`, `Repeat`; `code` is the `OsCode` name, e.g. `KEY_CAPSLOCK`, `BTN_SIDE`). With `--debug` the logger (`simplelog` `TermLogger`, `TerminalMode::Mixed`, ANSI colours always) writes it to stdout as `HH:MM:SS.ffff [DEBUG] process recv ev …`. The supervisor already strips ANSI codes from every line.
- **Decision**: a managed Kanata whose highlighting isn't "off" is started with `--debug`. Its key events are parsed from stdout; the `OsCode` name is already the canonical key name the boards use for labels (`kbd::canonical`).
- **Rationale**: the only source of *physical* keys while Kanata holds the device exclusively (clarification Q1). The TCP protocol has no press events.
- **Alternatives**: reading the input device (impossible while Kanata grabs it); Kanata's output device (shows the result, not the key; rejected in Q1).

## R2. Isolation and privacy of the debug output (constitution II and VI)

- **Decision**:
  - `kanata_client::output_tap`: `pub trait OutputTap: Send + Sync { fn line(&self, line: &str) -> bool }`. The supervisor offers every captured output line to the tap first; `true` means "consumed, don't log it".
  - `DebugKeyTap` (the only implementation) consumes **every** `[DEBUG]` and `[TRACE]` line, whether or not it is a key event, so nothing Kanata logs at those levels (which can name keys in other messages too) reaches the in-memory log, the log window or `tracing`. Key events are parsed and passed to a sink closure; other debug lines are dropped.
  - Parsing is a pure function, `parse_key_event(line) -> Option<KeyEvent>`, tested against lines captured from Kanata's source format; a line that doesn't match is simply not a key event (the feature degrades, nothing breaks).
- **Rationale**: II requires unstable interfaces to be optional and behind a trait; VI requires key data to stay in memory. Dropping all debug lines is simpler and safer than deciding which ones mention keys.
- **Alternatives**: logging non-key debug lines (risk of key names leaking into the log window).

## R3. Filtering on the reader thread

- **Decision**: the sink, built by `DeviceState`, runs on the output reader thread and checks, in order, before anything is queued to the Qt thread:
  1. press-to-bind armed → a `Press` becomes the captured key (any key, even under "no typing keys"), disarms, nothing else;
  2. not watching (board hidden and editor not on this device) → dropped;
  3. mode "no typing keys" and `kbd::is_typing_key` → dropped;
  4. otherwise queued as a press/release update.
  `Repeat` is ignored (a held key is already shown).
- **Rationale**: FR-005 ("typing keys are dropped as soon as they arrive"), FR-004 ("no presses are kept while hidden"); the Qt thread only ever sees what it may show.
- **Shared state**: the mode, watching flag and armed flag are atomics shared with the sink, so changing them needs no restart.

## R4. Tap-hold results over TCP

- **Finding**: `ServerMessage::HoldActivated { key }` / `TapActivated { key }` (protocol 1.11+, in our `kanata-tcp-protocol` 1.1130) carry the physical key name (`"caps"`).
- **Decision**: the connection thread maps the name with `kbd::canonical` and passes it through the same filter as presses (watching, and "no typing keys" drops a typing key's result), then queues a tap-hold update. It still logs only the message kind (constitution VI). "Tap" shows for 1 s. "Hold" shows until that key's release arrives (R1), or for 1.5 s when there is no debug source (Kanata started outside the overlay).
- **Rationale**: FR-003; works for every connected Kanata.

## R5. Restart rules

- **Decision**: whether a managed Kanata gets `--debug` is part of its setup: switching between "off" and an "on" mode restarts it (with the existing restart path); "all" ↔ "no typing keys" only flips the atomic. Turning highlighting off restarts Kanata without `--debug`, so it stops producing key events at the source.
- **Rationale**: FR-005 ("its Kanata is started without detailed output").

## R6. Watching

- **Decision**: `DeviceState.watching` (bool, set from QML) = the overlay is shown, or the layout editor is open on this device. Turning it off clears held keys and tap-hold results; while off the sink drops events.
- **Rationale**: FR-004 and the "overlay hidden" edge case.

## R7. Press to bind

- **Decision**: `DeviceState.capture()` arms the next press; it is delivered as the signal `captured(canonical)`; `cancelCapture()` disarms. The editor binds the captured key to the config's own spelling of that canonical key if the config has one, otherwise `kbd::preferred_name(canonical)` (shortest plain-text name, the picker's rule from feature 005). The 10-second timeout and Esc are QML timers/shortcuts calling `cancelCapture`. Capture needs a debug source; otherwise the button is disabled with the reason (FR-006, US3 scenario 3).
- **Rationale**: the overlay only watches: Kanata still acts on the press (FR-007).

## R8. Typing keys

- **Decision**: `kbd::is_typing_key(canonical)`: `KEY_A`–`KEY_Z`, `KEY_0`–`KEY_9`, `KEY_MINUS`, `KEY_EQUAL`, `KEY_LEFTBRACE`, `KEY_RIGHTBRACE`, `KEY_BACKSLASH`, `KEY_SEMICOLON`, `KEY_APOSTROPHE`, `KEY_GRAVE`, `KEY_COMMA`, `KEY_DOT`, `KEY_SLASH`, `KEY_SPACE`, `KEY_102ND`, `KEY_KP0`–`KEY_KP9`, `KEY_KPDOT`.
- **Rationale**: the spec's assumption list; everything a password is made of.

## R9. Drawing

- **Decision**: `DeviceState` publishes `pressedJson` (array of canonical names) and `tapHoldJson` (`{canonical: "tap" | "hold"}`). `Board.qml` and the layout editor's canvas look keys up by `kanataCanonical`, so duplicate keys both light. `KeyCap` / `EditorKey` gain `pressed` (lighter face, accent border) and `tapHold` (a small "tap"/"hold" badge).
- **Expiry**: tap-hold results carry their start time in Rust. While any is shown, a QML `Timer` calls `DeviceState.tick()` every 100 ms, and Rust drops expired results (state stays in Rust, constitution I).
- **Rationale**: FR-002 (one property update per event, no delegate rebuilds: the layout JSON doesn't change, only these two small properties).

## R10. Testing without hardware

- **Decision**: `fake-kanata.sh` gains `FAKE_KEYS="KEY_A:Press KEY_A:Release …"` (with `--debug`, prints those lines in Kanata's format, 100 ms apart, then repeats) and `FAKE_TAPHOLD="caps:hold caps:tap"` (sent over its TCP connection). Unit tests: `parse_key_event` (real formats, ANSI-stripped, non-matching lines), `DebugKeyTap` consumes all debug lines and logs none, `is_typing_key`, `preferred_name`, the filter order (R3) as a pure function, settings `highlight` round trip and leniency.
