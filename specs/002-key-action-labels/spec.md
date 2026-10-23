# Feature Specification: Key Action Labels

**Feature Branch**: `002-key-action-labels`

**Created**: 2026-10-23

**Status**: Draft

**Input**: User description: "show each key's action from the .kbd file"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - See what each key does on the active layer (Priority: P1)

A Kanata user has the overlay open. Instead of the physical key names they see today
(`q`, `caps`, `spc`…), each key shows what it does on the layer that is active right
now. When they hold Caps Lock to enter their `nav` layer, the keys `h j k l` change to
arrows; when they let go, the overlay shows the base layer again.

**Why this priority**: This is the overlay's purpose. Without it the overlay only shows
the physical board and the layer's name, which the user already knows.

**Independent Test**: Load the example config, start the overlay, and compare the labels
on the base layer and on the `nav` layer with the config file: every bound key shows the
action written for it.

**Acceptance Scenarios**:

1. **Given** the overlay is connected and the base layer is active, **When** the user looks at the overlay, **Then** each bound key shows its action from that layer (e.g. `a` shows "a", Caps Lock shows "Esc" with a hold marker).
2. **Given** the user switches to another layer, **When** the layer change arrives, **Then** the labels change to that layer's actions within a fraction of a second (SC-002).
3. **Given** a key is transparent on the active layer, **When** the overlay shows that layer, **Then** the key shows the action it falls through to, visibly marked as inherited.
4. **Given** a key does nothing on the active layer, **When** the overlay shows that layer, **Then** the key is shown as blank and visibly "disabled".
5. **Given** a key on the board has no Kanata binding or is not part of the config's source keys, **When** any layer is shown, **Then** it keeps its printed legend and the existing "unbound" outline.

---

### User Story 2 - Readable labels for complex actions (Priority: P2)

Many keys do more than type one character: tap for one thing and hold for another,
switch layers, run a macro or send a shortcut. The user wants to recognise these at a
glance without reading Kanata syntax.

**Why this priority**: Simple keys already work with User Story 1. Complex keys are
where users most often forget what they configured, so readable labels add most of the
remaining value.

**Independent Test**: A config with one example of each supported action kind shows the
short label for each kind defined in FR-006.

**Acceptance Scenarios**:

1. **Given** a key with different tap and hold actions, **When** it is shown, **Then** the tap action is the label and a small marker shows that the key also has a hold action; the hold action itself is not shown.
2. **Given** a key that switches or holds a layer, **When** it is shown, **Then** the label names the target layer with a layer symbol.
3. **Given** a key bound to an alias, **When** it is shown, **Then** the label describes the alias's action, not the alias name; the alias name is available as secondary text.
4. **Given** a key with a modifier combination (e.g. Ctrl+C), **When** it is shown, **Then** the label uses modifier symbols (e.g. "⌃C").
5. **Given** an action the overlay cannot summarise, **When** it is shown, **Then** the label shows a shortened form of the configured text instead of nothing.

---

### User Story 3 - Labels follow config changes (Priority: P3)

The user edits their Kanata config and reloads Kanata. The overlay picks up the new
actions without being restarted.

**Why this priority**: Users tune their configs often; restarting the overlay each time
is a small but frequent annoyance.

**Independent Test**: Change one key's action in the config, reload Kanata, and see the
overlay update.

**Acceptance Scenarios**:

1. **Given** the overlay is running, **When** Kanata reports that it reloaded its config, **Then** the overlay re-reads the file it was given and updates all labels within 2 seconds.
2. **Given** the new config has an error the overlay cannot read, **When** it reloads, **Then** the overlay keeps the last good labels and shows a notice explaining what failed.
3. **Given** Kanata reloads a *different* file (e.g. switching between several configs), **When** the reload is reported, **Then** the overlay keeps reading the file it was given and shows a notice that Kanata is now using another file.

---

### Edge Cases

- No config path is given: the overlay shows the physical legends (today's behaviour), without a notice.
- The given config cannot be found or read at start: the overlay shows the physical legends and a notice saying which file it tried.
- The active layer name is not in the config (e.g. Kanata runs a different config): the first layer is labelled, with a notice naming the unknown layer.
- A layer has fewer or more entries than the config's source keys: labels are applied position by position as far as they go; the mismatch is reported once.
- Two physical keys on the board bind to the same Kanata key: both show the same action.
- An alias refers to another alias, or to itself: aliases are followed to a reasonable depth; loops show the alias name, and a notice names the loop.
- Very long labels (macros, unicode strings): the label is shortened to fit the key and never overflows it.
- Config features the overlay does not understand (e.g. includes, templates, per-layer key maps): the parts it understands are labelled; the rest fall back to legends, and the notice lists what was skipped.
- Kanata is not connected: the overlay labels the first layer of the config (Kanata's default layer) so the board is still useful.

## Requirements *(mandatory)*

### Functional Requirements

**Reading the config**

- **FR-001**: The app MUST read the Kanata config file whose path the user passes on the command line for that run. The path is not saved; without it, the overlay shows the printed legends as today.
- **FR-002**: The app MUST understand the source key list, the layer definitions and aliases, which together decide what each key does on each layer.
- **FR-003**: Config parts the app does not understand MUST NOT stop it from labelling the parts it does understand; they are reported in a notice.
- **FR-004**: Reading the config MUST never change it or any other file.

**Showing actions**

- **FR-005**: Each bound physical key MUST show the action assigned to it on the active layer, matched through the key's Kanata name and its position in the source key list.
- **FR-006**: Actions MUST be summarised as short labels:
  - plain key → its symbol or short name (e.g. "a", "⏎", "←", "Esc");
  - modifier combination → modifier symbols + key (e.g. "⌃C", "⌃⇧T");
  - layer switch / momentary layer → layer symbol + layer name;
  - tap-hold → the tap action, plus a marker that the key has a hold action (see US2 scenario 1);
  - transparent → the inherited action, marked as inherited;
  - no-op → blank, marked as disabled;
  - alias → the alias's resolved action;
  - anything else → shortened raw text.
- **FR-007**: Labels MUST fit inside their key at every overlay scale; text that does not fit is shortened, not overflowing.
- **FR-008**: Keys without a Kanata binding, or whose Kanata name is not in the source key list, MUST keep their printed legend and "unbound" styling.
- **FR-009**: When the active layer changes, labels MUST update to the new layer (SC-002).
- **FR-010**: Before Kanata reports a layer (or when it is not connected), the first layer in the config MUST be shown.

**Keeping up to date**

- **FR-011**: When Kanata reports a config reload, the app MUST re-read the config and update the labels.
- **FR-012**: If a re-read fails, the app MUST keep the previous labels and show a notice.

**Privacy and safety**

- **FR-013**: The app MUST NOT log, store or transmit the config's contents beyond what it needs in memory to show labels; notices name files and problems, not full config text.

### Key Entities

- **Kanata config**: The user's `.kbd` file. Contains the ordered list of source keys, one or more named layers (each an ordered list of actions matching the source keys), and named aliases.
- **Layer**: A named, ordered list of actions, one per source key; the first layer is the default.
- **Action**: What a key does: a plain key, a modifier combination, a layer change, a tap-hold pair, transparent, no-op, an alias reference, or something more complex.
- **Key label**: The short text (and style: normal, inherited, disabled, unbound) shown on one physical key for the active layer.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: With the example config, 100% of bound keys on both layers show the action written in the config.
- **SC-002**: After a layer change, the new labels appear within 100 ms of the layer change being reported.
- **SC-003**: After a config reload, updated labels appear within 2 seconds.
- **SC-004**: A user can tell what any key on the active layer does by looking at the overlay alone, without opening the config, for every action kind in FR-006 (checked with a sample config that has one of each).
- **SC-005**: A config with unsupported parts still gets labels for all keys whose actions the app understands; it never shows an empty board because of one unknown construct.

## Assumptions

- The physical layout's per-key Kanata names (from feature 001's layout import) are the link between physical keys and config source keys; binding unbound keys is the later "Layout editor" feature.
- The overlay understands the commonly used subset of Kanata's config language (source keys, layers, aliases, the usual action forms). Full compatibility with every Kanata construct is out of scope (constitution VII: hand-written parser for the subset the overlay needs).
- Live highlighting of pressed keys or of a tap-hold key's current state is out of scope; that is the later "Live key highlighting" feature.
- Labels use the existing key cap look; only the text and a few styles (inherited, disabled) are added.
- The overlay only reads the config; it never edits it or tells Kanata to reload.
- The config path is given per run (clarified 2026-10-23); saving it in settings or following Kanata to another file can come later.
- Showing a tap-hold key's hold action (e.g. a small second line or a legend view) is out of scope (clarified 2026-10-23); the hold marker only says that one exists.
