# Feature Specification: Live Key Highlighting

**Feature Branch**: `006-live-key-highlighting`

**Created**: 2026-10-23

**Status**: Draft

**Input**: User description: "Live key highlighting: while Kanata runs, each device's board highlights the keys being pressed on that device, shows when a tap-hold key resolves to tap or hold, and the layout editor gains press-to-bind (press the key on the device to bind the selected key). Key presses stay in memory only and are never logged or saved."

## Context

Each device's board shows its active layer and what every key does (features 002 and
004), but not what the user is doing right now. Two things are missing:

- **Pressed keys**: while learning a layout, seeing the key light up confirms "this is
  the key I think it is", especially on split keyboards and odd devices like the mouse
  keypad.
- **Tap-hold results**: keys like the mouse's Caps (tap: switch layer, hold: Caps Lock)
  or a home-row modifier act differently depending on timing; the user can't see which
  one Kanata decided.

The layout editor (feature 005) binds keys from a list only; its clarification deferred
"press the key on the device to bind it" to this feature.

Key presses are sensitive: what is pressed can reveal passwords. Constitution VI: key
data stays in memory, is never written to disk, logs or crash reports, and never
leaves the machine.

While Kanata runs, it takes the device for itself, so the overlay can only learn about
presses through Kanata. Kanata reports tap-hold results to any connected client; it
reports individual key presses only in its detailed diagnostic output, which is
available when the overlay started that Kanata itself.

## Clarifications

### Session 2026-10-23

- Q: Where do key presses come from, and for which devices? → A: Kanata's detailed output: exact physical keys, for Kanatas the overlay starts (managed). Optional and kept separate, since that output isn't a stable interface (constitution II). Tap-hold results come from Kanata's normal reports, for every Kanata.
- Q: Which pressed keys are shown, given that typing can reveal passwords? → A: A per-device choice between "all keys" and "no typing keys" (everything except letters, digits and symbols), default all keys; highlighting can also be off.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - See which key I'm pressing (Priority: P1)

While typing or holding keys, the matching keys on the device's board light up, and go
dark when released. Keys held together all stay lit. Pressing a key on the Defy lights
it on the Defy's board only.

**Why this priority**: the core of the feature: it ties the picture to the device
under the user's fingers.

**Independent Test**: with the overlay running a device, press and hold a key: it lights
on that device's board; release: it goes dark. Hold three keys: three light.

**Acceptance Scenarios**:

1. **Given** a running device, **When** the user presses a key, **Then** the key it is bound to on that device's board is highlighted while held, and the highlight ends on release.
2. **Given** two devices, **When** the user presses a key on one, **Then** only that device's board changes.
3. **Given** a key that isn't on the board (not in the layout), **When** it is pressed, **Then** nothing is highlighted and nothing breaks.
4. **Given** a key pressed while its board is hidden, **When** the board is shown again, **Then** no stale highlight remains.

---

### User Story 2 - See whether a tap-hold key tapped or held (Priority: P1)

When a tap-hold key resolves, its key on the board briefly shows which way it went:
"tap" or "hold", with the action that fired.

**Why this priority**: tap-hold timing is the most common source of "why did that
happen?" with Kanata, and the result is otherwise invisible.

**Independent Test**: tap the mouse's Caps quickly: its key shows "tap"; hold it: it
shows "hold".

**Acceptance Scenarios**:

1. **Given** a tap-hold key, **When** it is tapped, **Then** its key shows a tap indication for about a second.
2. **Given** a tap-hold key, **When** it is held past its timeout, **Then** its key shows a hold indication while held (for a Kanata started outside the overlay, whose releases aren't visible, for 1.5 s).
3. **Given** the device's Kanata was started outside the overlay, **When** a tap-hold key resolves, **Then** the indication still appears.

---

### User Story 3 - Bind a key by pressing it (Priority: P2)

In the layout editor, with one key selected, the user chooses "Press to bind" and
presses the key on the device; the selected key is bound to it. Esc or a timeout
cancels.

**Why this priority**: the fastest way to bind odd devices; it builds on US1's source
of presses and on feature 005's editor.

**Independent Test**: select an unbound key, choose Press to bind, press the matching
key on the device: the key is bound to it and the editor says so.

**Acceptance Scenarios**:

1. **Given** one selected key and a running device, **When** the user chooses Press to bind and presses a device key, **Then** the selected key is bound to that key (one undo step).
2. **Given** Press to bind is waiting, **When** 10 seconds pass or the user cancels, **Then** nothing is bound.
3. **Given** the device's Kanata was started outside the overlay, or its highlighting is off, **Then** Press to bind is disabled with the reason shown. With "no typing keys", Press to bind still accepts any key while it is waiting.
4. **Given** Press to bind is waiting, **When** the device key is pressed, **Then** Kanata still does what that key normally does (the overlay only watches).

---

### Edge Cases

- **Rapid typing** (10+ keys per second): highlights keep up; none stay stuck after release.
- **Kanata restarts or disconnects** while keys are held: all highlights of that device clear.
- **A key bound twice on the layout** (feature 005's duplicates): both copies light.
- **Key repeat** while holding: one continuous highlight, no flicker.
- **Layer changes while a key is held**: the highlight stays on the physical key; the label changes with the layer as today.
- **The layout editor is open**: its canvas also highlights pressed keys, so the user can find a key on the canvas by pressing it.
- **Overlay hidden** (show/hide shortcut): nothing is shown, and no presses are kept.
- **Highlighting turned off for a device**: no presses of that device are read or kept.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: Each device's board MUST highlight the keys currently held on that device and clear each highlight on release. Presses come from the detailed output of a Kanata the overlay started for that device.
- **FR-002**: A highlight MUST appear within 50 ms of the press reaching the overlay and clear within 50 ms of the release.
- **FR-003**: When Kanata reports that a tap-hold key resolved, its key MUST show "tap" (for about one second) or "hold" (while held), for every connected device, whoever started its Kanata, unless that device's highlighting is "off" (a typing key's result is also hidden under "no typing keys").
- **FR-004**: All highlights of a device MUST clear when its Kanata stops, restarts or disconnects, and when its board is hidden.
- **FR-005**: Users MUST be able to choose per device, in the settings window, between "all keys" (default), "no typing keys" (letters, digits and symbol keys are never highlighted; modifiers, layer, navigation, function, editing and mouse keys are) and "off". With "off", none of that device's presses are read or kept, and its Kanata is started without detailed output. With "no typing keys", typing keys are dropped as soon as they arrive and never kept.
- **FR-006**: The layout editor MUST offer Press to bind for a single selected key: the next key pressed on the edited device binds it; Esc, a 10-second timeout or a click elsewhere cancels.
- **FR-007**: Press to bind MUST NOT change what the pressed key does on the device.
- **FR-008**: The layout editor's canvas MUST highlight held keys too.
- **FR-009**: Key presses MUST stay in memory only: never written to files, logs (including the Kanata log window and the app's own logs), crash reports or settings, and never sent anywhere (constitution VI). Only the set of currently held keys and the last tap-hold result per key are kept; there is no history.
- **FR-010**: When presses aren't available for a device (its Kanata was started outside the overlay), the settings window MUST say so for that device ("pressed keys: only for a Kanata started by the overlay"); tap-hold results still show. The board header is not used for this.

### Key Entities

- **Held keys**: per device, the set of physical keys currently held. Exists only in memory; emptied on release, disconnect, restart, hide or when highlighting is off.
- **Tap-hold result**: per device and key, "tap" or "hold" with its start time; replaced by the next result, expired after it is shown.
- **Highlight setting**: per device, "all", "no typing keys" or "off" (saved with the device's settings; missing means "all").

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A pressed key lights on its board fast enough that the user perceives it as instant (no visible lag) during normal typing.
- **SC-002**: After 1 minute of fast typing (10 keys per second), no key stays highlighted after all keys are released.
- **SC-003**: The user can tell whether a tap-hold key tapped or held in 10 out of 10 tries.
- **SC-004**: Binding an unbound key by pressing it takes under 5 seconds per key.
- **SC-005**: After any session, a search of the app's files, logs and settings finds no record of which keys were pressed.

## Assumptions

- Kanata's tap-hold reports name the physical key, as its protocol documents; boards map that name to the layout's key as labels already do.
- One overlay instance; each device's presses come from that device's own Kanata (feature 004).
- Highlighting is "all keys" by default for new and existing devices.
- "Typing keys" are letters, digits, the symbol keys (`-`, `=`, `[`, `]`, `\`, `;`, `'`, `` ` ``, `,`, `.`, `/`), space and the keypad digits; everything else is shown.
- The highlight style follows the board's look (a lighter key face with an accent border); exact styling is a design detail.
- Mouse buttons (`mbck`, `mfwd`) are keys like any other here.
- Showing presses on screen is inherent to the feature; a user who screen-shares can pick "no typing keys" or "off" (FR-005).
- Press to bind is a deliberate, short request from the user, so while it waits it accepts typing keys even under "no typing keys"; the pressed key is used for the binding and then dropped.
