# Implementation Plan: Overlay Window

**Branch**: `001-overlay-window` | **Date**: 2026-10-23 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/001-overlay-window/spec.md`

## Summary

This feature turns today's scaffold window into a keyboard overlay that ignores clicks and
never takes focus.

- **Plasma Wayland**: a layer-shell surface on the *overlay* layer. It uses LayerShellQt's QML
  module, with no keyboard interactivity and an empty input region.
- **X11, and Wayland without layer-shell**: a best-effort frameless, override-redirect,
  transparent-for-input window.
- **Placement and appearance**: edge, margin, opacity, scale and monitor live in a new
  Qt-free `settings` crate. It covers the versioned JSON file, lenient validation, CLI
  overrides and the placement / scale-capping maths.
- **Settings window**: a normal Qt Quick Controls window edits those settings with live
  preview.
- **Shortcuts**: two global shortcuts (toggle overlay, open settings) are registered with KDE's
  KGlobalAccel through a ~40-line C++ shim.

## Technical Context

**Language/Version**: Rust stable (edition 2024, MSRV 1.88; dev shell has rustc 1.98.1); QML (Qt 6.11.2); C++17 for one shim

**Primary Dependencies**:
- Existing: cxx-qt / cxx-qt-lib / cxx-qt-build 0.10, serde, serde_json.
- New: LayerShellQt 6.7 QML module (already in the dev shell), Qt Quick Controls (part of
  qtdeclarative, already present), KF6 KGlobalAccel 6.30 (new system library), and the
  `wayland-client` crate (new, for the layer-shell probe).

**Storage**: One JSON file, `$XDG_CONFIG_HOME/kanata-overlay/settings.json` (see [contracts/settings-file.md](contracts/settings-file.md))

**Testing**: `cargo test` for the `settings` crate (pure logic); manual test notes in [quickstart.md](quickstart.md) for compositor and shortcut behaviour (constitution V)

**Target Platform**: Linux. Primary: KDE Plasma 6 Wayland. Secondary: Plasma X11. Other Wayland compositors use a fallback.

**Project Type**: Desktop app (Cargo workspace: library crates + Qt/QML binary)

**Performance Goals**: Show/hide reacts in under 200 ms (SC-003); live preview updates within one frame; start-up grows by no more than 0.5 s (SC-006)

**Constraints**:
- The overlay must never take focus or input.
- No root at runtime.
- No key-press data in settings or logs.
- `just check` must pass.

**Scale/Scope**:
- 1 new library crate (~400 lines + tests).
- 1 new QObject type plus extensions to an existing one.
- 1 C++ shim.
- 4 new QML files, with `Main.qml` reworked.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | How |
|-----------|--------|-----|
| I. Rust owns state, QML renders | ✅ | Settings, validation, overrides, placement and scale capping live in `crates/settings`. Visibility and notices live in `OverlayState`. QML binds properties and calls invokables. The C++ shim only forwards "shortcut triggered" to a Rust QObject. Nothing in this feature uses background threads. |
| II. Stable Kanata interfaces | ✅ n/a | The Kanata connection is unchanged. |
| III. Never strand the keyboard | ✅ | The feature starts no processes. The overlay can't grab input: it has an empty input region and no keyboard interactivity. Global shortcuts go through the compositor and never grab the keyboard. |
| IV. Plasma Wayland first | ✅ | Primary path: layer-shell via LayerShellQt. X11 is best-effort through window hints, as the constitution describes. |
| V. Testable without hardware | ✅ | The `settings` crate is pure and fully unit-tested, with a temp-dir round-trip. Compositor and shortcut behaviour have manual test notes ([quickstart.md](quickstart.md) §2–5). |
| VI. Key events are private | ✅ | KGlobalAccel delivers only "triggered" for our two actions. The settings file holds six layout fields only (FR-018, FR-019). Logging adds no key data. |
| VII. Simple first | ⚠️ justified | Two new dependencies and one C++ shim. See Complexity Tracking. Everything else uses std plus existing crates (no `dirs`, no `clap`, no async). |

**Gate result**: PASS. The VII items are justified below.

**Post-design re-check** (after Phase 1): still PASS. The contracts keep all logic in Rust.
The only C++ is the KGlobalAccel shim, which the constitution's "C++ only for thin shims"
allows. The LayerShellQt QML module removed the expected LayerShellQt C++ shim.

## Project Structure

### Documentation (this feature)

```text
specs/001-overlay-window/
├── spec.md
├── plan.md              # this file
├── research.md          # Phase 0
├── data-model.md        # Phase 1
├── quickstart.md        # Phase 1 (validation + manual test notes)
├── contracts/
│   ├── settings-file.md
│   ├── cli.md
│   └── qml-bridge.md
├── checklists/requirements.md
└── tasks.md             # Phase 2 (/speckit-tasks, not created here)
```

### Source Code (repository root)

```text
crates/
├── settings/                    # NEW, Qt-free
│   ├── Cargo.toml               # deps: serde, serde_json (workspace)
│   └── src/
│       ├── lib.rs               # Edge, Settings, Overrides, LoadOutcome, warnings
│       ├── file.rs              # path resolution, lenient load, atomic save
│       ├── cli.rs               # Overrides::parse(args) -> Result<_, CliError>
│       └── placement.rs         # place(screen, base, edge, margin, scale) -> Placement
├── app/
│   ├── build.rs                 # + cpp_file("cpp/global_shortcut.cpp"), link KF6GlobalAccel, new QML files
│   ├── cpp/
│   │   ├── global_shortcut.h    # NEW: kog_register_global_shortcut(...)
│   │   └── global_shortcut.cpp  # NEW
│   ├── src/
│   │   ├── main.rs              # app name / desktop file name; Overrides::parse → exit 2 on error
│   │   ├── overlay_state.rs     # + useLayerShell, notices, overlayShown, toggle/preview
│   │   ├── settings_state.rs    # NEW: SettingsState QObject over the settings crate
│   │   ├── global_shortcut.rs   # NEW: GlobalShortcut QObject + cxx bridge to the shim
│   │   └── platform.rs          # NEW: Capabilities + wayland-client layer-shell probe
│   └── qml/
│       ├── Main.qml             # reworked: root Item, Loader.setSource picks overlay variant
│       ├── Board.qml            # NEW: header + keys, extracted from Main.qml
│       ├── LayerShellOverlay.qml# NEW
│       ├── FallbackOverlay.qml  # NEW
│       ├── SettingsWindow.qml   # NEW
│       └── KeyCap.qml           # unchanged
├── kanata-client/               # unchanged
└── layout/                      # unchanged

flake.nix                        # + kdePackages.kglobalaccel in buildInputs
Cargo.toml                       # + settings and wayland-client in workspace.dependencies
README.md                        # roadmap step 1 done; shortcuts and settings documented
```

**Structure Decision**: Follow the existing workspace pattern: logic goes in a new Qt-free
library crate (`crates/settings`), and the `app` crate holds only the bridge, the shim and
QML. Platform probing stays in `app/src/platform.rs`, because it needs a live session and
can't be unit-tested anyway. Its output (`Capabilities`) is a plain struct.

## Implementation Notes

- **Order**: Build the stories in priority order.
  1. US1: the layer-shell variant with fixed defaults and no settings. This proves
     click-through on Plasma.
  2. The `settings` crate with tests.
  3. US2: the settings bridge, CLI options and settings window.
  4. US3: the shortcut shim.
  5. US4: the fallback window and notices.
- **Layer-shell initialisation**: Create the window with `visible: false` and show it
  after `Component.onCompleted`, so the attached properties apply before the surface is
  mapped. When the screen changes, hide the window and show it again (research R1, R8).
- **Board size**: `Board.qml` reports its 100% size. `SettingsState.placement(...)` turns
  that into the window size and effective scale, and on X11 into x/y.
- **Keep the scaffold behaviour**: The header with the connection dot, layer name and
  status stays as it is. Notices are added to the status text.
- **README / CLAUDE.md**: Document the shortcuts, the settings path and the new CLI options,
  and mark roadmap step 1 as done.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| New system dependency: KF6 KGlobalAccel, plus a C++ shim (`cpp/global_shortcut.*`) | Global shortcuts on Plasma, Wayland and X11, rebindable in System Settings (FR-014, FR-015). A Wayland client can't grab global keys itself. | XDG GlobalShortcuts portal (`ashpd`): it is async (conflicts with VII), needs an app id or registration, and asks for a confirmation dialog. kglobalacceld over raw D-Bus with `zbus`: the API is semi-internal and zbus is a bigger dependency than a small shim. |
| New crate dependency: `wayland-client` | Decides layer-shell vs fallback and fills the FR-013 notice reliably (research R4). | Guessing from `XDG_CURRENT_DESKTOP`: wrong for sway, Hyprland and nested sessions. Qt has no API that lists Wayland globals. |
| New workspace crate: `crates/settings` | Constitution I and V: settings logic must be testable without Qt. | Putting the logic in `app`: its tests would need Qt linked and a display, and it would break the "logic in library crates" convention. |
