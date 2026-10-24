# Quickstart: Desktop Integration

§1 needs nothing; §2 uses the fake Kanata (agents can run it, with a separate
`--settings` file); §3 is for the user.

## 1. Automated

```sh
just check
```

Covers: `short_label` (base, nav, long names, digits, not connected), tooltip and menu
model; the `style` setting (missing, invalid, round trip); desktop and login entry
contents; the single-instance line protocol; `permissions::report` for every
combination (ok, missing, needs-login; NixOS and not).

## 2. With the fake Kanata

1. **Tray (US1)**: start with two devices: two tray icons, labels from their layers, tooltips "name · layer · state"; a layer change on one updates only its icon (`request_layer` from a script). Menu entries emit their signals (script). With no StatusNotifierWatcher (a session without a tray, e.g. `dbus-run-session`), the overlay starts and adds the "no system tray" notice.
2. **Compact (US2)**: set one device to compact: its board shrinks to the header at the same corner (screenshot; area under a tenth of the full board, SC-002); with `FAKE_KEYS` it highlights nothing; back to full.
3. **Single instance (US3)**: start a second overlay with the same settings file: it exits at once and the first opens its settings; one with a different settings file runs alongside.
4. **Desktop entry (US3)**: after a start, the desktop file, icon and `launch.sh` exist in a scratch `HOME`/`XDG_DATA_HOME`; the "App info not found" warning is gone on the next start; `launch.sh` starts the overlay from a clean environment. `nix build` gives a wrapped binary that starts outside the dev shell.
5. **Start at login (US3)**: tick and untick: the autostart file appears and disappears (scratch `XDG_CONFIG_HOME`).
6. **Permissions (US4)**: the settings window shows the check for this machine; unit tests cover the other cases.

## 3. User

1. Tray icons for the Defy and the mouse in the Plasma panel; layer changes follow within a second; every menu entry works; left click shows/hides the boards.
2. The mouse as compact board.
3. Start at login: log out and in; the overlay is there within 10 s (SC-003); the application menu lists Kanata Overlay; no startup warning (SC-005).
4. Permissions: the check says ready on this machine.

## Validation log

**Automated (2026-10-24)**: `just check` passes. New tests: tray `short_label` and
`tooltip`; the `style` setting (missing, invalid with a warning, round trip); desktop
and login entry contents, `Exec` quoting, write-only-when-changed, the launcher script;
the single-instance protocol (one socket per settings file, a stale socket replaced, a
second claim handing over); `permissions::report` (all ok, uinput missing on NixOS and
not, groups needing a new login, input missing) and `gather` on this machine.

**Agent (2026-10-24)**, fake Kanata, scratch settings and scratch `XDG_DATA_HOME` /
`XDG_CONFIG_HOME`:
- §2.1 tray: two items `kanata-overlay-a` / `-b` registered with Plasma's
  StatusNotifierWatcher, tooltips "Alpha · base · managed"; a layer change on Alpha
  changed only Alpha's tooltip ("navigation · managed"). The icon pixmap read back over
  D-Bus: 32×32, a blue rounded square with "Base" in white, transparent corners.
  `Activate` toggled the boards; the menu (Show/Hide, Settings… (Beta), Restart Kanata ›
  Alpha/Beta, Quit) clicked over DBusMenu: Settings… opened on Beta, Quit quit and left
  no Kanata. In a D-Bus session without a tray: "no system tray: use the shortcuts".
- §2.2 compact: 680×298 → 170×58 (under 5 % of the area, SC-002), the pill "● Alpha ·
  base"; no presses read while compact (`watched` false, nothing lit with `FAKE_KEYS`);
  back to full at 680×298.
- §2.3 single instance: a second start with the same settings printed "already
  running; opened its settings", exited 0, and the first opened its settings.
- §2.4 desktop entry: the entry, the icon and `launch.sh` written; the entry's `Exec`
  is `launch.sh` for the dev build. `launch.sh` started the overlay from `env -i` (only
  HOME, USER, XDG_RUNTIME_DIR, WAYLAND_DISPLAY, DBUS_SESSION_BUS_ADDRESS, a system PATH)
  with its tray icon and no QML/plugin errors. `nix build` gave a wrapped package that
  started the same way; its entry's `Exec` is the store wrapper and no `launch.sh` is
  written.
- §2.5 Start at login: the autostart entry appeared and disappeared with the toggle.
- §2.6 permissions: this machine reports ready (input devices readable, virtual
  keyboard writable, NixOS).

**User, first look (2026-10-24)**: clicking one tray icon hid both boards. Changed:
a click toggles that device's board only; the menu has Show/Hide <device> and Show/Hide
all boards; the shortcut and "all boards" toggle every board and showing all brings back
boards hidden on their own (`OverlayState.hiddenJson`, `toggleDevice`). Checked over
D-Bus: Alpha's icon hid only Alpha and brought it back; "all boards" hid both; Beta's
icon then showed only Beta.

**Compact board in the tray menu** (user request, 2026-10-24): a "Compact board" tick
on each icon sets that device's board style (saved, same setting as the settings
window). Checked over D-Bus: ticking shrank Alpha's board to 180×58 and the tick showed;
unticking restored 370×136; the settings file held "full" again.

**User (2026-10-24), T027 so far**: hiding one board from its icon works; the
compact board works (settings and tray menu); the settings window shows "✓ Kanata can
run without root" (§3.4, passed); the window's title bar shows the app icon, i.e. the
desktop entry is found. Kanata Overlay is listed in the application menu; launching
it while it runs opens the running overlay's settings (§3.3, single instance from the
menu). Passed. Pending: Start at login after a real login (the user will test it later).

**Layer colours** (user request, 2026-10-24): each layer name maps to one of 8 palette
colours (FNV-1a, tested: stable, typical names spread over several colours); the tray
icon is drawn in it (grey when not connected) and the board's state dot uses it.
Checked over D-Bus: Alpha's icon was blue (`#3b82f6`) in "base" and amber (`#d97706`)
after switching to "nav".

**App icon** (2026-10-24): a new icon (three layers in the palette colours under a
keycap). The menu first showed a blank icon: a `icon-theme.cache` left by Lutris in
`~/.local/share/icons/hicolor` lists only `128x128/apps`, so Qt trusted it and never
looked in `scalable/apps`. The app now touches every theme folder when it installs a new
icon, so such a cache counts as stale. After touching the folders, clearing Plasma's
icon cache, `kbuildsycoca6` and restarting plasmashell, the user's menu shows the icon.

**Not checked by the agent**: the "App info not found" warning going away needs the
real `~/.local/share` (the portal reads the user's real data folders); the tray as
drawn in the panel (checked through D-Bus instead); logging in with Start at login on.
