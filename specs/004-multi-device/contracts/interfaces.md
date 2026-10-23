# Contract: Settings, library and QML interfaces

## `settings` crate

```rust
pub struct Device { pub id: String, pub name: String, pub config: Option<PathBuf>,
                    pub layout: Option<PathBuf>, pub address: Option<String>,
                    pub placement: Placement /* edge, margin, opacity, scale, screen */ }
pub struct Settings { pub version: u32 /* 2 */, pub devices: Vec<Device> }

impl Settings {
    /// v1 → one "main" device; no devices + CLI → implicit device (data-model).
    pub fn effective_devices(&self, cli: &Overrides, cli_config: Option<&Path>,
                             cli_layout: Option<&Path>) -> Vec<Device>;
    pub fn add_device(&mut self, name: &str) -> String;   // returns the new id
    pub fn remove_device(&mut self, id: &str);
    pub fn device_mut(&mut self, id: &str) -> Option<&mut Device>;
}
pub fn default_address(index: usize) -> String;          // "127.0.0.1:{7070+index}"
```

## `kbd` crate

```rust
pub struct DeviceSelection { pub paths: Vec<String>, pub include: Vec<String>,
                             pub exclude: Vec<String>, pub mouse_buttons: bool }
impl Config { pub fn device_selection(&self) -> &DeviceSelection; }
```

## `kanata-client` crate

```rust
pub struct InputDevice { pub name: String, pub events: Vec<String>, pub keyboard: bool, pub mouse: bool }
pub fn input_devices() -> io::Result<Vec<InputDevice>>;       // reads /proc/bus/input/devices
pub fn parse_input_devices(text: &str) -> Vec<InputDevice>;   // pure, for tests
/// The devices (by eventN) a config would grab; names may repeat across devices.
pub fn grabs<'a>(sel: &kbd::DeviceSelection, present: &'a [InputDevice]) -> Vec<&'a InputDevice>;
```

`grabs` lives in `kanata-client`, which gains a `kbd` dependency. Both crates are Qt-free.

## QML bridge

### `SettingsState` (extended)

| Member | Kind | Notes |
|--------|------|-------|
| `deviceIds` | QString property | JSON array of ids, the model of the `Instantiator` |
| `revision` | i32 property | Bumped on every device change; bindings touch it (feature 001's lesson) |
| `device(id)` | invokable → QString | JSON of one device's fields |
| `addDevice(name)` / `removeDevice(id)` | invokables | Save, then bump `revision` and `deviceIds` |
| `updateDevice(id, field, value)` | invokable | Validates, then live-updates the device; saved on commit, as in feature 001 |
| `placement(id, screenX, screenY, screenW, screenH, baseW, baseH)` | invokable | As before, per device |

The single-device properties (`edge`, `margin` …) stay for one release, as aliases for the first device.

### `DeviceState` (new; replaces `KanataState`)

Properties: `deviceId`, `name`, `connected`, `currentLayer`, `status`, `layoutJson`,
`labelsJson`, `mode`, `stateText`, `logText`, `canRestart`, `canStart`, `notices`.

Invokables: `start()`, `restart()`, `startManaged()`, `shutdown()`.

### QML files

| File | Role |
|------|------|
| `Main.qml` | `Instantiator { model: JSON.parse(settingsState.deviceIds); delegate: DeviceView {} }`, plus the global objects |
| `DeviceView.qml` (new) | `QtObject` owning one `DeviceState` and creating its overlay window |
| `LayerShellOverlay.qml` / `FallbackOverlay.qml` | take `device` (a `DeviceState`) instead of `overlay` + `kanata`; placement via `settings.placement(device.deviceId, …)` |
| `Board.qml` | header: `device.name · layer · stateText · notices` |
| `SettingsWindow.qml` | device selector with Add/Remove, device fields with Browse…, per-device placement, per-device Kanata row |
