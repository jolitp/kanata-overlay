# Feature Specification: Managed Kanata

**Feature Branch**: `003-managed-kanata`

**Created**: 2026-10-23

**Status**: Draft

**Input**: User description: "Managed Kanata: the overlay starts and supervises Kanata itself"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - One command starts both (Priority: P1)

Today the user opens two terminals: one for Kanata, one for the overlay. With this
feature they start only the overlay, pointing it at their Kanata config, and the overlay
starts Kanata for them, connects to it, and shows the layers and labels as before. When
they quit the overlay, Kanata stops too and the keyboard behaves normally again.

**Why this priority**: This is the feature. Everything else (restarts, logs, attach mode)
builds on the overlay owning Kanata's lifetime.

**Independent Test**: With no Kanata running, start the overlay with a config. Within a
few seconds the header shows "connected" and the remapping works (e.g. Caps acts as Esc
on tap). Quit the overlay: within 2 seconds no Kanata process remains and Caps is Caps
again.

**Acceptance Scenarios**:

1. **Given** no Kanata is running and a valid config is given, **When** the overlay starts, **Then** it starts Kanata with that config, connects to it and shows "connected" within 5 seconds.
2. **Given** the overlay started Kanata, **When** the user quits the overlay normally (window closed, Ctrl+C in the terminal, logout), **Then** Kanata stops within 2 seconds and the keyboard is released.
3. **Given** the overlay started Kanata, **When** the overlay crashes or is killed, **Then** Kanata stops too and never keeps the keyboard grabbed without an overlay.
4. **Given** the overlay started Kanata, **When** the user looks at the header, **Then** it shows that Kanata is managed by the overlay (as opposed to attached).

---

### User Story 2 - Kanata problems are visible and recoverable (Priority: P2)

Kanata can fail to start (wrong config, missing permissions, keyboard not found) or
stop later. The user wants to see why, without hunting through terminals, and wants a
crashed Kanata to come back on its own when it makes sense, but not to loop forever.

**Why this priority**: Without this, a failed or crashed Kanata looks like a blank
overlay with "not connected", which is confusing. It is second because the happy path
(US1) already delivers value.

**Independent Test**: Start with a config that has an error: the overlay explains the
error in a notice and offers the log. Start with a valid config, then kill Kanata from
outside: the overlay restarts it and says so. Make Kanata fail repeatedly: after a few
attempts the overlay stops retrying and says why.

**Acceptance Scenarios**:

1. **Given** the config is invalid, **When** Kanata fails to start, **Then** the overlay shows a notice with Kanata's own error summary (first error line) and does not retry until the user asks for a restart.
2. **Given** the user lacks the permissions Kanata needs, **When** Kanata fails to start, **Then** the notice says so in plain words and points to the setup instructions; the overlay does not retry.
3. **Given** Kanata was running and exits unexpectedly, **When** the overlay notices, **Then** it restarts Kanata after a short delay that grows with each consecutive failure, and shows "restarting Kanata (attempt N)".
4. **Given** Kanata keeps failing, **When** it has failed 5 times within 2 minutes, **Then** the overlay stops restarting and shows "Kanata keeps stopping; see the log".
5. **Given** Kanata is managed, **When** the user opens the Kanata log, **Then** they see Kanata's recent output (at least the last 500 lines) in a window they can scroll and copy from.
6. **Given** Kanata is managed, **When** the user asks to restart Kanata (e.g. after editing the config), **Then** the overlay stops and starts it again and the labels follow the new config.

---

### User Story 3 - Attach to a Kanata that is already running (Priority: P3)

Some users already run Kanata themselves (a system service, a login item, a terminal).
The overlay must not start a second one; it should use the running one, as it does today.

**Why this priority**: It keeps today's workflow working and avoids two Kanata instances
fighting over the keyboard. It is lower priority because users of that setup can keep
using today's behaviour until this lands.

**Independent Test**: Start Kanata yourself on the overlay's port, then start the
overlay: it connects without starting another Kanata, shows "attached", and quitting the
overlay leaves your Kanata running.

**Acceptance Scenarios**:

1. **Given** a Kanata already answers on the configured address, **When** the overlay starts, **Then** it attaches to it, starts nothing, and shows "attached".
2. **Given** the overlay is attached, **When** the user quits the overlay, **Then** the running Kanata is left untouched.
3. **Given** the overlay is attached and that Kanata stops, **When** the overlay notices, **Then** it shows "not connected" and keeps trying to reconnect; it does not start its own Kanata by itself.
4. **Given** the overlay is attached, that Kanata has stopped, and a config was given, **When** the user asks the overlay to start Kanata, **Then** the overlay starts its own Kanata and from then on manages it (US1, US2).

---

### Edge Cases

- No config is given: the overlay cannot start Kanata; it attaches if a Kanata is running, otherwise shows "not connected" as today, and no "start Kanata" action is offered.
- The Kanata program is not installed or not found: a notice says so; nothing is retried.
- The configured port is taken by something that is not Kanata: the overlay picks another free local port for its own Kanata and connects there.
- Kanata starts but never answers on its port (stuck, wrong build without the network server): after 10 seconds the overlay stops it, shows a notice, and treats it as a failed start.
- Kanata is grabbing the keyboard when the overlay hangs (not crashed, just frozen): Kanata keeps running, which is acceptable because the keyboard still works through Kanata; only an overlay that is gone must take Kanata with it.
- The user kills Kanata on purpose (e.g. its own exit key combination): treated like any unexpected exit (restart with backoff), because the overlay cannot tell the difference reliably. [Assumption, see below.]
- Several overlays started at once: only one may manage a Kanata for the same keyboard; a second overlay attaches to the first one's Kanata.
- Kanata's output contains key presses (e.g. in debug mode): the log window shows it live, but it is never written to disk (constitution VI).

## Requirements *(mandatory)*

### Functional Requirements

**Starting and stopping**

- **FR-001**: When a config is given and no Kanata answers on the configured address, the overlay MUST start Kanata itself with that config and connect to it ("managed").
- **FR-002**: The Kanata it starts MUST run as the current user (never with elevated rights) and MUST only accept connections from the same machine.
- **FR-003**: A managed Kanata MUST stop when the overlay exits for any reason, including a crash or being killed, so that it never keeps the keyboard grabbed without an overlay (constitution III).
- **FR-004**: The overlay MUST consider Kanata started only once it answers on its port; if it does not answer within 10 seconds, the start has failed.
- **FR-005**: The overlay MUST show in its header whether Kanata is managed, attached or not connected.

**Failures and restarts**

- **FR-006**: If a managed Kanata exits unexpectedly, the overlay MUST restart it with a growing delay (starting around 1 second, at most 30 seconds).
- **FR-007**: After 5 failures within 2 minutes, the overlay MUST stop restarting and show a notice until the user asks for a restart.
- **FR-008**: Failures that a restart cannot fix (invalid config, missing permissions, program not found) MUST NOT be retried; the notice MUST say which of these it is in plain words.
- **FR-009**: Users MUST be able to restart a managed Kanata on demand.

**Logs**

- **FR-010**: The overlay MUST keep Kanata's recent output (at least the last 500 lines) in memory and let the user view it in a scrollable, copyable window.
- **FR-011**: Kanata's output MUST NOT be written to disk, logs or crash reports by the overlay, and MUST be discarded when the overlay exits (constitution VI).

**Attach mode**

- **FR-012**: If a Kanata already answers on the configured address, the overlay MUST attach to it and MUST NOT start another one.
- **FR-013**: An attached Kanata MUST NOT be stopped or restarted by the overlay.
- **FR-014**: When an attached Kanata disappears, the overlay MUST NOT start its own Kanata on its own; it MUST offer the user a "start Kanata" action (available only when a config was given), which switches to managed mode.

### Key Entities

- **Kanata session**: One Kanata the overlay is connected to: how it came to be (managed or attached), its address, its state (starting, running, restarting, failed, stopped) and, if managed, the failure history used for backoff.
- **Kanata log**: The recent lines of a managed Kanata's output, in memory only.
- **Failure**: Why a start or run ended: invalid config, missing permissions, program not found, no answer on the port, or unexpected exit.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Starting the overlay with a config brings up a working, connected Kanata in under 5 seconds on a typical machine.
- **SC-002**: In 10 out of 10 trials (normal quit, Ctrl+C, kill, crash), no Kanata started by the overlay is left running 2 seconds after the overlay is gone.
- **SC-003**: A crashed Kanata is back and connected within 5 seconds of the first crash.
- **SC-004**: A user can find out why Kanata failed to start (config error, permissions, not installed) from the overlay alone, without a terminal, in under 1 minute.
- **SC-005**: With an already running Kanata, starting and quitting the overlay 10 times never starts a second Kanata and never stops the existing one.

## Assumptions

- The Kanata config path comes from the existing `--config` option (feature 002); without it there is nothing to start.
- The Kanata program is taken from the normal program search path unless the user names one explicitly.
- Running Kanata as a per-user background service is out of scope (clarified 2026-10-23); it can be a later feature. A Kanata run that way is used through attach mode (US3).
- The overlay cannot tell a deliberate Kanata exit from a crash, so both are treated as unexpected exits.
- Permissions setup itself (groups, device rules) stays a manual, one-time step documented in the README; doing it from the app is the later "Desktop integration" feature.
- The log window is a plain, read-only view; filtering or saving logs is out of scope.
