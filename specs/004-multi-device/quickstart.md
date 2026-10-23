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
