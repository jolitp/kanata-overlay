<!--
Sync Impact Report
- Version change: template → 1.0.0 (initial ratification)
- Principles added: I–VII (all new)
- Sections added: Technology Constraints, Development Workflow, Governance
- Templates requiring updates: none (plan-template's "Constitution Check" reads this file)
- Deferred TODOs: none
-->

# kanata-overlay Constitution

kanata-overlay shows an on-screen picture of the user's physical keyboard with the
actions of the active Kanata layer, and can launch and manage Kanata itself. It is
a Linux desktop app written in Rust with a Qt 6 / QML front end, targeting KDE Plasma.

## Core Principles

### I. Rust Owns State, QML Renders It

All application state and logic (Kanata connection, config parsing, layouts,
process management) lives in Rust library crates. QML only presents state exposed
through cxx-qt QObjects and forwards user intent back through invokables.
Background threads MUST update QObjects only via `qt_thread().queue(...)`.
Rationale: keeps logic testable without a GUI and keeps the C++/QML surface small.

### II. Talk to Kanata Through Stable Interfaces

The TCP protocol (via the upstream `kanata-tcp-protocol` crate, pinned exactly
because it does not follow semver) is the primary channel. Anything less stable,
such as parsing Kanata's debug output, MUST be optional, isolated behind a trait,
degrade gracefully when it stops matching, and be covered by captured fixtures per
supported Kanata version. Re-implementing Kanata's wire types is not allowed.

### III. Never Strand the Keyboard (NON-NEGOTIABLE)

A Kanata process started by the app MUST NOT outlive it: use parent-death signals,
terminate on exit, and cap restart loops. The app MUST NOT require root at runtime;
privileged steps (group membership, udev rules) are one-time setup performed through
polkit (`pkexec`), never `sudo` from the GUI. Failures that could leave the keyboard
grabbed or unusable are treated as release blockers.

### IV. KDE Plasma on Wayland Is the Primary Target

The overlay MUST work as an always-on-top, click-through surface on KDE Plasma
Wayland using layer-shell (LayerShellQt). X11 support is best-effort via window
hints. Other compositors are welcome but never at the expense of Plasma behaviour.

### V. Testable Without Hardware or Kanata

Every library crate MUST be testable with `cargo test` alone: fake TCP servers for
Kanata, fixture files for KLE layouts, configs and logs. Features touching input
devices or the compositor need a manual test note in their spec. `just check`
(format, clippy with `-D warnings`, tests) MUST pass before merge.

### VI. Key Events Are Private

Pressed-key data (from evdev, debug logs or any other source) MUST stay in memory,
MUST NOT be written to disk, logs or crash reports, and MUST NOT leave the machine.
The Kanata TCP port binds to 127.0.0.1 only.

### VII. Simple First

Prefer the smallest thing that works: std threads before an async runtime,
hand-written parsers for the small subset of `.kbd` the overlay needs before
depending on Kanata's full parser, JSON before a database. New dependencies and
abstractions need a stated reason in the plan's Complexity Tracking table.

## Technology Constraints

- Language: Rust (stable, edition 2024, MSRV 1.88) in a Cargo workspace.
- UI: Qt 6 with QML via cxx-qt 0.10.x; C++ only for thin shims (e.g. LayerShellQt).
- Layouts: KLE JSON import (`kle-serial`) plus the app's own versioned JSON format.
- Environment: Nix flake dev shell (`nix develop`), tasks via `just`.
- Platform: Linux only.

## Development Workflow

- Work is spec-driven with Spec Kit: `/speckit-specify` → `/speckit-clarify`
  (optional) → `/speckit-plan` → `/speckit-tasks` → `/speckit-implement`.
- Each feature lives on its own branch with its spec under `specs/`.
- Plans MUST include a Constitution Check against the principles above.
- Commits are small and pass `just check`.

## Governance

This constitution takes precedence over other practices in this repository.
Amendments are made by editing this file through `/speckit-constitution`, with a
Sync Impact Report and a semantic version bump: MAJOR for removing or redefining a
principle, MINOR for adding one or materially expanding guidance, PATCH for wording.
Reviews of specs, plans and code check compliance; deviations must be justified in
the plan's Complexity Tracking table. Runtime guidance for agents lives in `CLAUDE.md`.

**Version**: 1.0.0 | **Ratified**: 2026-10-15 | **Last Amended**: 2026-10-15
