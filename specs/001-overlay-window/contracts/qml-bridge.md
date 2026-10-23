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
| `edge` | QString | `"top"`, `"bottom"`, `"left"`, `"right"`, `"top-left"`, `"top-right"`, `"bottom-left"` or `"bottom-right"` |
| `margin` | i32 | Logical px |
| `opacity` | f64 | 0.10–1.00 (converted from %) |
| `scale` | i32 | Percent, as configured (before capping) |
| `screenName` | QString | Empty = primary |
| `path` | QString | The settings file in use, for messages |

| Invokable | Effect |
|-----------|--------|
| `load()` | Reads the file and applies the CLI overrides. Each warning is emitted as the `warning(text)` signal, which `Main.qml` forwards to `OverlayState.addNotice`. |
| `updateEdge(QString)`, `updateMargin(i32)`, `updateOpacityPercent(i32)`, `updateScalePercent(i32)`, `updateScreenName(QString)` | Validate and clamp, drop that field's override, and update the property. Changes are live and not saved yet. (Named `update*` because cxx-qt already generates `set*` setters for the properties.) |
| `save() -> bool` | Writes the settings file atomically if anything changed since the last save (so an unreadable or newer file is never replaced without an edit). Returns false, and emits `warning`, if the write fails. |
| `warning(text)` | signal | A settings problem for the status line. |
| `placement(screenX, screenY, screenW, screenH, baseW, baseH) -> QString` | Calls the `settings` crate's placement function and returns JSON `{x, y, width, height, scale, capped}` for QML to parse. Used to size the window in both variants, and to position it on X11. |

## GlobalShortcut *(new, backed by the C++ KGlobalAccel shim)*

| Property | Type | Notes |
|----------|------|-------|
| `shortcutId` | QString | `toggle-overlay` / `open-settings` |
| `text` | QString | Label in System Settings › Shortcuts |
| `defaultKeys` | QString | `QKeySequence` portable text, e.g. `Meta+Ctrl+K` |
| `registered` | bool | Read-only from QML |

| Member | Kind | Notes |
|--------|------|-------|
| `registerShortcut()` | invokable | Called once from `Component.onCompleted`. Sets `registered`. (Not `register`: that is a reserved word in C++.) |
| `activate()` | invokable | Called by the C++ shim when the shortcut fires. Emits `activated`. |
| `activated()` | signal | QML connects it to `overlay.toggleOverlay()` or to opening the settings window. |

**C++ shim** (`crates/app/cpp/global_shortcut.{h,cpp}`):

```
bool kog_register_global_shortcut(QObject &receiver, const QString &id,
                                  const QString &text, const QString &defaultKeys);
```

`crates/app/cpp/qt_extras.{h,cpp}` wraps two `QGuiApplication` statics that cxx-qt-lib
doesn't expose: `platformName()` and `setDesktopFileName()`.

The shim creates a `QAction` parented to `receiver` and sets its component properties. It
calls `KGlobalAccel::setGlobalShortcut` and connects `triggered` to
`QMetaObject::invokeMethod(&receiver, "activate")`. It returns the registration result. It
never inspects key events (FR-018).

## QML files

| File | Role |
|------|------|
| `qml/Main.qml` | Root `QtObject` holding its objects in an explicit `property list<QtObject>`. It is not an `Item`, because since Qt 6.7 a `Window` inside an `Item` becomes a child window and never shows. It creates `OverlayState`, `SettingsState`, both `GlobalShortcut`s and the settings window. The overlay variant is created with `Qt.createComponent(url).createObject(root, { overlay, settings })`, so its `required` properties are set when it is created and it stays a top-level window. |
| `qml/Board.qml` | The header and keys, extracted from today's `Main.qml`. It reports its base (100%) size. |
| `qml/LayerShellOverlay.qml` | `Window` with layer-shell attached properties (research R1, R2). |
| `qml/FallbackOverlay.qml` | `Window` with bypass / top / transparent-for-input flags and absolute placement (research R3). |
| `qml/SettingsWindow.qml` | A normal `Window` with Qt Quick Controls bound to `SettingsState`. It saves on commit and on close. |
| `qml/KeyCap.qml` | Unchanged. |
