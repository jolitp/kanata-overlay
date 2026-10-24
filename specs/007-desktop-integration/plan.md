# Implementation Plan: Desktop Integration

**Branch**: `007-desktop-integration` (stacked on `006-live-key-highlighting`) | **Date**: 2026-10-24 | **Spec**: [spec.md](spec.md)

## Summary

Each device gets a tray icon (StatusNotifierItem via `ksni`) showing a short layer label
drawn with Qt into its pixmap, a tooltip, and a shared menu (show/hide, settings,
restart, quit). Devices can use a compact board: only the header pill. The app installs
its own desktop entry and icon in the user's home and can add a login entry. A Unix
socket keeps it to one instance per session. The settings window checks Kanata's
permissions and shows the fix for this system, never applying it.

## Technical Context

**Language/Version**: Rust stable (edition 2024); QML (Qt 6.11)

**Primary Dependencies**: **new**: `ksni` 0.3 (`blocking` feature, default features off; brings `zbus`), research R1. Otherwise std and existing crates. The flake gains `packages.default` (wrapped binary, research R9).

**Storage**: settings v2 gains per-device `style`; desktop, icon and login entries in the user's home (no new config).

**Testing**:
- `cargo test`: short labels, tooltip/menu model, `style` setting, entry file contents, single-instance protocol, permission report;
- manual: quickstart §2 (agent, with scratch `HOME`/`XDG_*` and the fake Kanata), §3 (user).

**Target Platform**: Linux, KDE Plasma (StatusNotifierItem tray) first; other SNI trays work; without a tray, the app runs as before.

**Performance Goals**: a layer change shows in the tray within 1 s (FR-002); icons are redrawn only when their label or state changes.

**Constraints**: no root, no administrator prompts (FR-012, constitution III); no Qt Widgets (the app stays a QML `QGuiApplication`).

**Scale/Scope**:
- app: `tray_state.rs` (about 250 lines), `desktop_state.rs` with single instance, entries and permissions bridge (about 250);
- kanata-client: `permissions.rs` (about 150 plus tests);
- settings: `style` (about 40);
- QML: Board, DeviceView, SettingsWindow, Main changes;
- one SVG icon.

## Constitution Check

| Principle | Status | How |
|-----------|--------|-----|
| I. Rust owns state, QML renders | ✅ | Tray entries, labels, icon drawing, entries, permission report and single instance are Rust. QML forwards device state and connects signals to existing actions. |
| II. Stable Kanata interfaces | ✅ | Nothing new talks to Kanata. |
| III. Never strand the keyboard | ✅ | Quit from the tray goes through the existing quit path (managed Kanatas stopped). No root; the permission fix is only shown. |
| IV. Plasma Wayland first | ✅ | StatusNotifierItem is Plasma's tray protocol; XDG autostart and desktop entries are what Plasma reads. |
| V. Testable without hardware | ✅ | Pure helpers tested; the tray and entries checked with scratch homes and the fake Kanata. |
| VI. Key events are private | ✅ | Compact boards read no presses; the tray shows layers only. |
| VII. Simple first | ✅ (one new dependency) | `ksni` replaces a widgets-based tray or a hand-written D-Bus implementation; everything else is std. |

**Gate**: PASS. **Post-design**: PASS.

## Project Structure

```text
flake.nix                                   # + packages.default: wrapped binary with Qt paths and kanata (R9)
assets/io.github.jolitp.kanataoverlay.svg   # NEW: app icon
crates/kanata-client/src/permissions.rs     # NEW: gather facts, pure report
crates/settings/src/                        # + Style on Device, file parse/save
crates/app/
├── Cargo.toml                 # + ksni (blocking)
├── build.rs                   # + tray_state.rs, desktop_state.rs
├── src/main.rs                # single-instance check, entries, before QML
├── src/tray_state.rs          # NEW: TrayState, icon drawing, ksni trays
├── src/desktop_state.rs       # NEW: DesktopState: login entry, permissions, instance listener
├── src/single_instance.rs     # NEW: socket, protocol (pure parse)
├── src/desktop_entry.rs       # NEW: entry contents (pure) and install
├── src/settings_state.rs      # + "style"
└── qml/
    ├── Board.qml              # compact
    ├── DeviceView.qml         # compact, watched
    ├── SettingsWindow.qml     # Board style, Start at login, Permissions
    └── Main.qml               # TrayState wiring, DesktopState
```

## Implementation Notes

- **Order**:
  1. Foundational: `style` setting; `ksni` dependency; pure helpers with tests (labels, entries, protocol, permission report).
  2. US1: TrayState with icons, tooltips, menu, signals; QML wiring.
  3. US2: compact board.
  4. US3: single instance, desktop entry and icon, Start at login.
  5. US4: permission check in settings.
  6. Polish: README, CLAUDE.md, quickstart log.
- **MVP**: US1 + US2 (both P1).
- **Icon drawing** happens on the Qt thread (QPainter needs it); the pixels go to the tray thread.

## Complexity Tracking

| Addition | Why needed | Simpler alternative rejected because |
|----------|------------|--------------------------------------|
| `ksni` (+ `zbus`) dependency | A StatusNotifierItem with menus without Qt Widgets | Qt's and KDE's trays need `QApplication` and `QMenu` (widgets); writing SNI + DBusMenu by hand is far more code |
