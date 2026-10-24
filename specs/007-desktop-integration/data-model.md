# Data Model: Desktop Integration

## Board style (settings file, per device)

| Field | Values | Rules |
|-------|--------|-------|
| `style` | `"full"`, `"compact"` | missing → `"full"`; invalid → `"full"` with a settings warning; settings stay version 2 |

## Tray entry (in memory, `TrayState`, one per device)

| Field | Meaning |
|-------|---------|
| `id` | the device id; the icon's id is `kanata-overlay-<id>` |
| `name` | tooltip title |
| `layer` | full layer name, `""` when not connected |
| `label` | short label (research R2): first 4 characters, first letter uppercase; `–` when not connected |
| `state` | the device's state text ("managed", "attached", "restarting Kanata (attempt 2)"…) |
| `connected` | bool: accent or grey icon |
| `managed` | bool: listed under Restart Kanata |

Entries are created, updated and dropped from `TrayState.update(devicesJson)`; an icon is redrawn only when `label` or `connected` changes.

## Login entry

Present ⇔ Start at login is on. Path `~/.config/autostart/io.github.jolitp.kanataoverlay.desktop`. Rewritten at start when its `Exec` differs.

## Launch target (research R9)

`Exec` of both entries: the installed wrapper when running from `/nix/store` (`KANATA_OVERLAY_WRAPPER`), else `~/.local/share/kanata-overlay/launch.sh` (rewritten at every start).

## Desktop entry

`~/.local/share/applications/io.github.jolitp.kanataoverlay.desktop` and the icon under `~/.local/share/icons/hicolor/scalable/apps/`; rewritten only when its contents (e.g. `Exec`) differ.

## Permission report (pure, `kanata_client::permissions`)

| Item | Ok when | NeedsLogin when |
|------|---------|-----------------|
| input devices | an `/dev/input/event*` is readable | not readable, user in `input` in the database but not in this session |
| virtual keyboard | `/dev/uinput` writable | not writable, user in `uinput` in the database but not in this session |

`fix`: the text for the missing items on this system (NixOS: `configuration.nix`; otherwise commands). Empty when everything is Ok.

## Single-instance protocol

One line per connection to `$XDG_RUNTIME_DIR/kanata-overlay-<hash of the settings path>.sock`: `settings` (open the settings window). Unknown lines are ignored. One instance per settings file.
