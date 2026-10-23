# Feature Specification: Overlay Window

**Feature Branch**: `001-overlay-window`

**Created**: 2026-10-16

**Status**: Draft

**Input**: User description: "Turn the window into an overlay for KDE Plasma Wayland: it stays above all windows, never takes focus or mouse/keyboard input, sits at a configurable screen edge with adjustable opacity and scale, and can be shown/hidden with a global shortcut. It must still work as a normal window on X11."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Overlay that never gets in the way (Priority: P1)

A Kanata user on KDE Plasma (Wayland) starts kanata-overlay. Instead of an ordinary
window, the keyboard picture appears on top of everything, including full-screen and
maximised windows. The user keeps typing and clicking in their other apps as if the
overlay were not there: it never steals keyboard focus, and clicks on it reach
whatever is underneath.

**Why this priority**: This is the core of the feature. An overlay that steals focus
or blocks clicks is worse than no overlay, because using it would interrupt the very
typing it is meant to help with.

**Independent Test**: Start the app on Plasma Wayland with a text editor focused.
Type and click through the area the overlay covers; the editor receives every
keystroke and click, and the overlay stays visible above it.

**Acceptance Scenarios**:

1. **Given** the app is running on Plasma Wayland, **When** the user focuses, maximises or full-screens another window, **Then** the overlay stays visible above it.
2. **Given** a text editor is focused, **When** the app starts or the overlay updates (e.g. the layer changes), **Then** the editor keeps keyboard focus and receives all typed characters.
3. **Given** the overlay covers part of another window, **When** the user clicks, scrolls or drags over the overlay, **Then** the window underneath receives the input as if the overlay were absent.
4. **Given** the overlay is shown, **When** the user switches virtual desktops or activities, **Then** the overlay is shown on the new desktop too.
5. **Given** the overlay is running, **When** the user opens the task switcher or task bar, **Then** the overlay does not appear as a regular window entry there.

---

### User Story 2 - Place, fade and size the overlay (Priority: P2)

The user wants the overlay where it does not cover their work: for example along the
bottom edge of the screen, at 60% opacity and 80% of its normal size. They set the
screen edge, the distance from that edge, the opacity and the scale, and the overlay
uses those settings now and every time it starts.

**Why this priority**: Without placement and sizing the overlay covers content in a
fixed spot, which limits its everyday usefulness. It is still secondary to the
overlay behaving correctly at all.

**Independent Test**: Open the settings window with its shortcut, set edge = top,
opacity = 50%, scale = 150% (the overlay updates as you change them), restart the app,
and check that the overlay appears centred along the top edge, half transparent and
one and a half times its default size.

**Acceptance Scenarios**:

1. **Given** the edge setting is top, bottom, left or right, **When** the overlay is shown, **Then** it is centred along that edge of the screen, offset by the configured margin.
2. **Given** an opacity between 10% and 100%, **When** the overlay is shown, **Then** the whole overlay (background, keys and labels) is drawn at that opacity.
3. **Given** a scale between 50% and 300%, **When** the overlay is shown, **Then** the keyboard, labels and header grow or shrink proportionally and the overlay stays on screen.
4. **Given** the user changed any of these settings, **When** the app is restarted, **Then** the same settings are in effect.
5. **Given** the user has more than one monitor, **When** they choose a monitor, **Then** the overlay is shown on that monitor.
6. **Given** the settings window is open, **When** the user moves a slider or picks an edge, **Then** the overlay updates immediately, and closing the window keeps the change.
7. **Given** a setting is passed on the command line, **When** the app runs, **Then** that value is used for this run and the settings file is unchanged.

---

### User Story 3 - Show and hide with a global shortcut (Priority: P2)

While working in any application, the user presses a keyboard shortcut to hide the
overlay when it is in the way, and presses it again to bring it back. Because the
overlay never has focus, the shortcut works system-wide.

**Why this priority**: Since the overlay ignores all input, a shortcut is the only
quick way to get it out of the way. It shares priority with placement because both
are needed for comfortable everyday use.

**Independent Test**: With another app focused, press the shortcut: the overlay
disappears. Press it again: it reappears in the same place with the same settings.

**Acceptance Scenarios**:

1. **Given** the overlay is shown and another app is focused, **When** the user presses the shortcut, **Then** the overlay is hidden within a fraction of a second.
2. **Given** the overlay is hidden, **When** the user presses the shortcut, **Then** the overlay is shown again with the current layer and settings.
3. **Given** the overlay is hidden, **When** Kanata changes layer, **Then** the app keeps tracking the layer and shows the correct one when the overlay is shown again.
4. **Given** the user wants a different key combination, **When** they change it in the desktop's shortcut settings, **Then** the new combination toggles the overlay.
5. **Given** the overlay is hidden, **When** the shortcut is pressed, **Then** the key press is not passed on to the focused application.

---

### User Story 4 - Still usable on X11 and other desktops (Priority: P3)

A user runs a Plasma X11 session, or a Wayland desktop that does not support
overlays. The app still starts and behaves as close to the Plasma Wayland overlay
as the session allows: borderless, on top, out of the task bar, placed at the
chosen edge, and click-through where possible.

**Why this priority**: The constitution makes Plasma Wayland the primary target and
X11 best-effort. A working fallback matters, but must not slow down the primary path.

**Independent Test**: Start the app in a Plasma X11 session and check that it shows
the keyboard with the current layer and responds to the edge, opacity and scale
settings as far as the session allows.

**Acceptance Scenarios**:

1. **Given** an X11 session, **When** the app starts, **Then** the keyboard is shown borderless, above other windows, at the configured edge, and layer changes are reflected as on Wayland.
2. **Given** an X11 session, **When** another window is focused or maximised, **Then** the overlay stays above it and does not appear in the task bar.
3. **Given** an X11 session that supports input pass-through, **When** the user clicks over the overlay, **Then** the click reaches the window underneath.
4. **Given** a Wayland session without overlay support, **When** the app starts, **Then** it uses the best-effort behaviour and tells the user which overlay properties are unavailable.

---

### Edge Cases

- The configured monitor is disconnected while the app is running, or is missing at start: the overlay moves to the primary monitor and returns if the monitor comes back.
- The screen resolution or scaling changes: the overlay stays anchored to its edge and fully on screen.
- The scaled overlay is larger than the screen: the scale is reduced until it fits, and the user is told the setting was capped.
- An invalid value in the settings (unknown edge, opacity 0 or 500%): the default for that setting is used and the problem is reported; the other settings still apply.
- The settings window is opened while the overlay is hidden: the overlay is shown for live preview while the window is open, then returns to hidden.
- The settings file is unreadable or from a newer app version: defaults are used, the user is told, and the file is not overwritten until they save from the settings window.
- A global shortcut is already taken by another app or cannot be registered: the overlay still runs and the user is told the shortcut is unavailable and how to set another one.
- Kanata is not connected: the overlay still shows, with the existing "not connected" state, and stays click-through.
- The user hid the overlay and then quit: on the next start it is shown again (see Assumptions).
- A full-screen game or video: the overlay stays on top (it is the user's job to hide it with the shortcut).

## Requirements *(mandatory)*

### Functional Requirements

**Overlay behaviour (Plasma Wayland)**

- **FR-001**: On KDE Plasma Wayland, the overlay MUST be drawn above all normal, maximised and full-screen application windows.
- **FR-002**: The overlay MUST NOT take keyboard focus, neither at start-up, when it is shown again, nor when its content changes.
- **FR-003**: All mouse, touch and scroll input over the overlay MUST pass through to whatever is underneath.
- **FR-004**: The overlay MUST be visible on all virtual desktops and activities.
- **FR-005**: The overlay MUST NOT appear in the task bar, task switcher or window lists as a regular window.
- **FR-006**: The overlay MUST NOT reserve screen space; maximised windows keep their full size underneath it.

**Placement and appearance**

- **FR-007**: Users MUST be able to choose the screen edge (top, bottom, left, right) the overlay is centred along, and a margin from that edge. Default: bottom edge, small margin.
- **FR-008**: Users MUST be able to set the overlay's opacity from 10% to 100%. Default: 85%.
- **FR-009**: Users MUST be able to set the overlay's scale from 50% to 300% of its default size. Default: 100%.
- **FR-010**: Users MUST be able to choose which monitor shows the overlay. Default: the primary monitor.
- **FR-011**: Placement, opacity, scale and monitor settings MUST persist across restarts in a per-user settings file, which is the single source of truth.
- **FR-011a**: Users MUST be able to change the settings in a small settings window, opened with a second global shortcut (default: Meta+Alt+Shift+K). Changes made there MUST apply to the overlay immediately (live preview) and be saved to the settings file.
- **FR-011b**: The settings window is an ordinary focusable window; opening it MUST NOT change the overlay's click-through or focus behaviour.
- **FR-011c**: Users MUST be able to edit the settings file by hand; hand edits take effect on the next start.
- **FR-011d**: Command-line options MUST be able to override any setting for a single run without changing the settings file.

**X11 and fallback**

- **FR-012**: On X11, and on Wayland desktops without overlay support, the app MUST still run and show the keyboard as a best-effort overlay: borderless, kept above other windows, left out of the task bar and window lists, placed by the edge / margin / monitor settings, and click-through and focus-less, each wherever the session allows it (on Wayland without overlay support, placement and stacking are up to the desktop and are reported as missing per FR-013). Opacity and scale MUST work as on Plasma Wayland.
- **FR-013**: When the full overlay behaviour is not available, the app MUST tell the user once (e.g. in the status line or log) which parts are missing.

**Show / hide**

- **FR-014**: The app MUST provide a system-wide shortcut that toggles the overlay between shown and hidden, working while any other application is focused. Default combination: Meta+Alt+K.
- **FR-015**: Users MUST be able to change both shortcut combinations (toggle overlay, open settings) through the desktop's standard shortcut settings.
- **FR-016**: While hidden, the app MUST keep its Kanata connection and layer state, so the overlay is up to date the moment it is shown.
- **FR-017**: If the shortcut cannot be registered, the app MUST keep running and tell the user.

**Privacy and safety**

- **FR-018**: Shortcut handling MUST NOT observe or record any key presses other than the app's own global shortcuts (toggle overlay, open settings) (constitution VI).
- **FR-019**: Settings MUST contain only placement and appearance preferences; no key-press data is ever stored.

### Key Entities

- **Overlay settings**: The user's preferences for the overlay: screen edge, margin from the edge, opacity, scale, chosen monitor and whether the overlay starts shown. Stored per user, versioned so later features can add fields.
- **Overlay visibility**: Whether the overlay is currently shown or hidden; changed by the global shortcut. Not persisted (see Assumptions).

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: In a manual test on Plasma Wayland, 100% of typed characters and clicks in the area covered by the overlay reach the application underneath (tested with at least 100 keystrokes and 20 clicks).
- **SC-002**: The overlay stays visible above other windows through 10 consecutive focus changes, maximise / full-screen toggles and desktop switches.
- **SC-003**: The global shortcut hides or shows the overlay in under 200 ms, from any focused application.
- **SC-004**: A user can place the overlay on a different edge with a different opacity and scale in under 2 minutes, and those settings survive a restart.
- **SC-005**: On X11, the app starts and shows the current layer in 100% of launches, without errors that stop it from running.
- **SC-006**: The overlay's start-up time grows by no more than 0.5 s compared with the current window.

## Assumptions

- The primary environment is KDE Plasma 6 on Wayland; other Wayland desktops get the fallback behaviour unless they happen to support overlays.
- The overlay is centred along its chosen edge; free positioning by dragging is out of scope, since the overlay ignores mouse input. Corners can be added later.
- The overlay always starts shown, even if it was hidden when the app last quit; this avoids a "the app started but I can't see it" situation.
- The default shortcut (Meta+Alt+K) is a suggestion; the desktop may refuse or change it, and the user can rebind it.
- The current window's look (colours, key caps, header with connection state and layer name) stays as it is; this feature changes how and where it is shown, not what it shows.
- The settings window covers only this feature's settings (edge, margin, opacity, scale, monitor); a tray icon, autostart and settings for later features belong to the "Desktop integration" feature.
- The default settings-window shortcut (Meta+Alt+Shift+K) is rebindable like the toggle shortcut.
- Settings file edits made by hand while the app runs are not picked up until restart, and may be overwritten if the settings window saves first.
- Manual test notes for Plasma Wayland and X11 will be part of the plan, as the constitution requires for features touching the compositor.
