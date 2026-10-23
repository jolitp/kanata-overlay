---
description: "Task list for 001-overlay-window"
---

# Tasks: Overlay Window

**Input**: Design documents from `/specs/001-overlay-window/`

**Prerequisites**: [plan.md](plan.md), [spec.md](spec.md), [research.md](research.md), [data-model.md](data-model.md), [contracts/](contracts/), [quickstart.md](quickstart.md)

**Tests**: Unit tests are included for the Qt-free `settings` crate, as the plan and constitution V require. Compositor, focus and shortcut behaviour can't be unit-tested; they are checked manually with [quickstart.md](quickstart.md) at each story checkpoint.

**Organization**: Tasks are grouped by user story, so each story can be built and tested on its own.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependency on unfinished tasks).
- **[Story]**: The user story the task belongs to (US1–US4 from spec.md).
- Paths are relative to the repository root.

## Conventions for every task

- Run all commands inside `nix develop`. `just check` must pass before each commit.
- QML only binds and calls; logic goes in Rust (constitution I). Bridge blocks use `#[auto_cxx_name]`.
- Never log or persist key-press data (constitution VI).

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Dependencies, the new crate skeleton and the build wiring.

- [X] T001 Add `kdePackages.kglobalaccel` to the dev shell in `flake.nix`, next to `pkgs.libGL` in `buildInputs`, with a one-line comment: "KGlobalAccel: global shortcuts (research R5); as a build input its headers/lib land on the cc-wrapper paths". Run `nix develop -c true` to confirm the shell still evaluates.
- [X] T002 Add the workspace dependencies to the root `Cargo.toml` `[workspace.dependencies]`: `settings = { path = "crates/settings" }` and `wayland-client = "0.31"`. Add a comment on `wayland-client`: "layer-shell probe (research R4)".
- [X] T003 [P] Create the crate skeleton `crates/settings/Cargo.toml`: name `settings`, description "Overlay settings: file, CLI overrides, placement maths (no Qt)", workspace package fields, deps `serde` and `serde_json` (workspace), and `[lints] workspace = true`. Create `crates/settings/src/lib.rs` with a module doc and empty `mod file; mod cli; mod placement;` plus the matching empty files `crates/settings/src/file.rs`, `crates/settings/src/cli.rs` and `crates/settings/src/placement.rs`.
- [X] T004 Add `settings.workspace = true` and `wayland-client.workspace = true` to `crates/app/Cargo.toml` `[dependencies]`.
- [X] T005 Update `crates/app/build.rs`:
  - add `.cpp_file("cpp/global_shortcut.cpp")` to the `CxxQtBuilder`;
  - add `"src/settings_state.rs"` and `"src/global_shortcut.rs"` to `.files([...])`;
  - add the QML files `qml/Board.qml`, `qml/LayerShellOverlay.qml`, `qml/FallbackOverlay.qml` and `qml/SettingsWindow.qml` to `.qml_files([...])`;
  - add `.qt_module("QuickControls2")` if Controls needs it at link time;
  - emit `println!("cargo:rustc-link-lib=KF6GlobalAccel")`.

  This task can be finished incrementally as the files appear; create empty placeholders now so the build keeps passing.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: The window and bridge skeleton every story builds on: the extracted board, the root QML object, the overlay state, the core settings types and placement maths, the platform probe, and the global-shortcut plumbing.

**⚠️ CRITICAL**: No user-story work can begin until this phase is complete.

### Settings crate core (pure Rust)

- [X] T006 [P] Define the core types in `crates/settings/src/lib.rs`:
  - `enum Edge { Top, Bottom, Left, Right }`, with serde lowercase names (`"top"`, `"bottom"`, `"left"`, `"right"`), `FromStr`/`Display` using the same names, and default `Bottom`.
  - `struct Settings { version: u32, edge: Edge, margin: u32, opacity: u8, scale: u16, screen: Option<String> }`, with `Default` = version `1`, edge `bottom`, margin `16`, opacity `85`, scale `100`, screen `null`.
  - Constants for the ranges, quoted from data-model.md: "margin 0 – 500" (logical px), "opacity 10 – 100" (%), "scale 50 – 300" (%).
  - `struct Rect { x: i32, y: i32, width: i32, height: i32 }` and `struct Size { width: i32, height: i32 }`.
- [X] T007 [P] Implement `pub fn place(screen: Rect, base: Size, edge: Edge, margin: u32, scale_percent: u16) -> Placement` in `crates/settings/src/placement.rs`. `Placement { x, y, size: Size, scale: f64, capped: bool }`. Rules from data-model.md › Placement:
  - `scale = scale%/100`, reduced to the largest value that fits `screen − 2·margin` along both axes; `capped = true` if it was reduced;
  - `size = base × scale` rounded;
  - `x`/`y` centred along the edge, `margin` from it, and fully inside the screen (honour `screen.x`/`screen.y` for multi-monitor).

  Re-export it from `lib.rs`.
- [X] T008 [P] Write unit tests in `crates/settings/src/placement.rs` (`#[cfg(test)]`):
  - each of the four edges on a 1920×1080 screen at offset (1920, 0);
  - margin 0;
  - scale 300 capped on a small screen (`capped == true`, fits);
  - scale 50;
  - a base larger than the screen at 100%.

### App bridge and QML skeleton

- [X] T009 [P] Create `crates/app/src/platform.rs`:
  - `pub enum Platform { Wayland, X11, Other(String) }` and `pub struct Capabilities { pub platform: Platform, pub layer_shell: bool }`;
  - `pub fn detect(qt_platform_name: &str) -> Capabilities`. For `"wayland"`, it opens a `wayland_client::Connection::connect_to_env()`, does one registry roundtrip and checks for the global `zwlr_layer_shell_v1`. Any error means `layer_shell = false`, logged at debug level;
  - `pub fn missing_features(&Capabilities) -> Vec<&'static str>`. Wayland with layer-shell → empty. X11 → `["always-on-top is best-effort on X11"]`. Wayland without layer-shell → `["not always on top", "not on all desktops", "no edge placement"]`, which matches data-model.md › Capabilities.
- [X] T010 Extend `OverlayState` in `crates/app/src/overlay_state.rs`, following contracts/qml-bridge.md:
  - add `#[qproperty(bool, use_layer_shell)]`, `#[qproperty(QString, notices)]` and `#[qproperty(bool, overlay_shown)]` (default `true`);
  - add `#[qinvokable] fn toggle_overlay`, `#[qinvokable] fn set_previewing(on: bool)` and `#[qinvokable] fn add_notice(text: &QString)`. `add_notice` appends with the separator `" · "`, skips duplicates, and logs once with `tracing::info!`;
  - add a computed `#[qproperty(bool, overlay_visible)]` = `overlay_shown || previewing`, updated by both invokables.

  In `start()`, call `platform::detect(...)` with Qt's platform name (`QGuiApplication::platformName()` via cxx-qt-lib), set `use_layer_shell`, and `add_notice` each entry from `missing_features`. The Kanata connection thread is unchanged (FR-016).
- [X] T011 Set the app identity in `crates/app/src/main.rs`: before creating the engine, `QCoreApplication::setApplicationName("kanata-overlay")` and `QGuiApplication::setDesktopFileName("io.github.jolitp.kanataoverlay")`, via cxx-qt-lib (research R5). Add `mod platform; mod settings_state; mod global_shortcut;`.
- [X] T012 [P] Extract the board into `crates/app/qml/Board.qml`. It is an `Item` holding today's header `Row` (connection dot, layer name, status text) and the `Repeater` of `KeyCap`s, moved from `crates/app/qml/Main.qml`. Properties: `required property var overlay` (the OverlayState) and `property real unit: 54`. Expose `readonly property size baseSize` (the 100%-scale width and height computed from the existing bounds logic). The status `Text` shows `overlay.status` plus `overlay.notices` when the latter is non-empty. Keep the colours unchanged. Board draws its own background, a `Rectangle` filling the Board with `color: "#1e1f22"`, behind the header and keys. This replaces today's `Window { color: … }`, because the overlay windows become transparent, and it lets FR-008 opacity apply to the background too.
- [X] T013 Create the `SettingsState` QObject skeleton in `crates/app/src/settings_state.rs`, following contracts/qml-bridge.md:
  - properties `edge: QString` (default `"bottom"`), `margin: i32` (16), `opacity: f64` (0.85), `scale: i32` (100) and `screen_name: QString` (empty), filled from `settings::Settings::default()`;
  - the invokable `placement(screen_x, screen_y, screen_w, screen_h, base_w, base_h: f64) -> QString`, which calls `settings::place` and returns JSON `{x, y, width, height, scale, capped}`.

  Mark it `#[qml_element]`. `load`, the setters and `save` come in US2.
- [X] T014 [P] Create the KGlobalAccel shim `crates/app/cpp/global_shortcut.h` and `crates/app/cpp/global_shortcut.cpp` with `bool kog_register_global_shortcut(QObject &receiver, const QString &id, const QString &text, const QString &defaultKeys)` (research R5, contracts/qml-bridge.md). It:
  - creates `new QAction(&receiver)`;
  - sets `setObjectName(id)` and `setText(text)`, plus the properties `componentName = "kanata-overlay"` and `componentDisplayName = "Kanata Overlay"`;
  - calls `KGlobalAccel::setGlobalShortcut(action, QKeySequence(defaultKeys, QKeySequence::PortableText))`;
  - connects `QAction::triggered` to a lambda that calls `QMetaObject::invokeMethod(&receiver, "activate", Qt::QueuedConnection)`;
  - returns the registration result.

  Include via `<KF6/KGlobalAccel/KGlobalAccel>`. It must not inspect any key events (FR-018).
- [X] T015 Create the `GlobalShortcut` QObject in `crates/app/src/global_shortcut.rs`:
  - properties `shortcut_id`, `text`, `default_keys` (QString) and `registered` (bool);
  - `#[qinvokable] fn register(self: Pin<&mut Self>)`, which calls the shim through an `unsafe extern "C++"` block (`include!("global_shortcut.h")`), passing `self.upcast_pin()` as `QObject`, and sets `registered`;
  - `#[qinvokable] fn activate(self: Pin<&mut Self>)`, which emits `#[qsignal] fn activated`;
  - `#[qml_element]`.

  Depends on T014.
- [X] T016 Rework `crates/app/qml/Main.qml` into a root `Item`, not `QtObject`: plain `QtObject` has no default property for child objects. It holds `OverlayState { id: overlay }` (calls `start()` on completion), `SettingsState { id: settings }` and `Loader { id: overlayLoader }`.
  - Load the variant imperatively, so that the `required` properties are set when the window is created. In `Component.onCompleted`, after `overlay.start()`, call `overlayLoader.setSource(overlay.useLayerShell ? "LayerShellOverlay.qml" : "FallbackOverlay.qml", { overlay: overlay, settings: settings })`.
  - Do **not** use a `source:` binding.
  - Until US1 / US4 exist, a placeholder `LayerShellOverlay.qml` keeps the app runnable: a plain `Window` containing `Board`, with the same two `required` properties.

**Checkpoint**: `just check` passes, and `just run` shows the board as before (now through `Board.qml` and the Loader). The `settings` placement tests pass.

---

## Phase 3: User Story 1 - Overlay that never gets in the way (Priority: P1) 🎯 MVP

**Goal**: On Plasma Wayland the board is a layer-shell surface on the overlay layer. It is above everything, never takes focus, lets all clicks through, shows on every desktop and is absent from the task bar. It uses the default settings.

**Independent Test**: quickstart.md §2. Type and click through the overlay area in a focused editor: every keystroke and click reaches the editor. The overlay stays above maximised and full-screen windows and across desktops, with no task-bar entry.

- [X] T017 [US1] Implement `crates/app/qml/LayerShellOverlay.qml`. It is a `Window` with `import org.kde.layershell 1.0 as LayerShell` and these settings:
  - `required property var overlay` and `required property var settings`;
  - `color: "transparent"`, `visible: false`, and `flags: Qt.FramelessWindowHint | Qt.WindowTransparentForInput | Qt.WindowDoesNotAcceptFocus`;
  - `LayerShell.Window.scope: "kanata-overlay"`, `.layer: LayerShell.Window.LayerOverlay`, `.keyboardInteractivity: LayerShell.Window.KeyboardInteractivityNone`, `.activateOnShow: false` and `.exclusionZone: 0`;
  - `.anchors` = the single anchor for `settings.edge` (`top` → `AnchorTop`, …);
  - `.margins` = `settings.margin` on that edge only.

  It contains `Board { overlay: …; opacity: settings.opacity; scale: placement.scale; transformOrigin: Item.TopLeft }`. `width`/`height` come from `JSON.parse(settings.placement(screen.virtualX, screen.virtualY, screen.width, screen.height, board.baseSize.width, board.baseSize.height))`. Research R1, R2, R6.
- [X] T018 [US1] In `crates/app/qml/LayerShellOverlay.qml`, show the window after the attached properties are set: in `Component.onCompleted`, bind `visible: overlay.overlayVisible` (via `Qt.binding`). Research R1: the properties must be set before the first map.
- [X] T019 [US1] Remove the placeholder from T016 and confirm `crates/app/qml/Main.qml` loads `LayerShellOverlay.qml` on Plasma Wayland. Update the comment at the top of `Main.qml` that says the overlay is "the first Spec Kit feature".
- [X] T020 [US1] Manually validate quickstart.md §2 on Plasma Wayland (SC-001: 100 keystrokes and 20 clicks pass through; SC-002: 10 focus, maximise, full-screen and desktop switches). Record the results in a short note under "Validation log" at the end of `specs/001-overlay-window/quickstart.md`.

**Checkpoint**: The MVP. The overlay works on Plasma Wayland with default placement.

---

## Phase 4: User Story 2 - Place, fade and size the overlay (Priority: P2)

**Goal**: Edge, margin, opacity, scale and monitor are configurable in a settings window (with live preview), saved to a versioned JSON file, and can be overridden from the command line.

**Independent Test**: quickstart.md §3. Open the settings window (Meta+Alt+Shift+K), set top / 50 / 150, watch the overlay update, restart, and the same settings apply. CLI overrides don't touch the file. Invalid file fields fall back per field.

### Tests for User Story 2

- [X] T021 [P] [US2] Write unit tests in `crates/settings/src/file.rs` (`#[cfg(test)]`, using a unique dir under `std::env::temp_dir()` with no new crates):
  - a missing file → defaults, writable, no warnings;
  - a valid v1 file round-trips through save and load;
  - `{"version":1,"edge":"diagonal","opacity":40}` → edge `bottom` + an `InvalidField{field:"edge"}` warning, opacity 40;
  - opacity 5 and opacity 500 → default + warning;
  - `{"version":2}` → defaults + warning + `writable == false`;
  - non-JSON → defaults + warning + not writable;
  - save creates the missing directory;
  - save leaves no `.tmp` file behind;
  - unknown fields are ignored.
- [X] T022 [P] [US2] Write unit tests in `crates/settings/src/cli.rs` (`#[cfg(test)]`):
  - every flag from contracts/cli.md parses;
  - `--screen primary` → an override to `None`;
  - unknown args (e.g. `-platform wayland`, `--connect x`) are ignored;
  - `--scale 9000`, `--opacity abc`, `--edge diagonal` and `--margin` with no value → `CliError` naming the flag;
  - precedence: `effective()` uses an override over the file value;
  - `clear(field)` drops one override and keeps the others.

### Implementation for User Story 2

- [X] T023 [P] [US2] Implement the file handling in `crates/settings/src/file.rs`, following contracts/settings-file.md and data-model.md › LoadOutcome:
  - `pub fn default_path() -> Option<PathBuf>`: `$XDG_CONFIG_HOME/kanata-overlay/settings.json`, else `$HOME/.config/kanata-overlay/settings.json`, ignoring an empty `XDG_CONFIG_HOME`;
  - `pub fn load(path) -> LoadOutcome { settings, warnings: Vec<SettingsWarning>, writable: bool }`. It parses through `serde_json::Value` and validates each field on its own, using the ranges "margin 0 – 500", "opacity 10 – 100", "scale 50 – 300", edge one of `top`/`bottom`/`left`/`right`, and screen a string or null. A missing field gets its default silently. A wrong type or a value out of range gets its default plus `SettingsWarning::InvalidField { field, reason }`. `version` > 1, or a file that is unreadable or not JSON, gives defaults + a warning + `writable: false`;
  - `pub fn save(path, &Settings) -> io::Result<()>`. It creates the parent directory, writes pretty JSON with two-space indentation and a trailing newline to `settings.json.tmp`, then calls `fs::rename`.

  `SettingsWarning` implements `Display` with user-readable text.
- [X] T024 [P] [US2] Implement `Overrides` in `crates/settings/src/cli.rs`, following contracts/cli.md and data-model.md › Overrides:
  - `pub struct Overrides { edge: Option<Edge>, margin: Option<u32>, opacity: Option<u8>, scale: Option<u16>, screen: Option<Option<String>>, settings_path: Option<PathBuf> }`;
  - `pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Overrides, CliError>`. It accepts `--settings`, `--edge`, `--margin`, `--opacity`, `--scale` and `--screen`, with the same ranges as T023, and ignores any other argument;
  - `CliError` implements `Display` as `invalid --FLAG value 'X': <reason>`;
  - `pub fn effective(&self, &Settings) -> Settings` and `pub fn clear(&mut self, field: Field)`.
- [X] T025 [US2] In `crates/app/src/main.rs`, call `settings::Overrides::parse(std::env::args().skip(1))` first. On `Err(e)`, print `kanata-overlay: {e}` to stderr and `std::process::exit(2)` before creating the `QGuiApplication` (contracts/cli.md). Pass the overrides to `SettingsState`. Simplest option: store them in a `OnceLock` in `settings_state.rs`, like `Args::from_env()` today. Keep the existing `--connect`/`--layout` parsing working.
- [X] T026 [US2] Complete `SettingsState` in `crates/app/src/settings_state.rs`, following contracts/qml-bridge.md:
  - `#[qinvokable] load()`. It resolves the path (`--settings` or `default_path()`), calls `settings::load`, stores `file_settings`, `writable` and `overrides`, publishes `overrides.effective(&file_settings)` to the properties, and exposes the warnings through a `#[qproperty(QString, warnings)]` that QML forwards to `overlay.addNotice`;
  - the invokables `set_edge(QString)`, `set_margin(i32)`, `set_opacity_percent(i32)`, `set_scale_percent(i32)` and `set_screen_name(QString)`. Each clamps to the data-model ranges, updates `file_settings`, calls `overrides.clear(field)` and updates the property (live, not saved);
  - `#[qinvokable] save() -> bool`. It calls `settings::save`, sets `writable = true` on success, and on error returns false and adds the warning "could not save settings: …".

  Opacity is exposed as `f64` = percent / 100.
- [X] T027 [P] [US2] Create `crates/app/qml/SettingsWindow.qml`. It is an ordinary `Window` (title "Kanata Overlay Settings", normal flags) using `QtQuick.Controls` and `QtQuick.Layouts`, with `required property var settings` and `required property var overlay`, and these controls:
  - a `ComboBox` for edge (Top, Bottom, Left, Right);
  - a `SpinBox` for margin, 0–500;
  - a `Slider` for opacity, 10–100, with a value label;
  - a `Slider` for scale, 50–300;
  - a `ComboBox` for monitor ("Primary" + `Qt.application.screens[i].name`).

  Each control calls its `settings.set*` on change. Call `settings.save()` when a slider is released (`onPressedChanged: if (!pressed)`), when a ComboBox is activated, and in `onClosing`. Call `overlay.setPreviewing(visible)` in `onVisibleChanged` (data-model › OverlayVisibility).
- [X] T028 [US2] Wire up `crates/app/qml/Main.qml`:
  - call `settings.load()` on completion and forward `settings.warnings` to `overlay.addNotice`;
  - instantiate `SettingsWindow { id: settingsWindow; settings: settings; overlay: overlay; visible: false }`;
  - add `GlobalShortcut { shortcutId: "open-settings"; text: "Open overlay settings"; defaultKeys: "Meta+Alt+Shift+K"; onActivated: { settingsWindow.show(); settingsWindow.raise(); settingsWindow.requestActivate() } }`, calling `register()` on completion. If `registered` is false, call `overlay.addNotice("settings shortcut unavailable — run with --settings or edit " + path)`.
- [X] T029 [US2] Monitor selection in `crates/app/qml/LayerShellOverlay.qml`:
  - resolve `settings.screenName` against `Qt.application.screens` by `.name`;
  - use the primary screen (`Qt.application.screens[0]`) when the name is empty or not found, and call `overlay.addNotice("monitor <name> not found, using primary")` when not found;
  - set `screen:` and `LayerShell.Window.screen:` to it;
  - when the resolved screen changes (including hot-plug through `Qt.application.screensChanged`), set `visible = false` and then rebind visibility, so the surface is re-created on the new output (research R1, R8);
  - the `settings.placement(...)` call must be a binding that depends on `screen.width`, `screen.height`, `screen.virtualX` and `screen.virtualY`, so that a resolution or scale-factor change on the same screen re-sizes and re-anchors the overlay (spec edge case "screen resolution or scaling changes"). Include a resolution change and a display-scale change in the T030 validation.

  Also call `overlay.addNotice` once when `placement.capped` becomes true ("scale capped to fit screen").
- [X] T030 [US2] Manually validate quickstart.md §3 (SC-004) and add the result to the "Validation log" in `specs/001-overlay-window/quickstart.md`.

**Checkpoint**: US1 and US2 work. Settings persist, previews are live, and CLI overrides work.

---

## Phase 5: User Story 3 - Show and hide with a global shortcut (Priority: P2)

**Goal**: Meta+Ctrl+K toggles the overlay from any application. It can be rebound in System Settings, and a failure to register it is reported.

**Independent Test**: quickstart.md §4. With another app focused, press the shortcut: the overlay hides, and the app gets no character. Press it again: the overlay reappears with the current layer. Rebinding works without a restart.

- [X] T031 [US3] In `crates/app/qml/Main.qml`, add `GlobalShortcut { shortcutId: "toggle-overlay"; text: "Show/hide overlay"; defaultKeys: "Meta+Ctrl+K"; onActivated: overlay.toggleOverlay() }` and call `register()` on completion. If `registered` is false, call `overlay.addNotice("show/hide shortcut unavailable")` (FR-014, FR-017).
- [X] T032 [US3] Make sure hiding and showing again keeps the layer-shell configuration, and that showing never activates the window. In `crates/app/qml/LayerShellOverlay.qml` and `crates/app/qml/FallbackOverlay.qml` (once it exists), visibility must come only from `overlay.overlayVisible`. Never call `requestActivate()` on the overlay (FR-002, FR-016).
- [X] T033 [US3] Manually validate quickstart.md §4 (SC-003: under 200 ms; rebinding in System Settings › Shortcuts › Kanata Overlay; preview while hidden) and add the result to the "Validation log" in `specs/001-overlay-window/quickstart.md`.

**Checkpoint**: US1–US3 work.

---

## Phase 6: User Story 4 - Still usable on X11 and other desktops (Priority: P3)

**Goal**: On X11, and on Wayland without layer-shell, a best-effort overlay: frameless, on top, out of the task bar, at the configured edge, and click-through where the session allows. Missing features are reported once.

**Independent Test**: quickstart.md §5. In a Plasma X11 session the overlay is borderless, on top, at the edge, not in the task bar and click-through, and the header notes the best-effort behaviour.

- [X] T034 [US4] Implement `crates/app/qml/FallbackOverlay.qml` (research R3). It is a `Window` with:
  - `required property var overlay` and `required property var settings`;
  - `color: "transparent"` and `flags: Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint | Qt.WindowTransparentForInput | Qt.WindowDoesNotAcceptFocus | Qt.Tool | (Qt.platform.pluginName === "xcb" ? Qt.X11BypassWindowManagerHint : 0)`;
  - the same screen resolution as T029, including the placement binding on the screen geometry, so that resolution and scale changes re-position the window;
  - `x`, `y`, `width` and `height` from `settings.placement(...)` using the screen's `virtualX`/`virtualY`;
  - the same `Board` with opacity and scale as T017;
  - `visible` bound to `overlay.overlayVisible` after `Component.onCompleted`.

  It must not import `org.kde.layershell`.
- [X] T035 [US4] Check in `crates/app/src/overlay_state.rs` that the notices from `platform::missing_features` (T009/T010) are added exactly once at start-up, and that `use_layer_shell` is false on X11 and on Wayland without `zwlr_layer_shell_v1` (FR-013).
- [ ] T036 [US4] **Deferred (2026-10-23): no Plasma X11 session available; only the XWayland smoke test was run, see the quickstart validation log.** Manually validate quickstart.md §5 in a Plasma X11 session (SC-005) and, if available, on a Wayland session without layer-shell. Add the result to the "Validation log" in `specs/001-overlay-window/quickstart.md`. If KWin/X11 doesn't keep the bypass window above full-screen windows, document it in research.md R3 under "Risk".

**Checkpoint**: All four stories work.

---

## Phase 7: Polish & Cross-Cutting Concerns

- [X] T037 [P] Update `README.md`:
  - replace the scaffold note under the screenshot;
  - add a "Using the overlay" section with the shortcuts Meta+Ctrl+K and Meta+Alt+Shift+K (rebindable in System Settings › Shortcuts), the settings file path and the new CLI options from contracts/cli.md;
  - mark Roadmap step 1 as done.
- [X] T038 [P] Update `CLAUDE.md` › Layout with `crates/settings` ("overlay settings file, CLI overrides, placement maths; no Qt") and `crates/app/cpp/` ("thin C++ shims (KGlobalAccel)").
- [X] T039 Run the privacy check in quickstart.md §6 (FR-018, FR-019): the settings file contains only the six documented fields, and `RUST_LOG=debug` output has no key-press data.
- [X] T040 Run `just check` (fmt-check, clippy `-D warnings`, tests) and fix every finding. Then run `just fmt` for the Nix formatting of `flake.nix`.
- [X] T041 Measure start-up time (SC-006). On `main` and on this branch, run `nix develop -c cargo build --release -p kanata-overlay`, then time 5 launches each from start to the first frame (`QSG_INFO=1` or a `tracing::info!` in `Component.onCompleted` of the overlay window). The median increase must be ≤ 0.5 s. Record both medians in the "Validation log" in `specs/001-overlay-window/quickstart.md`. If the layer-shell probe (T009) is the cause, move it to a background thread and push the result with `qt_thread().queue(...)` (constitution I).
- [X] T042 Review `specs/001-overlay-window/spec.md` FR-001 – FR-019 against the implementation and the validation log, and note any requirement left unmet in the "Validation log" in `specs/001-overlay-window/quickstart.md`.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies.
- **Foundational (Phase 2)**: Depends on Setup. It blocks all user stories.
- **US1 (Phase 3)**: Depends on Foundational.
- **US2 (Phase 4)**: Depends on Foundational. T029 edits `LayerShellOverlay.qml` from US1, so in practice do it after T017.
- **US3 (Phase 5)**: Depends on Foundational (the shortcut plumbing T014/T015 and the `toggleOverlay` from T010). It is independent of US2.
- **US4 (Phase 6)**: Depends on Foundational. It reuses the screen-resolution pattern from T029 if US2 is done; otherwise it uses the primary screen.
- **Polish (Phase 7)**: Depends on the stories you want to ship.

### Within phases

- T003 → T006/T007 → T008 (crate before code before tests in the same file).
- T014 → T015 (shim before its Rust wrapper).
- T012, T013 → T016 → T017 (Board and SettingsState before Main and the overlay).
- T023 → T021 and T024 → T022 can also run tests-first (TDD) if preferred.
- T026 → T027 → T028 (bridge before window before wiring).

### Parallel Opportunities

- Setup: T003 runs alongside T001/T002.
- Foundational: T006, T007, T009, T012 and T014 touch separate files and can run in parallel. T008 follows T007.
- US2: T021 and T022 (tests), and T023 and T024 (implementations), are all separate files. T027 (QML) can be written in parallel with T026 against the contract.
- With two people, after Foundational: one takes US1 → US2 and the other US3 → US4.

---

## Parallel Example: Foundational

```bash
Task: "T006 Define Edge/Settings/Rect/Size in crates/settings/src/lib.rs"
Task: "T007 Implement place() in crates/settings/src/placement.rs"
Task: "T009 Create platform probe in crates/app/src/platform.rs"
Task: "T012 Extract Board.qml from Main.qml"
Task: "T014 Write KGlobalAccel shim in crates/app/cpp/global_shortcut.{h,cpp}"
```

## Parallel Example: User Story 2

```bash
Task: "T021 Unit tests for settings file in crates/settings/src/file.rs"
Task: "T022 Unit tests for CLI overrides in crates/settings/src/cli.rs"
Task: "T023 Implement load/save in crates/settings/src/file.rs"
Task: "T024 Implement Overrides::parse in crates/settings/src/cli.rs"
Task: "T027 Build SettingsWindow.qml against contracts/qml-bridge.md"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Phase 1 Setup → Phase 2 Foundational. `just check` stays green, and the app still runs.
2. Phase 3 (US1): the layer-shell overlay with default placement.
3. **Stop and validate** with quickstart.md §2 on Plasma Wayland. Commit.

### Incremental Delivery

1. Add US2 (settings) → quickstart §3 → commit.
2. Add US3 (toggle shortcut) → quickstart §4 → commit.
3. Add US4 (X11 / fallback) → quickstart §5 → commit.
4. Polish → `just check`, docs, privacy check → commit, then open a PR to `main`.

---

## Notes

- [P] tasks touch different files and have no dependency on unfinished tasks.
- Commit after each task or logical group, with `just check` green (constitution workflow).
- The manual validation tasks (T020, T030, T033, T036) are the "manual test notes" constitution V requires for compositor-touching features.
