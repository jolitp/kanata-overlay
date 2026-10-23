# Implementation Plan: Managed Kanata

**Branch**: `003-managed-kanata` | **Date**: 2026-10-23 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/003-managed-kanata/spec.md`

## Summary

When `--config` is given and no Kanata answers on the address, the overlay starts Kanata
itself and supervises it.

- **Supervisor** (in `kanata-client`, no Qt): validates the config with `kanata --check`, spawns Kanata on a dedicated thread with a parent-death signal, waits until it answers on its TCP port, restarts it with a growing delay, gives up after 5 failures in 2 minutes, and keeps the last 1000 lines of output in memory.
- **Attach**: if a Kanata already answers, the overlay attaches and never stops it. If that Kanata disappears, a "Start Kanata" action switches to managed mode.
- **App**: a new `KanataState` QObject exposes the mode, the state text, the log and the actions. The header, the settings window and a new log window show them.

## Technical Context

**Language/Version**: Rust stable (edition 2024, MSRV 1.88); QML (Qt 6.11)

**Primary Dependencies**: `libc` (new: `prctl`, `getppid`, `kill`); the existing `kanata-tcp-protocol`

**Storage**: none (the log is in memory only, FR-011)

**Testing**: `cargo test -p kanata-client`, with pure tests (backoff, classification, probe) and process tests against `tests/fake-kanata.sh`, including PDEATHSIG; manual checks with real Kanata ([quickstart.md](quickstart.md) §3–§5)

**Target Platform**: Linux (`PR_SET_PDEATHSIG` is Linux-only, as is the whole app)

**Project Type**: Desktop app (Cargo workspace)

**Performance Goals**:
- connected in under 5 s, including Kanata's own 2 s start-up pause (SC-001);
- after a crash, back within 5 s (SC-003);
- a stopped child is gone within 2 s (SC-002).

**Constraints**:
- never root;
- Kanata only on 127.0.0.1;
- never strand the keyboard;
- Kanata's output never on disk or in logs.

**Scale/Scope**:
- 4 new modules in `kanata-client` (supervisor, backoff, probe, failure), one changed launcher, and one test script;
- 1 new QObject and 1 new QML window;
- small changes to `overlay_state.rs`, `SettingsWindow.qml` and `main.rs`.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | How |
|-----------|--------|-----|
| I. Rust owns state, QML renders | ✅ | The supervisor, backoff, failure rules and log live in `kanata-client`. `KanataState` bridges them. Events cross to Qt only via `qt_thread().queue(...)`. QML only shows properties and calls `restart()`, `startManaged()` and `shutdown()`. |
| II. Stable Kanata interfaces | ✅ | The probe and readiness checks use the TCP protocol through `kanata_client::protocol`. `--check` is a documented CLI flag. Parsing output to classify failures is the "less stable" part, so it is isolated in `failure.rs`. It degrades gracefully, because an unrecognised failure becomes `Exited` or `NoAnswer` and is retried with backoff. Captured Kanata 1.12 output is the test fixture. |
| III. Never strand the keyboard (NON-NEGOTIABLE) | ✅ | Parent-death signal with a race check, an orderly SIGTERM → SIGKILL on every quit path, and spawning only from the dedicated supervisor thread (research R1). Restart loops are capped at 5 in 2 minutes. Kanata runs as the user; no root, no `sudo`. A direct PDEATHSIG test plus 10 manual kill trials (SC-002). |
| IV. Plasma Wayland first | ✅ n/a | Not a display feature. |
| V. Testable without hardware | ✅ | Every supervisor path is tested with a fake Kanata script and needs no permissions. The keyboard-grab paths have manual notes (quickstart §3–§5). |
| VI. Key events are private | ✅ | Kanata's output, which can include key events in debug mode, stays in a memory ring. It never goes to `tracing`, disk or crash reports (FR-011), and is dropped on exit. TCP stays on 127.0.0.1 (`LaunchOptions` enforces it). |
| VII. Simple first | ⚠️ justified | One new dependency, `libc`; see Complexity Tracking. std threads and channels, no async runtime. No service-manager integration (Q2 = A). |

**Gate result**: PASS.

**Post-design re-check**: still PASS. The design adds no Qt dependency to
`kanata-client`. The attach/manage decision and every restart rule are unit-testable.

## Project Structure

### Documentation (this feature)

```text
specs/003-managed-kanata/
├── spec.md, plan.md, research.md, data-model.md, quickstart.md
├── contracts/interfaces.md
├── checklists/requirements.md
└── tasks.md               # /speckit-tasks
```

### Source Code (repository root)

```text
crates/kanata-client/
├── Cargo.toml             # + libc
├── src/
│   ├── lib.rs             # re-export Supervisor, SupervisorEvent, SessionState, Failure, probe
│   ├── launcher.rs        # + pre_exec PDEATHSIG and ppid race check; terminate(child)
│   ├── supervisor.rs      # NEW: supervisor thread, check → spawn → ready → run → restart
│   ├── backoff.rs         # NEW: pure backoff and give-up rules (injected clock)
│   ├── probe.rs           # NEW: is a Kanata answering at this address?
│   └── failure.rs         # NEW: Failure, classify(check output / exit + output)
└── tests/
    ├── supervisor.rs      # NEW: process tests with the fake Kanata
    └── fake-kanata.sh     # NEW: --check → exit $FAKE_CHECK; else python3 TCP server on --port

crates/app/
├── src/main.rs            # + --kanata PROGRAM; shutdown on SIGINT/SIGTERM/SIGHUP
├── src/kanata_state.rs    # NEW: KanataState QObject (mode, stateText, logText, actions)
├── src/overlay_state.rs   # connection loop reads the shared address
├── build.rs               # + kanata_state.rs, LogWindow.qml
└── qml/
    ├── Main.qml           # + KanataState, aboutToQuit → shutdown()
    ├── Board.qml          # header shows kanata.stateText
    ├── SettingsWindow.qml # + Kanata row: mode, Restart, Start, Show log
    └── LogWindow.qml      # NEW

README.md, CLAUDE.md       # managed vs attached, escape hatch
```

**Structure Decision**: The supervisor goes in `kanata-client`, next to the launcher
it extends. Its module doc already says "launcher should … restart-with-backoff and
`PR_SET_PDEATHSIG`". The app only bridges it.

## Implementation Notes

- **Order**:
  1. US1: PDEATHSIG, supervisor start/stop, probe, managed mode, header.
  2. US2: check, failures, backoff, log window, restart.
  3. US3: attach mode and the "Start Kanata" action.
- **Signals in the app**: SIGINT/SIGTERM/SIGHUP must call `QGuiApplication::quit()`, so that `aboutToQuit → shutdown()` runs. Use a self-pipe thread with `libc::signal` and `sigwait`, or simply rely on PDEATHSIG for those paths. The default handler kills the overlay, and PDEATHSIG then stops Kanata within milliseconds. **Decision: rely on PDEATHSIG for signals.** The orderly path covers window close and `Qt.quit()`. This keeps the change small (VII), and R1's PDEATHSIG test proves it.
- **`just run`**: unchanged. Because it passes `--config`, it now starts Kanata when none is running. `just fake-kanata` and `just kanata` still lead to attach mode.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| New dependency `libc` | `prctl(PR_SET_PDEATHSIG)`, `getppid()` and `kill(pid, SIGTERM)` are required by constitution III ("use parent-death signals, terminate on exit"). std has no API for any of them. | `Child::kill()` only sends SIGKILL and doesn't cover the overlay crashing. `rustix`/`nix` are larger wrappers around the same calls. |
