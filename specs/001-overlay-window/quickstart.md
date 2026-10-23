# Quickstart & Validation: Overlay Window

How to show that the feature works. Contracts: [cli.md](contracts/cli.md),
[settings-file.md](contracts/settings-file.md), [qml-bridge.md](contracts/qml-bridge.md).

## Prerequisites

- `nix develop`, or direnv. The dev shell must include `kdePackages.kglobalaccel` (see the
  plan).
- A KDE Plasma 6 Wayland session for the main checks. A Plasma X11 session for the X11
  checks.
- No Kanata needed: run `just fake-kanata nav` in a second terminal.
- So that your real settings file isn't touched, the checks below use a scratch file:
  `export S=$(mktemp -d)/settings.json`.

## 1. Automated

```sh
just check          # fmt, clippy -D warnings, all tests
cargo test -p settings
```

Expected: everything passes. The `settings` tests cover:
- defaults
- lenient per-field fallback
- future-version and unreadable files (not writable)
- an atomic save round-trip
- override precedence and clearing an override on edit
- CLI parse errors
- placement on all four edges
- scale capping

## 2. Overlay behaviour on Plasma Wayland (US1; SC-001, SC-002)

```sh
just run --settings $S
```

| Step | Expected |
|------|----------|
| Focus a text editor and type 100 characters, some with the cursor over the overlay | Every character appears in the editor. The overlay never takes focus. |
| Click, scroll and drag 20 times over the overlay | The window underneath receives every action. |
| Maximise the editor, then make it full-screen (F11) | The overlay stays visible on top. |
| Switch virtual desktop, then switch activity | The overlay is visible on each one. |
| Open the task manager, Alt+Tab and the overview | There is no entry for the overlay. |
| Maximise a window | The window uses the full screen height. The overlay floats above and doesn't push it. |
| Look at the header | There are no notices (all features available). |

## 3. Placement, opacity, scale (US2; SC-004)

| Step | Expected |
|------|----------|
| Press **Meta+Alt+Shift+K** | The settings window opens as a normal window. The overlay is still click-through. |
| Set edge **Top**, opacity **50**, scale **150**, while watching the overlay | Each change shows on the overlay immediately. |
| Close the settings window, then run `cat $S` | The file holds `"edge": "top"`, `"opacity": 50`, `"scale": 150`. |
| Quit the app and run it again with `--settings $S` | It comes back at the top, half transparent and 1.5× size. |
| Set the scale to 300 on a small screen | The overlay fits on screen, and the header notes that the scale was capped. |
| With two monitors: pick the second monitor | The overlay moves to it. Unplug that monitor: the overlay moves to the primary. Plug it back in: the overlay returns. |
| `just run --settings $S --opacity 100`, then `cat $S` | The overlay is opaque for this run, and the file still says 50. |
| `just run --settings $S --scale 9000` | `invalid --scale value` on stderr, exit status 2, and no window. |
| `echo '{"version":1,"edge":"diagonal","opacity":40}' > $S`, then run | The overlay is at the bottom edge with opacity 40, and the notice mentions `edge`. |
| `echo '{"version":2}' > $S`, then run, change nothing and quit | Defaults are used with a notice, and the file still contains `{"version":2}`. |

## 4. Global shortcut (US3; SC-003)

| Step | Expected |
|------|----------|
| With the editor focused, press **Meta+Ctrl+K** | The overlay disappears at once, and the editor gets no character. |
| Run `just fake-kanata base`, then press **Meta+Ctrl+K** again | The overlay comes back showing layer `base`. |
| Rebind it in System Settings › Shortcuts › Kanata Overlay | The new combination toggles the overlay, without restarting the app. |
| Hide the overlay, then open the settings window | The overlay shows for preview. Closing the window hides it again. |
| Run under a session without kglobalacceld (e.g. a bare X11 window manager) | The app runs, and the header notes that the shortcuts are unavailable. |

## 5. X11 and fallback (US4; SC-005)

In a Plasma X11 session:

| Step | Expected |
|------|----------|
| `just run --settings $S` | No border, on top, at the configured edge, not in the task bar. The current layer is shown. |
| Click and type over the overlay | Input reaches the window underneath. |
| Focus or maximise other windows | The overlay stays above them. |
| Look at the header | The notice says that "always on top" is best-effort on X11. |

On a Wayland session without layer-shell (e.g. GNOME), the app starts as a normal window.
The notice lists the missing features: always on top, all desktops, edge placement.

## 6. Privacy check (FR-018, FR-019)

```sh
RUST_LOG=debug just run --settings $S 2>&1 | tee overlay.log   # type for a while, toggle the overlay
cat $S; less overlay.log; rm overlay.log
```

Expected: the settings file holds only the six documented fields. The log has no key-press
data, only layer changes and connection status.

## Validation log

### 2026-10-23: automated checks and agent smoke test (Plasma 6, Wayland, single 1920×1080 `eDP-1`)

| Check | Result |
|-------|--------|
| `just check` (fmt, clippy `-D warnings`, 30 tests incl. 19 `settings` tests) | ✅ pass |
| Start-up on Plasma Wayland | ✅ `Capabilities { platform: Wayland, layer_shell: true }`, no QML warnings |
| Overlay drawn bottom-centre at 85% opacity above a full-screen app, clear of the panel (screenshot) | ✅ |
| Both shortcuts registered with KGlobalAccel (`qdbus org.kde.kglobalaccel /component/kanata_overlay … shortcutNames` → `open-settings`, `toggle-overlay`) | ✅ |
| `invokeShortcut toggle-overlay` twice → hidden, then shown; `open-settings` → preview on | ✅ (debug log) |
| Opening and closing settings without edits leaves no settings file (nothing is saved without a change) | ✅ |
| X11 through XWayland (`QT_QPA_PLATFORM=xcb`) → fallback window, notice "always-on-top is best-effort on X11", no QML warnings | ✅ |
| `--scale 9000` / `--edge` with no value → message on stderr, exit 2 | ✅ |
| Privacy (FR-018, FR-019): logs contain platform, notices and visibility only; the settings file holds the six documented fields | ✅ |

**Still to do by hand** (they need a person at the keyboard):
- §2: typing and clicking *through* the overlay (SC-001), and staying on top through maximise, full-screen and desktop switches (SC-002) (T020).
- §3: the settings window controls, persistence across a restart, a second monitor and hot-plug, and resolution or scale changes (T030).
- §4: pressing the real key combinations, the < 200 ms response, and rebinding in System Settings (T033).
- §5: a real Plasma X11 session (T036).
- SC-006 start-up comparison against `main` (T041).

**Implementation deviations** (recorded in contracts/qml-bridge.md and research.md R1):
- `Main.qml`'s root is a `QtObject` with an explicit object list. The overlay window is created with `createObject`, not a `Loader`: since Qt 6.7 a `Window` inside an `Item` becomes a child window.
- The margin is transparent padding, because `QMargins` can't be built in QML.
- `LayerShell.Window.screen` is not set, because of a type mismatch; the window's own `screen` is used.
- The invokables are named `registerShortcut` and `update*` to avoid a C++ keyword and cxx-qt's generated setters.

**Requirement review (T042)**: FR-001 – FR-019 are all implemented. FR-001 – FR-006, FR-010, FR-015 and FR-012 on real X11 are implemented but not yet confirmed by hand (see above).

### 2026-10-23: manual check by the user, §2 (T020), Plasma Wayland

| Check | Result |
|-------|--------|
| Overlay stays on top of a YouTube video playing in Firefox | ✅ |
| Typing in a VS Code text box with the overlay open on top of it: the text reaches the editor | ✅ (SC-001, keyboard part) |
| Clicks through the overlay reach the window underneath (SC-001, FR-003) | ✅ |
| Stays on top of a full-screen YouTube video (FR-001) | ✅ |
| Visible on another virtual desktop (FR-004) | ✅ |
| No entry in the task bar or Alt+Tab (FR-005) | ✅ |
| A maximised window keeps its full height, with the overlay floating above it (FR-006) | ✅ |

US1 / the MVP is confirmed on Plasma Wayland. Not checked: switching activities (FR-004), and counting exactly 100 keystrokes and 20 clicks (SC-001); normal use passed through without problems.

### 2026-10-23: shortcut conflict found by the user (§4, T033)

- Meta+Alt+K did nothing. Plasma's own "Switch to Next Keyboard Layout" already uses it, so KGlobalAccel registered the action with **no key** (`toggle-overlay=,Meta+Alt+K` in `kglobalshortcutsrc`). The app still reported it as registered, so FR-017 wasn't met.
- Fix:
  - The toggle default is now **Meta+Ctrl+K**, which is free on a stock Plasma 6.
  - The shim asks the daemon (`globalShortcut()`) and counts an action whose only binding is an empty sequence as unavailable. The header then shows "show/hide shortcut unavailable — set one in System Settings › Shortcuts" (verified).
- A stored empty binding is kept by KGlobalAccel, so existing installs have to bind the key once in System Settings.
- The user then bound the action in System Settings › Shortcuts › Kanata Overlay, and the toggle worked without restarting the app (FR-014, FR-015 ✅). Meta+Alt+Shift+K opens the settings window (FR-011a ✅). Response time was not measured, but showed no noticeable delay in use (SC-003).

### 2026-10-23: scale slider had no live effect (§3, T030)

- Reported by the user: moving the scale slider did nothing. The window size came from `settings.placement(...)`, a Rust invokable, and QML can't track properties read inside an invokable, so the binding never re-ran (scale only applied after a restart).
- Fix: the `place` bindings in `LayerShellOverlay.qml` and `FallbackOverlay.qml` read `settings.edge`, `settings.margin` and `settings.scale` explicitly. Verified with a temporary timer: `updateScalePercent(150)` resized the window from 748×349 to 1122×516 immediately.

### 2026-10-23: settings persistence and corner positions (§3, T030)

- The user confirmed that settings persist across restarts (FR-011 ✅) and that the scale slider works after the fix above.
- Requested by the user: corner positions. `edge` now also takes `top-left`, `top-right`, `bottom-left` and `bottom-right`. A corner gets two layer-shell anchors, and the margin applies to both edges it touches. The setting keeps its name, so existing files stay valid. In the settings window the field is labelled "Position". Unit test `places_in_every_corner`; a screenshot confirmed bottom-right at 70% scale, clear of the panel.

### 2026-10-23: X11 check deferred (§5, T036)

- The user has no machine with a Plasma X11 session, so §5 can't be run as written.
- What we have is the XWayland smoke test above: `QT_QPA_PLATFORM=xcb` picks `FallbackOverlay.qml`, loads with no QML warnings, and shows the "always-on-top is best-effort on X11" notice.
- Not verified: override-redirect stacking above other and full-screen windows, XShape click-through, and no task-bar entry under a real X11 window manager. These need a real X11 window manager; under XWayland, KWin's Wayland side does the stacking.
- If anyone reports X11 problems, a nested `Xephyr` with an X11 window manager is the cheapest way to reproduce them.

### 2026-10-23: T030 closed

- The user confirmed that the corner positions work in the settings window and persist across restarts. Together with the earlier checks (edge, margin, opacity and scale live, persistence), §3 passes (SC-004 ✅).
- Not checked: a second monitor and hot-plug (the user has a single screen), and resolution or scale-factor changes. The unit tests and the screen-geometry binding cover them.

### 2026-10-23: start-up time (SC-006, T041)

- Release build, Plasma Wayland. The time from process launch to the overlay window becoming visible was measured with a temporary `console.warn(Date.now())` in `onVisibleChanged`, then removed. Five runs: 318, 318, 311, 325 and 321 ms (median **318 ms**).
- Since the whole start-up, including the layer-shell probe and both KGlobalAccel registrations, is under 0.5 s, the increase over `main` is too, so building `main` for comparison wasn't needed. SC-006 ✅. The probe stays on the GUI thread (no need to move it).
