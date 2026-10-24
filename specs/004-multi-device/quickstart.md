# Quickstart & Validation: Several Devices

[data-model.md](data-model.md) · [contracts/interfaces.md](contracts/interfaces.md)

## Prerequisites

- `nix develop`, and the permissions from feature 003 for the real-Kanata sections.
- Two Kanata configs, each limited to its device:

```lisp
;; defy.kbd
(defcfg linux-dev-names-include ("DYGMA DEFY Keyboard")
        linux-continue-if-no-devs-found yes)   ;; wait instead of exiting while unplugged
;; mouse.kbd: the keypad and the side buttons are separate devices. Three receiver
;; devices share the name "Compx 2.4G Wireless Receiver" (event3 mouse, event6, event7),
;; and this include grabs all three
(defcfg linux-dev-names-include ("Compx 2.4G Wireless Receiver"
                                 "Compx 2.4G Wireless Receiver Keyboard")
        linux-continue-if-no-devs-found yes)
(defsrc esc tab caps bspc del ret spc lctl lalt lsft lmet mbck mfwd)
```

- **Escape hatch**: `pkill -x kanata`.

## 1. Automated

`just check`. The tests cover:
- **settings**: v2, v1 migration, implicit device, per-device leniency, ids;
- **kbd**: device selection parsing, mouse glyphs, the layout grid;
- **kanata-client**: `/proc/bus/input/devices` parsing (with a fixture of the user's devices), `grabs` (paths, include, exclude, auto, mouse mode) and overlap.

## 2. Two devices with fake Kanatas (no permissions)

1. Run `KANATA_PORT=7070 just fake-kanata base` and `KANATA_PORT=7071 just fake-kanata nav`.
2. In settings, add "Defy" (`examples/kanata/minimal.kbd`, `examples/layouts/sample.kle.json`, bottom) and "Mouse" (`examples/kanata/all-actions.kbd`, no layout, bottom-right).

| Expected | Requirement |
|----------|-------------|
| Two boards, both "attached". The Defy board shows `base` labels; the Mouse board shows a grid with the `nav` layer | FR-004, FR-007, R7 |
| Restarting one fake server on another layer changes only that board | SC-002 |
| Removing "Mouse" makes its board disappear at once | FR-001a |

## 3. The user's devices, managed (real Kanata)

Set up both devices with the configs above, then quit any running Kanata.

| Step | Expected |
|------|----------|
| `just run` | Both boards "managed" within 6 s (SC-001). The laptop keyboard types normally (no remapping). |
| Hold a Defy layer key | Only the Defy board changes (SC-002). |
| `pkill -x kanata` once | Each Kanata restarts on its own. Typing on the other device is never interrupted (SC-004). |
| Quit via Quit, Ctrl+C, `pkill -9 kanata-overlay` (10× in total) | `pgrep -ax kanata` is empty every time (SC-003). |
| Mouse side buttons | Labelled with their action, or `🖱←`/`🖱→`. The mouse still moves and clicks (FR-010). |

## 4. Overlap warnings (US3)

| Setup | Expected |
|-------|----------|
| Remove the `defcfg` from `mouse.kbd` | Warning: "mouse.kbd would grab every keyboard: … AT Translated Set 2 keyboard …". Only the Defy's Kanata starts. |
| Put "DYGMA DEFY Keyboard" in both configs | Warning naming both configs and the Defy. Only the first starts. |
| The configs above | No warning (SC-005). |

## 5. Unchanged single-device use (SC-006)

`just run` with no devices configured behaves as in feature 003, and an existing v1 settings file keeps its placement.

## Validation log

### 2026-10-23: automated checks and agent smoke tests (fake Kanatas only)

| Check | Result |
|-------|--------|
| `just check`: 97 tests (settings 27, kbd 37, kanata-client 23 unit + 4 process, layout 5, app 1), clippy `-D warnings` | ✅ |
| `/proc` fixture of the user's machine: 7 devices, three named "Compx 2.4G Wireless Receiver" (`event3`, `event6`, `event7`); keyboard and mouse flags right; the user's Defy and mouse configs don't overlap | ✅ |
| Missing device (fake prints Kanata's "No keyboard devices were found…" and exits): "waiting for its device", never `GaveUp` (process test) | ✅ |
| **Refactor checkpoint (T012)**, one implicit device: managed on `--connect`'s port, `kill -9` of the overlay → its Kanata gone, attached → the fake is left running, `--kanata /nope` → notice; the implicit device is not written to the settings file | ✅ (after two fixes, below) |
| Two devices in a v2 file (fakes on 7081 `base`, 7082 `nav`): two boards, "Defy · base" top-left with the sample layout, "Mouse · nav" top-right as a grid of `all-actions.kbd`'s keys with `nav` labels (screenshot) | ✅ FR-004, FR-007, FR-008 |
| Removing a device live (temporary timer calling `removeDevice`): the file keeps only `defy` | ✅ FR-001a |
| Device check with the example configs (no `defcfg` limits): "would grab every keyboard: …", plus "all-actions.kbd and minimal.kbd both grab …; all-actions.kbd was not started"; the blocked device still attaches to a running Kanata | ✅ FR-011 |

**Fixed during the checkpoint**:
- `Main.qml`'s delegate redeclared `required property string deviceId`. That shadowed `DeviceView`'s property, so no device was created.
- `--connect` was ignored, because every device used `7070 + index`. Now the first device keeps `--connect` (FR-002).

**Design notes**:
- **No restarts on add**: the `Instantiator` uses a `ListModel` updated incrementally. A plain JS array would recreate, and so restart, every device on each change.
- **`placement()` signature**: it takes a `QRectF` and a `QSizeF`, because cxx-qt allows no `#[allow]` on bridge items and 8 arguments trip clippy.
- **`Appearance`**: the per-device appearance struct is `settings::Appearance`. The contract's name, `Placement`, was already taken by `place()`'s result.
- **Unlimited-config warning with one device**: it is shown only with **several** devices. With one device it would fire for every existing single-device setup, which FR-002 forbids.
- **T009's two sub-steps were done in one pass**, and checked at the end (the T012 checkpoint above) instead of after each step.

**Still to do by hand with real Kanata**:
- T018 (the user's Defy and mouse, SC-001 timing to 6 s, SC-002 – SC-004);
- T022 (mouse side buttons);
- T029 (overlap warnings with real configs).

**T012 (2026-10-23)**: passed by the user with real Kanata.

**Configs for T018 (2026-10-23)**: `~/.config/kanata/defy.kbd` (by name), `mouse.kbd`
(by-id `linux-dev` links, from the user's earlier home-manager module) and
`mouse-layout.json`. Both pass `kanata --check`. Fix found on the way: the device check
compared `linux-dev` paths by file name, so by-id links matched nothing; they are now
resolved first (`kanata_client::resolve_links`, tested). On the real device list:
Defy → event13, mouse → event3 + event4, no overlap.

**T018 (2026-10-23, partly)**: by the user with real Kanata. Defy and mouse both
"managed · connected"; Caps on the mouse shows `game` and holding Esc on the Defy
shows `nav` (arrows on hjkl, Home/End on u/o), each on its own board only. `pkill -x kanata`:
both come back within a couple of seconds; the laptop keyboard keeps typing. Passed.

**T022 (2026-10-23)**: side buttons go back and forward and the mouse moves, scrolls
and clicks with its Kanata running. Passed.

**T029 (2026-10-23)**: the warning first appeared only after a fresh start; the check
ran only on load and on settings edits. Fixed: `SettingsState.recheck()` every 2 s
(Main.qml timer) bumps the revision only when a result changed, and a device whose
check result changed starts over like for a new setup. Agent check with two fake
Kanatas: removing one config's limit blocks it within the 2 s cycle ("not started"),
restoring the limit starts it again; the other device is untouched.
User recheck with real Kanata (2026-10-23): passed.
Fixes on the way: `~/` in typed paths, the portal file dialog (hidden folders, typed
paths) and the device dropdown in settings (stale selection, swapped fields, fields
not reloaded); `defy.kbd` gained Esc as tap Esc / hold `nav`.
