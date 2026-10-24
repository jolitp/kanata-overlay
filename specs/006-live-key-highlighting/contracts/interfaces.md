# Interfaces: Live Key Highlighting

## `kanata_client` additions

| Item | Meaning |
|------|---------|
| `output_tap::OutputTap` (trait) | `fn line(&self, line: &str) -> bool`: offered every captured output line (ANSI stripped); `true` = consumed, not logged |
| `output_tap::DebugKeyTap::new(sink: impl Fn(KeyEvent) + Send + Sync)` | consumes every `[DEBUG]`/`[TRACE]` line; key events go to `sink` |
| `output_tap::parse_key_event(line) -> Option<KeyEvent>` | pure parser for `process recv ev KeyEvent { code: NAME (N), value: Press }` |
| `KeyEvent { key: String, action: KeyAction }`, `KeyAction::{Press, Release, Repeat}` | |
| `SupervisorOptions.tap: Option<Arc<dyn OutputTap>>` | new field (default `None`); the caller adds `--debug` to `LaunchOptions.extra_args` when it sets a tap |
| `live::route(...) -> Route` | the filter (data-model › Filter), pure |

## `kbd` additions

| Item | Meaning |
|------|---------|
| `is_typing_key(canonical) -> bool` | research R8 |
| `preferred_name(canonical) -> Option<&'static str>` | shortest plain-text Kanata name for a canonical key |

## `settings` additions

| Item | Meaning |
|------|---------|
| `Device.highlight: Highlight` (`All`, `NoTyping`, `Off`) | data-model › Highlight setting |
| device JSON `"highlight"` and `updateDevice(id, "highlight", value)` | values `"all"`, `"no-typing"`, `"off"` |

## `DeviceState` additions

| Name | Kind | Meaning |
|------|------|---------|
| `pressedJson` | property | JSON array of held canonical keys |
| `tapHoldJson` | property | JSON object canonical → `"tap"` \| `"hold"` |
| `pressSource` | property | `"debug"`, `"tcp"` (tap-hold only) or `"off"` |
| `watching` | property (write from QML) | research R6 |
| `tick()` | invokable | drop expired tap-hold results (QML calls it every 100 ms while `tapHoldJson` isn't `{}`) |
| `capture()`, `cancelCapture()` | invokables | press to bind (research R7) |
| `captured(key)` | signal | the canonical key pressed while armed |

## QML

- `KeyCap.qml`, `EditorKey.qml`: `pressed: bool`, `tapHold: string` (`""`, `"tap"`, `"hold"`).
- `Board.qml`, `LayoutEditorWindow.qml`: pass them from the device's `pressedJson` / `tapHoldJson`.
- `SettingsWindow.qml`: a **Highlight** combo (All keys / No typing keys / Off) and, when `pressSource` is `"tcp"`, "pressed keys: only for a Kanata started by the overlay".
- `LayoutEditorWindow.qml`: **Press to bind** button beside **Clear**, with a 10 s countdown; Esc cancels.

## Fake Kanata (`crates/kanata-client/tests/fake-kanata.sh`)

- `FAKE_KEYS="KEY_A:Press KEY_A:Release …"`: with `--debug`, prints those events in Kanata's debug format every 100 ms, looping; plus a non-key debug line, to check it is dropped.
- `FAKE_TAPHOLD="caps:tap caps:hold"`: sends `TapActivated` / `HoldActivated` to connected clients, one per second, looping.
