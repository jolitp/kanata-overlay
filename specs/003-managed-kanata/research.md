# Research: Managed Kanata

**Feature**: [spec.md](spec.md) | **Plan**: [plan.md](plan.md) | **Date**: 2026-10-23

Kanata facts are from the Kanata 1.12.0 source in nixpkgs and from running the dev
shell's `kanata` binary.

## R1. Kanata must never outlive the overlay (constitution III, FR-003)

- **Decision**: Three layers of protection:
  1. **Parent-death signal**. The child is spawned with a `pre_exec` hook that calls `prctl(PR_SET_PDEATHSIG, SIGTERM)`. It then checks `getppid()` against the expected parent PID and exits immediately if the parent already died during the fork.
  2. **Orderly stop** when the app quits normally (window closed, `Qt.quit()`, `aboutToQuit`): the overlay sends SIGTERM, waits up to 1.5 s, then sends SIGKILL and reaps the child. When the overlay itself gets SIGINT, SIGTERM or SIGHUP (Ctrl+C, logout), the default handler ends it, and step 1 stops Kanata within milliseconds (plan › Implementation Notes).
  3. **Dedicated supervisor thread**. `PR_SET_PDEATHSIG` fires when the *thread* that forked the child exits, not the process. So every spawn, including restarts, happens on one supervisor thread that lives as long as the app.
- **Why SIGTERM is safe**: Kanata's Linux signal handler (`src/oskbd/linux.rs`, `handle_signals`) runs the default handler for SIGINT and SIGTERM, so the process exits. The kernel then closes its evdev grab and destroys its uinput device, which also releases any keys the virtual device was holding. Even SIGKILL leaves the keyboard usable, for the same reason.
- **Overlay hangs** (alive, but stuck): Kanata keeps running. That is acceptable, because the keyboard still works through Kanata (spec edge case).
- **Alternatives considered**:
  - A `systemd --user` scope with `BindsTo=`. It is the later service feature (Q2 = A).
  - A process group plus `killpg` on exit. That covers normal quit only, not a crash.
  - Polling the parent PID from a Kanata wrapper. That needs a wrapper process and is more fragile.

## R2. Telling managed from attached (FR-001, FR-012)

- **Decision**: At start-up, probe the configured address (default `127.0.0.1:7070`):
  1. connect with a 1 s timeout;
  2. send `RequestCurrentLayerName {}`;
  3. wait up to 1 s for any line that parses as a `kanata_client::protocol::ServerMessage`.

  **Changed 2026-10-23**: step 2 was dropped. Kanata 1.12 greets every new client with `LayerChange`, so the probe only listens; sending a request and hanging up made Kanata log "Broken pipe" (found in T024).

  If a message arrives, the overlay **attaches**. If nothing is listening, the overlay **manages** (only if `--config` was given). If something answers but not with Kanata's protocol, the overlay manages on a free port (`free_port()`; spec edge case).
- **Rationale**:
  - Real Kanata answers the request with `CurrentLayerName`, and `just fake-kanata` sends `LayerChange` on connect. Both count as Kanata.
  - Port `7070` is the address everybody already uses (`just kanata`), so a managed Kanata binds there when it is free. That also makes a second overlay attach instead of starting a second Kanata (spec edge case "several overlays").

## R3. Readiness (FR-004)

- **Decision**: After spawning, repeat the R2 probe every 200 ms against the managed address until it succeeds (**ready**), the child exits (**failed**), or 10 s pass. On timeout, stop the child (R1 step 2) and record `Failure::NoAnswer`.
- **Note**: Kanata 1.12 deliberately sleeps 2 s at start ("Please release all keys…", the `--nodelay` option skips it). We keep the sleep, because it protects against stuck keys. SC-001's 5 s budget allows for it.

## R4. Classifying failures (FR-008)

| Failure | How it is detected | Retried? |
|---------|--------------------|----------|
| `NotInstalled` | `spawn` returns `ErrorKind::NotFound` | no |
| `InvalidConfig { summary }` | Before each start, run `kanata --cfg PATH --check --no-wait`, with the parent-death signal like every Kanata the app starts (constitution III). It doesn't grab anything. Exit ≠ 0 → take the first `help:` line from its output (e.g. "Unknown key/action: nosuchkey"), else the first `[ERROR]` line. | no (only a user restart or a config reload retries) |
| `Permissions` | The child exits before ready, and its output contains `Permission denied` (opening `/dev/input/*` or `/dev/uinput`) | no |
| `PortInUse` | The child exits before ready with `Address already in use` | re-probe the configured address: a Kanata there → attach instead (another overlay won the race); otherwise retry once on a fresh `free_port()` |
| `NoAnswer` | Readiness timeout | yes (backoff) |
| `Exited { code }` | The child exits after being ready | yes (backoff) |

- **Rationale**: `--check` is exact and cheap: we measured exit 1 plus the `help:` line on a bad config. The other patterns are stable strings from the OS or Kanata (`src/kanata/linux.rs`: "failed to open keyboard device(s): …").
- No keyboard found: Kanata logs "no keyboard devices found; kanata is waiting" and keeps running, so it is **not** a failure. The line shows in the log, the readiness timer pauses, and the state is "waiting for a keyboard".

## R5. Backoff (FR-006, FR-007)

- **Delays**: 1, 2, 4, 8, 16, then 30 s, capped. The count resets after 60 s of healthy running.
- **Give up**: on the 5th failure within a sliding 2-minute window, stop and set `GaveUp`. Only a user restart clears it.
- **Testing**: a pure `Backoff` struct with an injected clock (`Instant` values passed in) is unit-tested in `kanata-client`.

## R6. Log capture (FR-010, FR-011)

- **Capture**:
  - Two reader threads read the child's stdout and stderr line by line.
  - ANSI colour codes are stripped.
  - Lines go into a `VecDeque<String>` capped at 1000 lines (≥ 500, FR-010), shared as `Arc<Mutex<…>>`.
- **Delivery to the UI**: The supervisor sends a "log changed" event. The app publishes the joined text to QML at most every 250 ms, via `qt_thread().queue`.
- **Privacy**: The log lives only in memory and is dropped on exit. It is never passed to `tracing` (constitution VI), because Kanata's debug output can contain key events.
- **Alternatives**: Writing to a file under `$XDG_STATE_HOME` was rejected by FR-011.

## R7. Where the code lives

- **`crates/kanata-client`** (no Qt): extend `launcher.rs` with `pre_exec` and PDEATHSIG, and add:
  - `supervisor.rs`: the supervisor thread, start/check/ready/restart/stop, events;
  - `backoff.rs`: the pure backoff logic;
  - `probe.rs`: the R2 probe;
  - `failure.rs`: `Failure` and output classification.
- **New dependency**: `libc`, for `prctl`, `getppid` and `kill(SIGTERM)`. std has no safe API for these. `rustix` would also work, but `libc` is smaller, and it is already in `Cargo.lock` (0.2.190) as a transitive dependency, so no new code is added to the build.
- **`crates/app`**:
  - a new `kanata_state.rs` QObject owns the supervisor and exposes mode, state, log and actions;
  - `overlay_state.rs`'s connection loop reads the current address from a shared value, because a managed Kanata may sit on a free port.

## R8. Testing without Kanata (constitution V)

- **Pure unit tests**: `Backoff`, `Failure::classify(output)` with captured real Kanata output as fixtures, and the probe against a local `TcpListener`.
- **Process tests**: in `crates/kanata-client/tests/supervisor.rs`, with a fake Kanata (`tests/fake-kanata.sh`). It handles `--check` (exit 0 or 1 depending on an env var) and otherwise runs a tiny TCP server on the `--port` it was given, using `python3`, which the dev shell has. The tests cover:
  - start → ready;
  - a crash → restart with backoff;
  - giving up after 5 failures;
  - `InvalidConfig`;
  - `NotInstalled`;
  - stop → the child is gone within 1.5 s;
  - **PDEATHSIG**: spawning from a thread that then exits must kill the child. This is a direct test of the R1 mechanism.
- **Manual tests** with real Kanata, for the keyboard grab: quickstart §3, kill/crash scenarios, and SC-002's 10 trials. The escape hatch if a test misbehaves: the keyboard still works through Kanata, and `pkill -x kanata` from a terminal stops it.
