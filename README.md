# kanata-overlay

An on-screen overlay for the [Kanata](https://github.com/jtroo/kanata) keyboard remapper:
it draws **your** physical keyboard and shows what each key does on the active layer.
Rust + Qt 6/QML (via [cxx-qt](https://github.com/KDAB/cxx-qt)), built for KDE Plasma.

![scaffold](docs/scaffold.png)

*Current state: the scaffold window. It imports a KLE layout, connects to Kanata's
TCP server and shows the active layer. The overlay behaviour comes next (see Roadmap).*

## Why a layout designer?

Kanata's config says *which* keys exist (`defsrc`) but not *where* they are. The app
needs the physical geometry, so each key in a layout carries its position, size,
rotation and the Kanata key name that links it to `defsrc`:

```
physical key ──kanata name──▶ index in defsrc ──▶ action in the active deflayer
```

Layouts can be imported from [keyboard-layout-editor.com](https://www.keyboard-layout-editor.com/)
JSON; obvious legends (`Q`, `Esc`, `Caps Lock`…) are bound automatically and unbound
keys are outlined in orange.

## Quick start

Requires [Nix](https://nixos.org/download) with flakes enabled.

```sh
nix develop            # or: direnv allow
just fake-kanata nav   # terminal 1: fake Kanata, no permissions needed
just run               # terminal 2: the overlay
```

With real Kanata:

```sh
just check-perms       # can Kanata run without root? (see below)
just kanata            # Kanata with examples/kanata/minimal.kbd and TCP on 127.0.0.1:7070
LAYOUT=path/to/your-kle.json just run   # your own board
just watch             # print raw TCP events
```

Run `just` for every task. On non-NixOS distros, if Qt Quick fails to create an
OpenGL context from the Nix shell, run the app through
[nixGL](https://github.com/nix-community/nixGL) or with `QT_QUICK_BACKEND=software`.

## Running Kanata without root

Kanata needs to read `/dev/input/event*` and write `/dev/uinput`. Grant that to your
user once (from Kanata's [Linux setup guide](https://github.com/jtroo/kanata/blob/main/docs/setup-linux.md)):

```sh
sudo groupadd --system uinput
sudo usermod -aG input,uinput "$USER"
echo 'KERNEL=="uinput", MODE="0660", GROUP="uinput", OPTIONS+="static_node=uinput"' \
  | sudo tee /etc/udev/rules.d/99-input.rules
sudo modprobe uinput
sudo udevadm control --reload-rules && sudo udevadm trigger
# then log out and back in
```

The app is designed never to need root at runtime (constitution, principle III).

## Repository layout

| Path | What |
| --- | --- |
| `crates/kanata-client` | Kanata TCP client and process launcher (no Qt) |
| `crates/layout` | KLE import and the app's layout format (no Qt) |
| `crates/app` | `kanata-overlay` binary: cxx-qt bridge + QML |
| `examples/` | Sample KLE layout and Kanata config |
| `.specify/` | Spec Kit templates, scripts and the [constitution](.specify/memory/constitution.md) |
| `.claude/skills/` | Spec Kit commands for Claude Code (`/speckit-*`) |
| `flake.nix`, `justfile` | Dev shell and tasks |

## Development workflow

Features are built spec-first with [Spec Kit](https://github.com/github/spec-kit),
from Claude Code in this directory:

1. `/speckit-specify <what and why>` creates a branch and `specs/NNN-feature/spec.md`
2. `/speckit-clarify` (optional) resolves open questions
3. `/speckit-plan` writes the technical plan, checked against the constitution
4. `/speckit-tasks` breaks it into tasks
5. `/speckit-implement` builds it; `just check` must pass

## Roadmap

Suggested order for the first Spec Kit features:

1. **Overlay window**: always-on-top, click-through layer-shell surface on Plasma
   Wayland (LayerShellQt via a small C++ shim), screen position and opacity settings,
   toggle shortcut; X11 fallback via window hints.
2. **Layer actions on keys**: parse `defsrc` / `deflayer` / `defalias` from the
   `.kbd` file (re-parse on `ConfigFileReload`) and label each key with its action.
3. **Managed Kanata**: launch Kanata with a localhost TCP port, readiness probing,
   log panel, restart with backoff, `PR_SET_PDEATHSIG`; attach mode when Kanata is
   already running; optional `systemd --user` mode.
4. **Layout editor**: bind unbound keys, move/resize/rotate keys, save layouts.
5. **Live key highlighting**: tap-hold events from the TCP protocol
   (`HoldActivated`/`TapActivated`), evdev on Kanata's output device, optional
   `--debug` output parsing.
6. **Desktop integration**: tray icon, autostart, permissions setup through `pkexec`.

Example for step 1:

```
/speckit-specify Turn the window into an overlay for KDE Plasma Wayland: it stays
above all windows, never takes focus or mouse/keyboard input, sits at a configurable
screen edge with adjustable opacity and scale, and can be shown/hidden with a global
shortcut. It must still work as a normal window on X11.
```
