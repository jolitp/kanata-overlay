# Feature Specification: Desktop Integration

**Feature Branch**: `007-desktop-integration`

**Created**: 2026-10-24

**Status**: Draft

**Input**: User description: "Desktop integration: a system tray icon that shows each device's current Kanata layer (tooltip and short label) with a menu to show/hide the boards, open settings, restart a device's Kanata and quit; a per-device board style "full" (the keyboard, as today) or "compact" (only the device name and layer); start the overlay automatically at login; install a desktop entry so the desktop knows the app; and help the user set up the permissions Kanata needs without root."

## Context

Today the overlay is started from a terminal (`just run`) and controlled by two global
shortcuts (show/hide, settings). There is no visible handle on it: nothing in the panel,
no menu entry, no way to start it at login, and the desktop warns at startup that it
doesn't know the app. A full keyboard board is also a lot of screen for someone who only
wants to know which layer is active.

Kanata needs access to input devices without root; the README explains how, but the
overlay only reports a failure after Kanata can't start.

## Clarifications

### Session 2026-10-24

- Q: One tray icon for all devices, or one per device? → A: One icon per device: each shows its own device's layer; every icon has the same menu.
- Q: How far does the permissions help go? → A: Check and explain: name what is missing and show the fix for this system, ready to copy (NixOS gets the `configuration.nix` lines). The app never applies it.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - See and control the overlay from the panel (Priority: P1)

Each device has its own icon in the panel's system tray, showing that device's active
layer at a glance; its tooltip names the device, its layer and its state. Its menu shows or hides the boards, opens the
settings, restarts a device's Kanata and quits.

**Why this priority**: the everyday handle on the overlay, and the layer readout the user
asked for without any board on screen.

**Independent Test**: start the overlay: the icon appears; switch layers on a device: the
icon and tooltip follow; use each menu entry.

**Acceptance Scenarios**:

1. **Given** the overlay is running with two devices, **When** the panel is visible, **Then** there are two tray icons, each showing its device's active layer, with a tooltip "name · layer · state".
2. **Given** a layer change on a device, **When** it happens, **Then** that device's icon and tooltip update within a second; the other icons don't change.
3. **Given** the tray menu, **When** the user picks Show/Hide boards, Settings…, Restart <device>, or Quit, **Then** that happens (Restart only for devices whose Kanata the overlay manages; Quit stops managed Kanatas as today).
4. **Given** a click on a device's icon, **When** it happens, **Then** that device's board is shown or hidden; the other boards don't change. (Changed during validation, 2026-10-24: clicking one icon hid every board, which surprised the user.) If every board was hidden, the click shows just that one.
5. **Given** a desktop without a system tray, **When** the overlay starts, **Then** it runs as today and says once that the tray isn't available.

---

### User Story 2 - A small board that only shows the layer (Priority: P1)

Per device, the board style is "full" (the keyboard, as today) or "compact": a small pill
with the device name and its layer (and the connection dot), placed like the full board.

**Why this priority**: the user's request: know the layer without covering the screen.

**Independent Test**: set a device to compact: its board shrinks to the pill at the same
corner; layer changes still show; set it back to full.

**Acceptance Scenarios**:

1. **Given** a device set to compact, **Then** its board shows only its name, layer and state dot, at its configured position, scale and opacity.
2. **Given** a compact board, **When** the layer changes, **Then** the pill updates as fast as the full board would.
3. **Given** two devices, **When** one is compact, **Then** the other is unaffected.
4. **Given** a compact board with live key highlighting on (feature 006), **Then** no keys are shown and no presses or tap-hold results are read for it (the pill has nothing to highlight).

---

### User Story 3 - Start at login and be known to the desktop (Priority: P2)

The settings window has "Start at login". The app installs a desktop entry for itself, so
it appears in the application menu with a name and icon, and the desktop's "app not
known" warning goes away.

**Why this priority**: removes the terminal from daily use; less urgent than the tray and
the compact board.

**Independent Test**: tick Start at login, log out and in: the overlay is running with its
tray icon; the application menu lists Kanata Overlay; no startup warning.

**Acceptance Scenarios**:

1. **Given** Start at login is ticked, **When** the user logs in, **Then** the overlay starts (once, even if it was also started by hand).
2. **Given** Start at login is unticked, **When** the user logs in, **Then** it does not start.
3. **Given** the app has run once, **Then** the application menu lists it and launching it from there starts the overlay (or brings up its settings if it is already running).
4. **Given** the app is started a second time while running, **Then** no second overlay starts; the running one opens its settings.

---

### User Story 4 - Get the permissions right (Priority: P2)

The settings window shows whether Kanata can run without root on this system (input
devices readable, the virtual keyboard device writable, group membership active in this
session), and what to do when it can't, per the clarification.

**Why this priority**: a failed first start is the most common Kanata problem; the
overlay can explain it before Kanata fails.

**Independent Test**: on a system missing a permission, open settings: the check names
what's missing and shows the fix for this system; after fixing and logging in again, the
check passes.

**Acceptance Scenarios**:

1. **Given** all permissions are in place, **Then** the check shows "ready".
2. **Given** a missing permission, **Then** the check names it and shows the fix for this system (NixOS gets the declarative configuration, other systems the commands).
3. **Given** the groups were added but this session predates them, **Then** the check says "log out and back in" rather than "missing".
4. **Given** a managed Kanata fails with a permission error, **Then** its message links to the check.

---

### Edge Cases

- **Many devices** (5+): one icon each; the menu's Restart entries stay usable (a submenu).
- **Device added or removed**: its icon appears or disappears with it.
- **Telling icons apart**: each icon's tooltip starts with the device name; the menu marks which device it belongs to.
- **Long layer names**: the icon's label is shortened (first characters); the tooltip shows the full name.
- **Device not connected**: tooltip shows "not connected" / its state; the label shows "–".
- **Tray restarted** (panel crash, Plasma restart): the icon comes back.
- **Start at login, but the app was moved or uninstalled**: the stale login entry does nothing harmful; unticking removes it.
- **Overlay hidden and tray menu used**: Settings… still opens the settings window.
- **Compact board and the layout editor**: editing a compact device's layout still works; the board stays compact.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The overlay MUST show one system tray icon per device while running, each with a tooltip of that device's name, layer and state; icons appear and disappear with their devices.
- **FR-002**: The tray icon MUST show a short label of the active layer (at most 4 characters, shortened from the layer name; "–" when not connected), updated within 1 second of a layer change.
- **FR-002a** *(added on request, 2026-10-24)*: Each layer MUST have a colour, chosen automatically and always the same for the same layer name (a fixed palette), used for its device's tray icon and the board's state dot; grey when not connected. Choosing colours is a later feature.
- **FR-003**: Every icon's menu MUST offer: Show/Hide <its device>, Show/Hide all boards, Compact board (a tick for its device's board style, added on request 2026-10-24), Settings… (opened on that icon's device), Restart Kanata for each managed device, and Quit. Activating an icon MUST toggle its own device's board only; the global shortcut and "all boards" toggle every board, and showing all brings back boards hidden on their own.
- **FR-004**: Without a system tray the overlay MUST keep working as today and tell the user once.
- **FR-005**: Each device MUST have a board style setting, "full" (default) or "compact", changeable in the settings window without restarting Kanata.
- **FR-006**: A compact board MUST show only the device name, its layer and the connection state, placed by the device's position, margin, scale, opacity and monitor; it MUST NOT read key presses (feature 006) while compact.
- **FR-007**: The settings window MUST offer "Start at login"; ticking it makes the overlay start in each new desktop session, unticking stops that.
- **FR-008**: Only one overlay MUST run per user session and settings file; starting it again with the same settings MUST open the running one's settings instead.
- **FR-009**: The app MUST provide a desktop entry (name, icon, description) so the desktop and its application menu know it, and the startup "app not known" warning no longer appears. Launched from the menu or at login, it MUST start with everything it needs (its Qt modules, Kanata), whether installed as a package or run from a development build.
- **FR-010**: The settings window MUST show whether Kanata can run without root, naming each missing piece and showing how to fix it on this system, ready to copy (NixOS detected and given its declarative `configuration.nix` form). The app never applies the fix itself.
- **FR-011**: The permission check MUST distinguish "missing" from "added, but needs a new login".
- **FR-012**: The app MUST NOT need root at runtime and never asks for administrator rights (constitution III); steps that need them are shown for the user to run.

### Key Entities

- **Board style**: per device, "full" or "compact" (saved with the device).
- **Start at login**: on or off (the presence of the login entry, not a separate setting).
- **Permission check**: input devices readable, virtual keyboard writable, groups active in this session; each passed, missing, or "needs a new login".

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: With all boards hidden or compact, the user can tell each device's active layer at a glance from the panel, within 1 second of a change.
- **SC-002**: A compact board covers less than a tenth of the area of the same device's full board.
- **SC-003**: After ticking Start at login, the overlay is running within 10 seconds of the next login, without a terminal.
- **SC-004**: A user on a fresh system who follows the permission check's instructions gets Kanata running without root on the first try.
- **SC-005**: No "app not known" warning appears at startup once the desktop entry is installed.

## Assumptions

- KDE Plasma's system tray (StatusNotifierItem) is the primary target (constitution IV); other trays that support the same standard work too.
- The icon is a simple keyboard glyph with the layer label drawn on it; a themed or monochrome variant follows the panel's style.
- Start at login uses the desktop's standard autostart mechanism for the user, not a system service.
- The desktop entry and icon are installed into the user's own data folder, so no root is needed.
- Board style changes take effect immediately and don't restart Kanata (the style doesn't change the Kanata session; switching to compact only stops reading presses).
- Permissions are checked by looking at the system (device files, groups), never by starting Kanata.
