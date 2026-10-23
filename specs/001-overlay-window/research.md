# Research: Overlay Window

**Feature**: [spec.md](spec.md) | **Plan**: [plan.md](plan.md) | **Date**: 2026-10-23

Versions checked against the dev shell's pinned nixpkgs: Qt 6.11.2, layer-shell-qt 6.7.5,
KGlobalAccel (KF6) 6.30.0, cxx-qt 0.10.0.

## R1. Overlay surface on Plasma Wayland

- **Decision**: Use layer-shell through LayerShellQt's **QML module** `org.kde.layershell`
  (attached properties `LayerShell.Window.*`). Settings: `layer: LayerOverlay`,
  `keyboardInteractivity: KeyboardInteractivityNone`, `activateOnShow: false`,
  `exclusionZone: 0`, `anchors` = the configured edge only, `margins` = the configured
  margin on that edge, `scope: "kanata-overlay"`, and the target `screen`.
- **Rationale**: The module ships with layer-shell-qt 6.7.5, which is already in the dev
  shell's `qt6.env` (`lib/qt-6/qml/org/kde/layershell`). It needs no C++ shim at all. The
  overlay layer sits above full-screen windows (FR-001). A surface anchored to one edge only
  is centred along that edge by the protocol (FR-007). Exclusion zone 0 avoids panels without
  reserving space (FR-006). Layer surfaces are never in the task manager and show on all
  virtual desktops (FR-004, FR-005).
- **Alternatives considered**:
  - The C++ `LayerShellQt::Window::get()` API via a shim. It works, but it adds C++ that the
    QML module makes unnecessary.
  - `LayerTop`. Full-screen windows cover it, so it would fail FR-001.
  - Exclusion zone `-1`. The overlay would sit on top of the Plasma panel.
- **Implementation notes (2026-10-23)**:
  - `LayerShell.Window.margins` is a `QMargins`, which QML can't construct. The margin is
    instead transparent padding on the anchored edge of the window; the whole window takes
    no input, so this is equivalent.
  - `LayerShell.Window.screen` expects a `QScreen*`, but QML's `Qt.application.screens`
    gives `QQuickScreenInfo`. Only the window's own `screen` is set; LayerShellQt's default
    `ScreenFromQWindow` follows it.
- **Notes**: Layer-shell properties must be set before the surface is first mapped. The
  window is created with `visible: false` and shown once the component is complete. When the
  target screen changes, the window is hidden and shown again, so the surface is re-created
  on the new output.

## R2. Click-through and no focus

- **Decision**: Set `Qt.WindowTransparentForInput | Qt.WindowDoesNotAcceptFocus` on the
  overlay window, on both platforms.
- **Rationale**: In Qt 6.11's Wayland client (`qtbase/src/plugins/platforms/wayland/qwaylandwindow.cpp`,
  `updateInputRegion()`), `WindowTransparentForInput` sets an **empty input region**, so all
  pointer and touch input passes through (FR-003). On X11, the xcb plugin sets an empty XShape
  input region for the same flag. Keyboard focus is covered by
  `KeyboardInteractivityNone` on Wayland and by `WindowDoesNotAcceptFocus` (WM_HINTS
  input=false) on X11 (FR-002).
- **Alternatives considered**: `QWindow::setMask` with a tiny region. It is a hack, and an
  empty mask means "no mask".

## R3. Fallback on X11 and on Wayland without layer-shell

- **Decision**: A second overlay window variant: a frameless window with
  `Qt.X11BypassWindowManagerHint | Qt.WindowStaysOnTopHint | Qt.WindowTransparentForInput |
  Qt.WindowDoesNotAcceptFocus | Qt.Tool`. Its absolute `x`/`y` are computed from the same
  placement function the settings crate uses. QML picks the variant with a `Loader`, based on
  a capability flag from Rust.
- **Rationale**: An override-redirect window (bypass hint) is not managed by the window
  manager. It is undecorated, on every desktop, not in the task bar, never focused, and KWin
  stacks it above normal windows. That covers FR-012 with only Qt flags and no new
  dependency. Keeping the layer-shell variant in its own QML file means LayerShellQt's
  attached object is never created on X11.
- **Alternatives considered**:
  - A managed window with `_NET_WM_STATE_ABOVE` plus KWindowSystem `setOnAllDesktops`.
    This adds a KF6 dependency for the same result.
  - A single QML file with conditional attached properties. Attached objects can't be made
    conditional cleanly.
- **Risk**: On Wayland compositors without layer-shell (e.g. GNOME), this fallback runs as a
  normal xdg-toplevel. "Above others", "all desktops" and "edge placement" are not available
  there, and FR-013 reports that.

## R4. Detecting layer-shell support

- **Decision**: At start-up, when Qt's platform is `wayland`, open a short-lived Wayland
  connection with the `wayland-client` crate and look for the `zwlr_layer_shell_v1` global
  in the registry. Result: `Capabilities { platform, layer_shell: bool }`.
- **Rationale**: This is precise and quick (one round-trip), and it decides which window
  variant is used and what FR-013 reports. Neither Qt nor LayerShellQt has a public
  "is layer-shell available" API.
- **Alternatives considered**:
  - Guessing from `XDG_CURRENT_DESKTOP`. This is wrong for sway and Hyprland (which do support
    layer-shell) and for nested sessions.
  - Asking Qt's native interface. It has no list of Wayland globals.

## R5. Global shortcuts

- **Decision**: KF6 **KGlobalAccel**, through a thin C++ shim compiled by
  `CxxQtBuilder::cpp_file(...)` (available in cxx-qt-build 0.10). The shim creates a
  `QAction` with `objectName` = shortcut id and the `componentName` / `componentDisplayName`
  properties. It calls `KGlobalAccel::setGlobalShortcut(action, QKeySequence)` and connects
  `QAction::triggered` to `QMetaObject::invokeMethod(receiver, "activate")`. The receiver is
  a cxx-qt `GlobalShortcut` QObject, passed in through `cxx_qt::Upcast<QObject>`. The shim
  returns whether registration succeeded.
- **Rationale**:
  - Plasma's kglobalacceld serves both Wayland and X11 sessions, so it covers the primary
    target in both.
  - Shortcuts appear under System Settings › Shortcuts and can be rebound there (FR-015).
  - KGlobalAccel loads the user's rebinding automatically.
  - The app only receives "triggered" and never sees other keys (FR-018).
  - No `.desktop` file or app-id registration is needed, unlike the portal.
  - Build: adding `kdePackages.kglobalaccel` to the dev shell's `buildInputs` puts its
    headers and library on the cc-wrapper search paths (the same mechanism as the libGL fix).
    `build.rs` only adds `cargo:rustc-link-lib=KF6GlobalAccel`.
- **Alternatives considered**:
  - The XDG `GlobalShortcuts` portal (`ashpd`). It is async, which conflicts with
    constitution VII. It needs an app id or host registration, and the user has to confirm a
    dialog on first use. Revisit it when non-Plasma desktops matter.
  - Talking to kglobalacceld's D-Bus API with `zbus`. That API is semi-internal, and zbus is
    a large dependency.
- **Failure handling**: If `setGlobalShortcut` returns false (no kglobalacceld, e.g. a
  non-Plasma X11 window manager), `registered` stays false and a notice is shown (FR-017).
- **Defaults**: toggle `Meta+Ctrl+K`, settings `Meta+Alt+Shift+K`. Component name
  `kanata-overlay`, display name "Kanata Overlay".
- **App identity**: `main.rs` sets `QCoreApplication::applicationName("kanata-overlay")` and
  `QGuiApplication::desktopFileName("io.github.jolitp.kanataoverlay")`. The latter also
  becomes the Wayland app_id of the settings window.

## R6. Opacity and scale

- **Decision**: Draw the overlay window with `color: "transparent"`. Apply `opacity` and
  `scale` to the root content item. The window size follows `base size × effective scale`.
- **Rationale**: Qt's Wayland client ignores `QWindow::opacity` (Wayland has no protocol for
  it), so window opacity can't be relied on. Content opacity works the same on both platforms
  (FR-008). Scaling the content root keeps the key geometry code unchanged (FR-009).
- **Effective scale**: Rust computes it. If `base × scale` does not fit the screen minus
  margins, it is reduced to the largest scale that fits, and a notice is set (edge case
  "larger than screen").
- **X11 note**: Transparency needs a compositing manager. Without one, the background is
  black, but the keys are still drawn.

## R7. Settings storage and overrides

- **Decision**: A new Qt-free library crate, `crates/settings`, with these parts:
  - **File**: `$XDG_CONFIG_HOME/kanata-overlay/settings.json`, falling back to
    `$HOME/.config/...`. It is versioned JSON (`"version": 1`), read through `serde_json::Value`
    so that each field is validated on its own.
  - **Lenient load**: Each invalid field falls back to its default with a warning, and the
    other fields still apply. A file from a newer version, or one that can't be parsed, gives
    all defaults plus a warning, and is marked read-only until the user saves from the
    settings window.
  - **Atomic save**: The file is written to `settings.json.tmp` in the same directory and then
    renamed over the original.
  - **Overrides**: Command-line values live in a separate `Overrides` struct. The effective
    settings are the file values with the overrides on top. When a field is edited in the
    settings window, its override is dropped and the new value is saved. Overrides are never
    written to the file (FR-011d).
  - **Saving**: The file is written when a change is committed in the settings window (a
    slider is released, or a choice is picked) and when that window closes. Hand edits made
    while the app runs are not picked up (spec assumption).
- **Rationale**: Logic stays in a library crate and is testable with `cargo test` alone
  (constitution I, V). It needs only std plus the serde and serde_json already in the
  workspace (VII).
- **Alternatives considered**:
  - `QSettings` / KConfig. This would put state on the Qt side and is harder to test.
  - The `dirs` crate. Four lines of std code are enough on Linux.
  - TOML. It would add a dependency, while the layouts are already JSON.

## R8. Monitor selection and hot-plug

- **Decision**: The monitor is stored by Qt screen name (the connector, e.g. `DP-1`), or
  `null` for the primary screen. QML resolves it against `Qt.application.screens`. If the
  name is missing, the primary screen is used and a notice is set. When the screen list
  changes, the binding re-evaluates and the window is re-shown on the resolved screen (R1).
- **Rationale**: Connector names are stable across reboots, unlike screen indexes. Using
  QML's own screen list keeps the code small (FR-010).

## R9. Settings window

- **Decision**: A separate, ordinary `Window` (`SettingsWindow.qml`) that uses **Qt Quick
  Controls** (ComboBox, Slider, SpinBox). Controls ship in `qtdeclarative`, which is already
  in the dev shell. The window binds to the `SettingsState` QObject.
- **Rationale**: It is a normal, focusable window (FR-011b). Live preview comes free, because
  the overlay binds to the same QObject (FR-011a).

## R10. Keeping the window flags separate from the settings window

- The overlay's flags and layer-shell properties are set per window, so the settings window
  is an ordinary xdg-toplevel or managed X11 window. Opening it doesn't change the overlay's
  behaviour (FR-011b).
