# Contract: QML ↔ Rust Bridge

The QObjects QML sees, all in module `io.github.jolitp.kanataoverlay`. They use
`#[auto_cxx_name]`, so QML names are camelCase. Following constitution I, QML only binds and
calls; validation, clamping, placement maths and persistence happen in Rust (the `settings`
crate).

## OverlayState *(existing, extended)*

Existing: `currentLayer`, `connected`, `status`, `layoutJson`, `start()`, `requestLayer(name)`.

New properties:

| Property | Type | Meaning |
|----------|------|---------|
| `useLayerShell` | bool | True on Wayland when `zwlr_layer_shell_v1` is available (research R4). Picks the overlay variant. |
| `notices` | QString | Human-readable notices joined with ` · `: FR-013 missing features, shortcut failures, settings warnings, scale capped, monitor missing. It is empty when there is nothing to report, and is shown in the header's status line. |
| `overlayShown` | bool | The visibility state from data-model › OverlayVisibility, flipped by `toggleOverlay()`. |
| `overlayVisible` | bool | Read-only from QML: `overlayShown || previewing`. The overlay windows bind `visible` to this. |

New invokables:

| Invokable | Effect |
|-----------|--------|
| `toggleOverlay()` | Flips `overlayShown`. |
| `setPreviewing(on: bool)` | Called by the settings window when it opens or closes. While on, the overlay is shown even if `overlayShown` is false. |
| `addNotice(text: QString)` | Appends a notice. QML uses it for shortcut registration failures. |

## SettingsState *(new)*

Holds the effective settings (file ⊕ overrides). It is created once in `Main.qml` and loads
on `Component.onCompleted: load()`.

| Property | Type | Notes |
|----------|------|-------|
| `edge` | QString | `"top"`, `"bottom"`, `"left"` or `"right"` |
| `margin` | i32 | Logical px |
| `opacity` | f64 | 0.10–1.00 (converted from %) |
| `scale` | i32 | Percent, as configured (before capping) |
| `screenName` | QString | Empty = primary |

| Invokable | Effect |
|-----------|--------|
| `load()` | Reads the file and applies the CLI overrides. Warnings go to `OverlayState.addNotice`, or are kept as a property the overlay reads. |
| `setEdge(QString)`, `setMargin(i32)`, `setOpacityPercent(i32)`, `setScalePercent(i32)`, `setScreenName(QString)` | Validate and clamp, drop that field's override, and update the property. Changes are live and not saved yet. |
| `save() -> bool` | Writes the settings file atomically. Returns false (and adds a notice) if the write fails. |
| `placement(screenX, screenY, screenW, screenH, baseW, baseH) -> QString` | Calls the `settings` crate's placement function and returns JSON `{x, y, width, height, scale, capped}` for QML to parse. Used to size the window in both variants, and to position it on X11. |

## GlobalShortcut *(new, backed by the C++ KGlobalAccel shim)*

| Property | Type | Notes |
|----------|------|-------|
| `shortcutId` | QString | `toggle-overlay` / `open-settings` |
| `text` | QString | Label in System Settings › Shortcuts |
| `defaultKeys` | QString | `QKeySequence` portable text, e.g. `Meta+Alt+K` |
| `registered` | bool | Read-only from QML |

| Member | Kind | Notes |
|--------|------|-------|
| `register()` | invokable | Called once from `Component.onCompleted`. Sets `registered`. |
| `activate()` | invokable | Called by the C++ shim when the shortcut fires. Emits `activated`. |
| `activated()` | signal | QML connects it to `overlay.toggleOverlay()` or to opening the settings window. |

**C++ shim** (`crates/app/cpp/global_shortcut.{h,cpp}`):

```
bool kog_register_global_shortcut(QObject &receiver, const QString &id,
                                  const QString &text, const QString &defaultKeys);
```

The shim creates a `QAction` parented to `receiver` and sets its component properties. It
calls `KGlobalAccel::setGlobalShortcut` and connects `triggered` to
`QMetaObject::invokeMethod(&receiver, "activate")`. It returns the registration result. It
never inspects key events (FR-018).

## QML files

| File | Role |
|------|------|
| `qml/Main.qml` | Root `Item` (a plain `QtObject` can't hold child objects). It creates `OverlayState`, `SettingsState`, both `GlobalShortcut`s and the settings window, and a `Loader` that picks the overlay variant with `setSource(url, { overlay, settings })`, so the windows' `required` properties are set when they are created. |
| `qml/Board.qml` | The header and keys, extracted from today's `Main.qml`. It reports its base (100%) size. |
| `qml/LayerShellOverlay.qml` | `Window` with layer-shell attached properties (research R1, R2). |
| `qml/FallbackOverlay.qml` | `Window` with bypass / top / transparent-for-input flags and absolute placement (research R3). |
| `qml/SettingsWindow.qml` | A normal `Window` with Qt Quick Controls bound to `SettingsState`. It saves on commit and on close. |
| `qml/KeyCap.qml` | Unchanged. |
