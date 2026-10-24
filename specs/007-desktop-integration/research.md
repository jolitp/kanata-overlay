# Research: Desktop Integration

## R1. Tray icons: `ksni`

- **Decision**: one `ksni` 0.3 StatusNotifierItem per device, with the `blocking` feature (each tray runs on its own thread, no async runtime of ours). Menus go over `ksni`'s DBusMenu implementation.
- **Rationale**: Qt's `QSystemTrayIcon` needs Qt Widgets and a `QApplication` (the app is a `QGuiApplication` with QML only); `Qt.labs.platform`'s tray falls back to widgets too. KDE's `KStatusNotifierItem` needs a `QMenu`, i.e. widgets as well, plus a new KF6 package and a C++ shim. `ksni` is a pure-Rust implementation of the same freedesktop/KDE StatusNotifierItem standard Plasma's tray uses (constitution IV), with menus, tooltips and pixmap icons.
- **Cost**: a new dependency (`ksni`, bringing `zbus`); accepted under VII because every alternative pulls in Qt Widgets.
- **Alternatives**: `tray-icon` (GTK/libappindicator on Linux: a second toolkit); writing SNI over D-Bus by hand (more code than the whole feature).

## R2. The layer label in the icon

- **Finding**: Plasma's tray shows an item's icon, not its title; text has to be drawn into the icon.
- **Decision**: on the Qt thread, `QPainter` draws onto a 32×32 `QImage` (premultiplied ARGB): a rounded square in the device's accent (grey when not connected) with the short label in bold, white, auto-sized to fit. The pixels are read out (`pixelColor`) into `ksni`'s ARGB32 big-endian `Icon` and handed to the device's tray thread. The icon is only redrawn when the label or the connected state changes.
- **Short label** (pure, tested): the layer name's first 4 characters, uppercase first letter (`base` → `Base`, `nav` → `Nav`, `game` → `Game`, `symbols` → `Symb`); "–" when not connected; a layer name that is all digits stays as is.
- **Rationale**: FR-002; Qt already renders text well, no font crate needed.

## R3. One icon per device, a shared menu

- **Decision**: `TrayState` (new QObject) keeps one `ksni` handle per device id, created and dropped as devices come and go (QML calls `update(json)` with every device's id, name, layer, state, connected and managed flag whenever any changes). Each icon's `id` is `kanata-overlay-<device id>`, title and tooltip title the device name, tooltip text "layer · state". Menu (same on every icon): **Show/Hide boards**, **Settings…** (opens on this icon's device), **Restart Kanata** → submenu with one entry per managed device, separator, **Quit**. Left click (`activate`) toggles the boards.
- **Threading**: `ksni` callbacks run on the tray's thread and only queue a signal to the Qt thread (`toggleRequested`, `settingsRequested(id)`, `restartRequested(id)`, `quitRequested`); QML connects them to the existing actions.
- **No tray** (FR-004): `ksni` reports a missing StatusNotifierWatcher on spawn; the overlay adds the global notice "no system tray: use the shortcuts" once and continues.

## R4. Compact board

- **Decision**: `Board.qml` gains `compact`: only the header row (state dot, "name · layer"), with padding; no keys, no notices line (the settings window still lists notices). The windows already size themselves from `board.baseSize`, so placement, scale, opacity and monitor apply unchanged. A compact device's `DeviceState.setWatching(false)` (feature 006): presses are dropped and nothing is highlighted (FR-006). Switching style needs no restart.
- **Setting**: `Device.style` (`full` | `compact`, default `full`) in settings v2, like `highlight`.

## R5. Single instance

- **Decision**: a Unix socket at `$XDG_RUNTIME_DIR/kanata-overlay-<hash>.sock` (fallback `/tmp/kanata-overlay-$UID-<hash>.sock`), where `<hash>` is a short hash of the settings file in use: one instance per settings file, so `just run --settings …` and scratch smoke tests don't collide with the everyday overlay. On start: try to connect; if it answers, send `settings\n` and exit 0; otherwise remove a stale socket file and listen. A listener thread queues "open settings" to the Qt thread for each `settings` line. Pure std (`UnixListener`/`UnixStream`), in `crates/app/src/single_instance.rs` with the protocol parsing testable.
- **Rationale**: FR-008, no new dependency, per user session by construction (the runtime dir is per user and per login).
- **Alternatives**: a D-Bus name (needs a D-Bus binding on the Qt side, or zbus directly: more for the same).

## R6. Desktop entry and icon

- **Decision**: at startup, write (only when missing or different) `~/.local/share/applications/io.github.jolitp.kanataoverlay.desktop` with `Exec=<this executable's absolute path>`, `Icon=io.github.jolitp.kanataoverlay`, `Name=Kanata Overlay`, `Comment`, `Categories=Utility;`, `StartupNotify=false`, and the icon `~/.local/share/icons/hicolor/scalable/apps/io.github.jolitp.kanataoverlay.svg` (shipped in the repo, embedded with `include_str!`). File contents are built by a pure function (tested); writes are atomic (`layout::edit::write_atomic`'s pattern).
- **Rationale**: FR-009. The portal warning ("App info not found for 'io.github.jolitp.kanataoverlay'") comes from the portal looking up `<app id>.desktop`; the file name matches the desktop file name the app already sets.

## R7. Start at login

- **Decision**: the presence of `~/.config/autostart/io.github.jolitp.kanataoverlay.desktop` (the XDG autostart standard Plasma follows), same contents as R6 plus `X-KDE-autostart-after=panel`. Ticking writes it, unticking removes it; the checkbox reads the file's presence.
- **Rationale**: FR-007, no service manager involved, user-owned (FR-012). Single instance (R5) makes a second start harmless.
- **Staying current**: at every start, if the login entry exists it is rewritten with the current `Exec` (R9) when it differs, like the desktop entry.

## R8. Permission check

- **Decision**: `kanata_client::permissions` (pure core + a thin gatherer):
  - **Facts** gathered: `/dev/uinput` exists and is writable (`access(W_OK)`); at least one `/dev/input/event*` readable (`access(R_OK)`); the user's groups in the user database (`/etc/group`) and in this session (`getgroups`); `uinput` group exists; `/etc/NIXOS` exists.
  - **Report** (pure, tested): items "input devices readable", "virtual keyboard writable", each with `Ok`, `Missing` or `NeedsLogin` (in the database but not this session); a fix text for this system: NixOS → the `configuration.nix` lines from the README; others → the README's commands. Only the missing parts are listed.
- **Rationale**: FR-010–FR-012: explain, never apply; the same logic as `just check-perms`, now in the app.
- **Where shown**: a "Permissions" row in the settings window (✓ ready / what's missing, a Copy fix button, and "log out and back in" when that's all); a managed Kanata's permission failure notice says "see Settings › Permissions".

## R9. Launchable outside the development shell

- **Finding**: started from the application menu or at login, the app doesn't get `nix develop`'s environment: `QT_PLUGIN_PATH` and `QML2_IMPORT_PATH` (the layer-shell QML module, `flake.nix:75-77`) are missing, and `kanata` isn't on the login `PATH`.
- **Decision** (both, chosen by where the binary runs from):
  - **Installed**: the flake gains `packages.default`, the binary wrapped (`makeWrapper`) with `QT_PLUGIN_PATH`, `QML2_IMPORT_PATH` from the same Qt environment and `kanata` (the flake's pinned one) prefixed to `PATH`; installed with home-manager or `nix profile install`. The wrapper sets `KANATA_OVERLAY_WRAPPER` to its own path; when that is set and the executable is under `/nix/store`, the entries' `Exec` is that path.
  - **Development build**: otherwise the app writes `~/.local/share/kanata-overlay/launch.sh`, which exports the current `QT_PLUGIN_PATH`, `QML2_IMPORT_PATH`, `QML_IMPORT_PATH`, `QT_QPA_PLATFORMTHEME` (if set) and runs the executable with `--kanata <absolute path of the kanata it would use>`; the entries run that script. It is rewritten at every start and stops working after a Nix garbage collection of those paths (documented).
- **Rationale**: SC-003 and FR-009 need an overlay that actually starts from the menu and at login; the package is the proper way, the script keeps `just run` users working.
