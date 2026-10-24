# Feature Specification: Several Devices

**Feature Branch**: `004-multi-device`

**Created**: 2026-10-23

**Status**: Draft

**Input**: User description: "Manage several devices: the overlay runs one Kanata per device group (e.g. a Dygma Defy and a mouse with an 11-key keypad and side buttons, while the laptop keyboard stays untouched by Kanata) and shows a board for each device with its Kanata's active layer and labels. Should the laptop keyboard appear in the overlay at all? No."

## Background

Kanata decides what a key does by its key code, not by the device it came from. The
user's mouse keypad sends the same codes as a keyboard (Esc, Tab, Enter, Ctrl…), so a
keypad key can only behave differently from the same key on the Defy if each device has
its **own Kanata**, each limited to its own device. Today the overlay runs or watches
exactly one Kanata and draws one board.

The user's setup:

| Device | Remapped by | Shown in the overlay |
|--------|-------------|----------------------|
| Dygma Defy (on its plain-keys firmware layer) | Kanata "defy" (`defy.kbd`) | yes |
| Mouse: 11-key keypad + two side buttons | Kanata "mouse" (`mouse.kbd`) | yes |
| Laptop keyboard | nothing (Kanata never grabs it) | no |

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Two Kanatas, two boards, one overlay (Priority: P1)

The user describes their two devices once: for each, a name, its Kanata config and its
physical layout. They start the overlay once. It starts (or attaches to) a Kanata for each
device and shows two boards: the Defy's and the mouse keypad's. Each board shows its own
Kanata's active layer and labels. Holding a layer key on the Defy changes only the Defy
board; the keypad board keeps showing the mouse's layer.

**Why this priority**: This is the feature: without it the user needs two overlay
instances, which fight over shortcuts and settings.

**Independent Test**: With two device entries (each with a config and layout), start the
overlay: two boards appear, each labelled from its own config. Change layer on one
Kanata: only that board's labels change. Quit: both managed Kanatas stop.

**Acceptance Scenarios**:

1. **Given** two devices are described in the settings window, **When** the overlay starts, **Then** it starts or attaches to one Kanata per device and shows one board per device, each with its device's name.
2. **Given** both boards are shown, **When** one Kanata changes layer, **Then** only that device's board changes its layer name and labels.
3. **Given** the overlay manages both Kanatas, **When** the overlay quits for any reason, **Then** both Kanatas stop (as in feature 003, for each of them).
4. **Given** one device's Kanata fails (e.g. its config has an error), **When** the user looks at the overlay, **Then** that board shows the failure and the other board keeps working normally.
5. **Given** a device whose Kanata is already running (e.g. the user started it), **When** the overlay starts, **Then** it attaches to that one and manages the other.

---

### User Story 2 - Mouse buttons and the keypad look right (Priority: P2)

The mouse board shows the keypad's 11 keys and the two side buttons, labelled like any
other key: the side buttons show what they do on the active layer, with recognisable
mouse-button symbols when they keep their default action.

**Why this priority**: The mouse is the second device the user cares about; its side
buttons are not keys today, so without this they'd show raw names.

**Independent Test**: A mouse config mapping the side buttons to actions and leaving one
as-is: the board shows the actions, and the unchanged one shows a mouse "back"/"forward"
symbol.

**Acceptance Scenarios**:

1. **Given** the mouse config maps a side button to an action, **When** the mouse board is shown, **Then** that button shows the action's label.
2. **Given** a side button keeps its default action, **When** shown, **Then** it shows a mouse back or forward symbol, not a raw name.
3. **Given** the mouse config includes mouse buttons, **When** its Kanata starts, **Then** mouse movement, clicks and wheel keep working normally (Kanata passes them through).

---

### User Story 3 - Devices never fight (Priority: P3)

Two Kanatas must never try to grab the same device, and a device the user wants left
alone (the laptop keyboard) must never be grabbed. The overlay helps the user get this
right instead of producing a stuck or doubled keyboard.

**Why this priority**: Misconfiguration here is confusing (keys typed twice, a Kanata
failing because the device is busy). Lower priority because careful configs avoid it.

**Independent Test**: Give two configs that would both grab the Defy: the overlay warns
before starting them and starts only the first, explaining why.

**Acceptance Scenarios**:

1. **Given** two device configs whose device selections overlap (or one selects nothing, which in Kanata means "all keyboards"), **When** the overlay starts, **Then** it warns, naming the configs and the overlap, and starts only the first of the conflicting Kanatas.
2. **Given** a device config does not limit itself to particular devices, **When** the overlay starts, **Then** it warns that this Kanata would grab every keyboard (including the laptop keyboard).
3. **Given** all selections are disjoint, **When** the overlay starts, **Then** no warning is shown.

---

### Edge Cases

- Only one device is described: the overlay behaves exactly like today (one board).
- No devices are described but `--config` is given (today's usage): treated as one device, so existing commands keep working.
- A device's layout file is missing: its board shows a generic grid of its config's source keys, with a notice.
- A device is unplugged while its Kanata runs: Kanata keeps running and picks it up again when it returns; only that board shows it.
- A device is missing when its Kanata starts (e.g. the wireless mouse is off): Kanata 1.12 exits unless `linux-continue-if-no-devs-found yes` is set. The overlay treats this as "waiting for its device", retrying without ever giving up, and only that board shows it.
- A board doesn't fit on its screen at its chosen scale: that board is scaled down to fit (as feature 001 does).
- Two boards are placed at the same position: they overlap; the settings window shows a hint, but the overlay doesn't move them.
- The show/hide shortcut: hides and shows all boards together.
- The Defy is on one of its own firmware layers (not the plain-keys layer): the overlay cannot know; its board may be wrong until the Defy is back on that layer (out of scope, see Assumptions).

## Requirements *(mandatory)*

### Functional Requirements

**Devices**

- **FR-001**: Users MUST be able to describe several devices, each with: a display name, a Kanata config, a physical layout, and optionally a fixed address for an already running Kanata. Devices are kept in the settings file and edited in the settings window: add, remove, rename, and choose the config and layout files (clarified 2026-10-23).
- **FR-001a**: Changes to the device list MUST take effect without restarting the overlay: an added device starts its Kanata, and a removed device stops its managed Kanata and its board disappears.
- **FR-002**: When exactly one device is described, or only today's `--config`/`--layout` options are used, behaviour MUST be the same as before this feature.
- **FR-003**: Devices that should stay untouched (like the laptop keyboard) MUST NOT be listed; the overlay MUST NOT show them.

**Kanata per device**

- **FR-004**: The overlay MUST run (managed) or attach to one Kanata per device, with all of feature 003's behaviour (start-up check, restarts, give-up, log, stop on exit) applied to each one independently.
- **FR-005**: A failure of one device's Kanata MUST NOT affect the other devices' Kanatas or boards.
- **FR-006**: Each managed Kanata MUST get its own local address, so they never collide.

**Boards**

- **FR-007**: The overlay MUST show one board per device with the device's name, its Kanata's connection state, active layer and per-key labels (feature 002), independently of the other boards.
- **FR-008**: Each board MUST have its own position (edge or corner, margin), opacity, scale and monitor, set per device in the settings window, with live preview as in feature 001 (clarified 2026-10-23).
- **FR-008a**: When there is only one device, its board uses today's placement settings, so existing settings files keep working.
- **FR-009**: The show/hide shortcut MUST hide and show all boards together.

**Mouse buttons**

- **FR-010**: Mouse side buttons (back, forward) MUST be labelled like keys; when they keep their default action they MUST show a mouse back/forward symbol.

**Device selection safety**

- **FR-011**: Before starting managed Kanatas, the overlay MUST check each config's device selection; overlapping selections, or a config without any selection, MUST produce a warning naming the configs, and only the first of overlapping managed Kanatas is started.

**Settings window**

- **FR-012**: The settings window MUST show each device with its Kanata state and its own Restart / Start / Show log actions.

### Key Entities

- **Device**: Something the user wants shown and remapped: display name, Kanata config, physical layout, optional fixed address.
- **Kanata session**: One Kanata (managed or attached) belonging to one device; as in feature 003.
- **Board**: The on-screen picture of one device: its layout, layer name and labels.
- **Device selection**: Which input devices a Kanata config will grab (from the config's own settings); used for the overlap check.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: With the user's two devices described, one start of the overlay brings up both Kanatas and both boards, each correctly labelled, within 6 seconds.
- **SC-002**: A layer change on one device is reflected on its board within 100 ms and never changes the other board.
- **SC-003**: In 10 out of 10 quits (normal, Ctrl+C, kill), neither managed Kanata is left running.
- **SC-004**: Killing one device's Kanata never interrupts typing or labels on the other device.
- **SC-005**: Every overlapping or unlimited device selection in a test set of configs is reported before any Kanata grabs a device.
- **SC-006**: Existing single-device commands (`just run`, `--config`, `--layout`) work unchanged.

## Assumptions

- The user's devices are a Dygma Defy (with a firmware layer that sends plain keys) and a mouse whose receiver exposes a keypad device ("Compx 2.4G Wireless Receiver Keyboard") and a mouse device ("Compx 2.4G Wireless Receiver") with side buttons; each device's Kanata config limits itself to its devices with Kanata's own device-selection settings.
- Writing those device selections stays in the user's Kanata configs; the overlay reads them for the overlap check but never edits the configs.
- Reading the Defy's own firmware layer (over its USB serial protocol) is out of scope; it could be a later feature.
- The laptop keyboard is not shown and not remapped (clarified in the request).
- Layout files for the Defy and the mouse keypad are provided by the user (KLE import, feature 001) or created with the later layout editor.
- One overlay instance remains the rule; running several overlay instances is not supported.
- The overlap check only covers configs the overlay knows. An attached Kanata that the user started with another config can't be checked.
