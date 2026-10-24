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

*(filled in during implementation)*
