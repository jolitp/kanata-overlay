# Interfaces: Desktop Integration

## `TrayState` QObject (new, `crates/app/src/tray_state.rs`)

| Name | Kind | Meaning |
|------|------|---------|
| `available` | property | false when no system tray answered (FR-004) |
| `update(devicesJson)` | invokable | `[{id, name, layer, state, connected, managed}]`; creates, updates, drops icons |
| `toggleRequested()` | signal | an icon was clicked, or Show/Hide boards |
| `settingsRequested(deviceId)` | signal | Settings… on that device's icon |
| `restartRequested(deviceId)` | signal | Restart Kanata › device |
| `quitRequested()` | signal | Quit |

Pure helpers (tested): `short_label(layer, connected) -> String`, `tooltip(entry) -> (title, text)`, the menu model.

## `DesktopState` QObject (new, `crates/app/src/desktop_state.rs`)

| Name | Kind | Meaning |
|------|------|---------|
| `startAtLogin` | property | the login entry exists |
| `setStartAtLogin(on)` | invokable | write or remove it |
| `permissionsJson` | property | `{items: [{name, status: "ok"|"missing"|"needs-login"}], fix, nixos}` |
| `checkPermissions()` | invokable | regather (also on settings window open) |
| `openSettings()` | signal | another instance asked (single instance) |

At startup (Rust, before QML): single-instance check (exit if another answers), desktop entry and icon installed or refreshed.

## `kanata_client::permissions` (new)

`gather() -> Facts`, `report(&Facts) -> Report` (pure), `Report { items, fix: String, nixos: bool }`.

## `settings`

`Device.style: Style { Full, Compact }`; device JSON `"style"`; `updateDevice(id, "style", "full"|"compact")`.

## QML

- `Board.qml`: `compact` (header only).
- `DeviceView.qml`: passes `compact`; `watched` also requires `!compact` (feature 006).
- `SettingsWindow.qml`: **Board** combo (Full / Compact) per device; **Start at login** checkbox; **Permissions** row (status, missing items, Copy fix, "log out and back in").
- `Main.qml`: a `TrayState`, fed from every `DeviceState` (name, layer, state, connected, mode); its signals wired to toggle, settings, restart, quit; the `DesktopState`'s `openSettings` opens the settings window.

## Files

- `assets/io.github.jolitp.kanataoverlay.svg` (new, the app icon).
- Written at runtime: the desktop entry, the icon, and (on request) the login entry, all in the user's home.
