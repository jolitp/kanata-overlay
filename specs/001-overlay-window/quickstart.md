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
| With the editor focused, press **Meta+Alt+K** | The overlay disappears at once, and the editor gets no character. |
| Run `just fake-kanata base`, then press **Meta+Alt+K** again | The overlay comes back showing layer `base`. |
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
