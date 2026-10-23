---
description: "Task list for 003-managed-kanata"
---

# Tasks: Managed Kanata

**Input**: Design documents from `/specs/003-managed-kanata/`

**Prerequisites**: [plan.md](plan.md), [spec.md](spec.md), [research.md](research.md), [data-model.md](data-model.md), [contracts/interfaces.md](contracts/interfaces.md), [quickstart.md](quickstart.md)

**Tests**: Included for `kanata-client`, as the plan and constitution III and V require: pure tests plus process tests against a fake Kanata, including a direct parent-death test. Keyboard-grab behaviour is checked by hand ([quickstart.md](quickstart.md) §3–§5).

**Organization**: Tasks are grouped by user story, so each story can be built and tested on its own.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependency on unfinished tasks).
- **[Story]**: The user story the task belongs to (US1–US3 from spec.md).

## Conventions for every task

- Work inside `nix develop`. `just check` must pass before each commit.
- **Constitution III**: a managed Kanata must never outlive the overlay. Spawn Kanata *only* on the supervisor thread (research R1: `PR_SET_PDEATHSIG` is tied to the spawning thread).
- **Constitution VI**: Kanata's output never goes to `tracing`, disk or crash reports, only to the in-memory log.
- Never use real Kanata in automated tests. Use `crates/kanata-client/tests/fake-kanata.sh`.
- The app crate's agent smoke tests must not run real Kanata either (it grabs the keyboard). Use `just fake-kanata` or the fake script via `--kanata`.

---

## Phase 1: Setup

- [ ] T001 Add `libc = "0.2"` to `[workspace.dependencies]` in `Cargo.toml` (comment: "prctl(PR_SET_PDEATHSIG), kill: constitution III"), and `libc.workspace = true` to `crates/kanata-client/Cargo.toml`.
- [ ] T002 Create empty modules `crates/kanata-client/src/{supervisor.rs,backoff.rs,probe.rs,failure.rs}` with module docs, declare them in `crates/kanata-client/src/lib.rs`, and update the `launcher.rs` module doc. The "Still to do" paragraph now points to `supervisor.rs`.
- [ ] T003 [P] Write `crates/kanata-client/tests/fake-kanata.sh` (executable, `#!/bin/sh`). It:
  - with `--check` among its arguments: prints `[ERROR]   × Error in configuration` and `  help: Unknown key/action: nosuchkey` and exits 1 if `FAKE_CHECK=fail`, else prints `[INFO] config file is valid` and exits 0;
  - with `FAKE_START=permission`: prints `failed to open keyboard device(s): Permission denied (os error 13)` to stderr and exits 1;
  - with `FAKE_START=never-answer`: sleeps forever;
  - otherwise: parses `--port 127.0.0.1:N` and `exec`s `python3 -c` with a server that accepts connections, sends `{"LayerChange":{"new":"base"}}\n`, prints `[INFO] fake kanata ready` to stdout, and runs until killed. With `FAKE_CRASH_AFTER=S`, it exits with code 3 after S seconds.

---

## Phase 2: Foundational (Blocking Prerequisites)

**⚠️ CRITICAL**: No user-story work can begin until this phase is complete.

- [ ] T004 Extend `crates/kanata-client/src/launcher.rs` (research R1):
  - add `pub parent_death_signal: bool` to `LaunchOptions`, default `true`;
  - when it is set, `command()` installs `std::os::unix::process::CommandExt::pre_exec`. The hook records `let parent = std::process::id()` before the fork, then in the child calls `libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM)` and returns an error if `libc::getppid() != parent` (the parent already died). Add a `// SAFETY:` comment: only async-signal-safe calls are made;
  - add `pub fn terminate(child: &mut Child, grace: Duration) -> io::Result<ExitStatus>`. It sends SIGTERM via `libc::kill(child.id() as i32, libc::SIGTERM)`, polls `try_wait` every 50 ms for up to `grace`, then calls `child.kill()` and `child.wait()`.

  Keep the existing `builds_expected_args` test passing.
- [ ] T005 [P] Write tests in `crates/kanata-client/tests/supervisor.rs` for the launcher:
  - `child_dies_with_its_spawning_thread`: spawn `sleep 30` via `LaunchOptions { kanata_bin: "sleep" … }` (or a `Command` built the same way with the same `pre_exec`) from a `std::thread` that returns right after spawning; within 1 s, `kill(pid, 0)` fails (the child is gone or a zombie reaped by the test);
  - `terminate_stops_within_grace`: `terminate(child, 1.5 s)` returns in under 2 s;
  - `terminate_kills_a_child_that_ignores_sigterm`: `sh -c 'trap "" TERM; sleep 30'` is still killed.
- [ ] T006 [P] Implement `crates/kanata-client/src/probe.rs` (research R2): `pub enum Probe { Kanata, Nothing, NotKanata }` and `pub fn probe(addr: SocketAddr, timeout: Duration) -> Probe`. It connects with `timeout`; a refused or timed-out connection gives `Nothing`. It then sends `ClientMessage::RequestCurrentLayerName {}` and waits up to `timeout` for one line. If the line parses as `ServerMessage` → `Kanata`, else (garbage, closed, or silent) → `NotKanata`. Re-export it from `lib.rs`. Unit tests use local `TcpListener`s for each case.
- [ ] T007 [P] Add `--kanata PROGRAM` (field `kanata: Option<PathBuf>`) to `Args` in `crates/app/src/main.rs`, and document it in the module doc.
- [ ] T008 In `crates/app/src/overlay_state.rs`, make the connection address shared. `spawn_connection` takes an `Arc<Mutex<String>>`, which is stored in `OverlayStateRust` as `pub(crate) address: Arc<Mutex<String>>` and initialised from `args.address`. Each loop iteration reads it, and `request_layer` reads it too. Add `pub fn set_address(&self, addr: String)` for `KanataState`.
- [ ] T009 Create the `KanataState` QObject skeleton in `crates/app/src/kanata_state.rs` (contracts/interfaces.md):
  - properties `mode: QString` (`"none"`), `state_text: QString`, `log_text: QString`, `can_restart: bool` and `can_start: bool`;
  - empty invokables `start()`, `restart()`, `start_managed()` and `shutdown()`;
  - `#[qml_element]`.

  Add it to `.files([...])` in `crates/app/build.rs`, declare `mod kanata_state;` in `main.rs`, and instantiate it in `crates/app/qml/Main.qml` as `KanataState { id: kanataState }`. Connect `Qt.application.aboutToQuit` to `kanataState.shutdown()`.

**Checkpoint**: The T005 PDEATHSIG and terminate tests and the T006 probe tests pass. The app builds and behaves as before.

---

## Phase 3: User Story 1 - One command starts both (Priority: P1) 🎯 MVP

**Goal**: With `--config` and no Kanata answering, the overlay starts Kanata, connects to it and shows "managed". Kanata stops whenever the overlay ends.

**Independent Test**: quickstart.md §3. No Kanata running, then `just run`: "managed" within 5 s and remapping works. Every quit path leaves no `kanata` process within 2 s.

### Tests for User Story 1

- [ ] T010 [P] [US1] Add process tests to `crates/kanata-client/tests/supervisor.rs`, using `fake-kanata.sh` as `kanata_bin`:
  - `starts_and_becomes_ready`: `Supervisor::start` emits `State(Starting)`, then `Ready(addr)` and `State(Running)` within 5 s, and `probe(addr)` is `Kanata`;
  - `stop_ends_the_child`: after `stop()`, the child's PID is gone within 2 s;
  - `drop_stops_the_child`;
  - `never_answer_fails_after_10s`: with `FAKE_START=never-answer`, `NoAnswer` arrives in under 11 s and the child is stopped. Use a shorter timeout parameter in tests if `Supervisor` exposes one (`SupervisorOptions::ready_timeout`).

### Implementation for User Story 1

- [ ] T011 [US1] Implement the core of `crates/kanata-client/src/supervisor.rs` (data-model.md › SessionState and SupervisorEvent):
  - `pub enum SessionState { Checking, Starting, WaitingForKeyboard, Running, Restarting { attempt: u32, in_secs: u64 }, Failed(Failure), Stopped }`;
  - `pub enum SupervisorEvent { State(SessionState), Ready(SocketAddr), Attach(SocketAddr), LogChanged }`;
  - `pub struct Supervisor`, holding a command `Sender<Command>` and the `JoinHandle`;
  - `Supervisor::start(opts, on_event)` spawns **one** thread named `kanata-supervisor`. That thread spawns the child (via `LaunchOptions::spawn`), polls `probe(opts.address(), 300 ms)` every 200 ms until it answers (`Ready` + `Running`), the child exits, or 10 s pass (`ready_timeout`, "if it does not answer within 10 seconds, the start has failed", FR-004). While Kanata's latest output line contains `no keyboard devices found` (research R4), pause the timer and emit `State(WaitingForKeyboard)`. While running it polls `try_wait` every 200 ms and listens for commands with `recv_timeout`;
  - `stop(self)` / `Drop`: send `Command::Stop`, then `terminate(child, 1.5 s)` on the supervisor thread, emit `State(Stopped)`, join.

  In this phase, a child that exits just becomes `Failed(Exited { code })`. The restart logic comes in US2.
- [ ] T012 [US1] Implement `KanataState::start()` in `crates/app/src/kanata_state.rs`. On a background thread: `probe(args.address)`.
  - **`Nothing`** with `args.config` set: managed on `args.address`'s port;
  - **`NotKanata`** with config: managed on `kanata_client::launcher::free_port()`;
  - **without config**: mode `"none"`.

  When managed, build `LaunchOptions { kanata_bin: args.kanata or "kanata", config, port, .. }` and `Supervisor::start` with a callback that `qt_thread().queue(...)`s each event. On `Ready(addr)`, write `addr.to_string()` into the shared address: `pub(crate) static ADDRESS: OnceLock<Arc<Mutex<String>>>` in `overlay_state.rs`, set once in `OverlayState::start()` (T008) and read by `KanataState`. On `Attach(addr)` (T020), drop the supervisor and switch to mode `"attached"`. On each `State(s)`, set `state_text` to the data-model text ("checking config…", "starting Kanata…", "managed", …), and set `mode` to `"managed"` and `can_restart` to true. Store the `Supervisor` in the Rust struct. `shutdown()` takes it and calls `stop()`.
- [ ] T013 [US1] In `crates/app/qml/Main.qml`, call `kanataState.start()` from `Component.onCompleted` right after `overlayState.start()`. In `crates/app/qml/Board.qml` (header status text), show `kanata.stateText` before the status and notices, joined with ` · `; pass `kanata: kanataState` into `Board` through the overlay windows (`LayerShellOverlay.qml`, `FallbackOverlay.qml`), with a new `required property var kanata`.
- [ ] T014 [US1] Manually validate quickstart.md §3 with real Kanata (SC-001, SC-002: 10 trials over the close, Ctrl+C and `pkill -9` paths). Add a "Validation log" to the end of `specs/003-managed-kanata/quickstart.md`.

**Checkpoint**: The MVP. One command starts both, and Kanata never outlives the overlay.

---

## Phase 4: User Story 2 - Kanata problems are visible and recoverable (Priority: P2)

**Goal**: Plain-language failure notices; restarts with backoff that give up after 5 failures in 2 minutes; an in-memory log window; restart on demand.

**Independent Test**: quickstart.md §4. A broken config shows Kanata's summary and isn't retried. `pkill kanata` → restarted within 5 s. Five kills in 2 minutes → "Kanata keeps stopping". The log window shows Kanata's output.

### Tests for User Story 2

- [ ] T015 [P] [US2] Write unit tests in `crates/kanata-client/src/backoff.rs` with injected `Instant`s:
  - delays "1, 2, 4, 8, 16, then 30 s";
  - reset after 60 s of running;
  - "After 5 failures within 2 minutes" → give up;
  - 5 failures spread over more than 2 minutes → no give-up;
  - `clear()` resets everything.
- [ ] T016 [P] [US2] Write unit tests in `crates/kanata-client/src/failure.rs` that feed in captured Kanata 1.12 output as fixtures:
  - the real `--check` output for `(deflayer base a nosuchkey)` (research R4) → `InvalidConfig { summary: "Unknown key/action: nosuchkey" }`;
  - output with `[ERROR]` but no `help:` → the first `[ERROR]` line's message;
  - `failed to open keyboard device(s): Permission denied` → `Permissions`;
  - `Address already in use` (from the panic seen with a busy port) → `PortInUse`;
  - other output plus an exit → `Exited`;
  - ANSI colour codes are stripped before matching.
- [ ] T017 [P] [US2] Add process tests to `crates/kanata-client/tests/supervisor.rs`:
  - `invalid_config_is_not_retried` (`FAKE_CHECK=fail` → `Failed(InvalidConfig)` with the summary, and the child is never spawned);
  - `not_installed` (`kanata_bin = "/nonexistent"` → `Failed(NotInstalled)`);
  - `permissions_are_not_retried` (`FAKE_START=permission`);
  - `crash_restarts_with_backoff` (`FAKE_CRASH_AFTER=1` → `Restarting { attempt: 1, in_secs: 1 }`, then `Running` again);
  - `gives_up_after_five` (use an injected short backoff through `SupervisorOptions` → `Failed(GaveUp)`);
  - `restart_clears_history`;
  - `log_keeps_output` (the fake's "ready" line is in `log()`, and ANSI codes are stripped).

### Implementation for User Story 2

- [ ] T018 [P] [US2] Implement `crates/kanata-client/src/backoff.rs` (data-model.md › Backoff): `pub struct Backoff`, with `failure(now) -> Next` where `Next` is `Retry(Duration)` or `GiveUp`, plus `running_since(now)`, `tick(now)` (resets after 60 s of running) and `clear()`.
- [ ] T019 [P] [US2] Implement `crates/kanata-client/src/failure.rs` (data-model.md › Failure, research R4):
  - `pub enum Failure { NotInstalled { program }, InvalidConfig { summary }, Permissions, PortInUse { port }, NoAnswer, Exited { code: Option<i32> }, GaveUp(Box<Failure>) }`;
  - `retryable()` and `notice()`, which returns the exact notice texts from data-model.md;
  - `classify_check(status, output)` and `classify_exit(status, output, port)`;
  - `strip_ansi(&str)`.
- [ ] T020 [US2] Extend `crates/kanata-client/src/supervisor.rs`:
  - **Before each start**: run `kanata --cfg PATH --check --no-wait` through the same `LaunchOptions` (same `kanata_bin`, **with** the parent-death signal: constitution III has no exemption), spawned on the supervisor thread and terminated on `Stop` like the main child. Capture its output into the log. A non-zero exit gives `Failed(InvalidConfig)`, and a spawn `NotFound` gives `Failed(NotInstalled)`;
  - **Capture**: stdout and stderr reader threads push `strip_ansi` lines into an `Arc<Mutex<VecDeque<String>>>` capped at 1000 lines and emit `LogChanged`. Never `tracing` the lines (FR-011);
  - **Classification**: an exit before ready or while running is classified (T019). `PortInUse` → probe the configured address again: if a Kanata answers now (another overlay won the race), emit `SupervisorEvent::Attach(addr)` and end supervision without spawning again; the app switches to **attached** (spec edge case "several overlays"). Only on `NotKanata` retry once at once on `free_port()` (and emit a new `Ready` address later). Retryable → `Backoff::failure` → `Restarting { attempt, in_secs }`, then sleep with `recv_timeout`, so `Stop` and `Restart` still work while waiting. `GiveUp` or not retryable → `Failed(..)`, then wait for commands;
  - **`restart()`**: `Command::Restart` → terminate the child if one is running, `Backoff::clear()`, and start over from the check;
  - `pub fn log(&self) -> Vec<String>`.

  Add `SupervisorOptions { ready_timeout, backoff_scale }` for the tests. The defaults follow FR-004 and FR-006.
- [ ] T021 [US2] Extend `crates/app/src/kanata_state.rs`:
  - on `State(Failed(f))`, call `overlay.addNotice(f.notice())`. QML forwards it: add a `#[qsignal] fn notice(text: QString)` on `KanataState` and connect it in `Main.qml` to `overlayState.addNotice`;
  - on `LogChanged`, throttle with an `Instant` stored in the struct to at most every 250 ms, then set `log_text` to `supervisor.log().join("\n")`;
  - `restart()` calls `Supervisor::restart()`.
- [ ] T022 [P] [US2] Create `crates/app/qml/LogWindow.qml`: an ordinary `ApplicationWindow` titled "Kanata log", with a `ScrollView` + `TextArea` that is `readOnly: true`, `selectByMouse: true`, uses a monospace font, and has `text: kanata.logText`. It auto-scrolls to the end when the text changes, unless the user has scrolled away from the bottom. Add it to `qml_files` in `crates/app/build.rs`.
- [ ] T023 [US2] Add a "Kanata" row to `crates/app/qml/SettingsWindow.qml`: a label with `kanata.mode` and `kanata.stateText`; a **Restart Kanata** button (`enabled: kanata.canRestart`, `onClicked: kanata.restart()`); and a **Show log** button that opens a `LogWindow` (`visible: kanata.mode === "managed"`). Pass `kanata: kanataState` into `SettingsWindow` from `Main.qml` with a new `required property var kanata`.
- [ ] T024 [US2] Manually validate quickstart.md §4 (SC-003: record the measured crash-to-connected time; if it is over 5 s, pass `--nodelay` on restarts only, since keys are already released after a crash; SC-004; and that labels follow an edited config after **Restart Kanata**) and add the result to the Validation log in `specs/003-managed-kanata/quickstart.md`.

**Checkpoint**: US1 and US2 work.

---

## Phase 5: User Story 3 - Attach to a Kanata that is already running (Priority: P3)

**Goal**: An already running Kanata is used and never stopped. When it disappears, the overlay offers "Start Kanata" but never takes over by itself (FR-014).

**Independent Test**: quickstart.md §2 (with the fake server) and §5 (with real Kanata). "attached", no second Kanata, and quitting leaves it running. When it is gone, **Start Kanata** appears and nothing starts automatically.

- [ ] T025 [US3] Complete the attach path in `crates/app/src/kanata_state.rs`:
  - `Probe::Kanata` → mode `"attached"`, `state_text` `"attached"`, no supervisor (FR-012, FR-013);
  - `can_start = mode == "attached" && !connected && config.is_some()`. Replace the `connected` property from T009 with an invokable `set_connected_state(bool)` that stores the value and recomputes `can_start`; `Main.qml` calls it from `overlayState.onConnectedChanged` (and once after start);
  - `start_managed()`: only if `can_start`, start a `Supervisor` exactly as in T012, using the configured port if `probe` says `Nothing` there, else `free_port()`. Set mode `"managed"` (US3 scenario 4).
- [ ] T026 [US3] In `crates/app/qml/SettingsWindow.qml`, add a **Start Kanata** button to the Kanata row: `visible: kanata.canStart`, `onClicked: kanata.startManaged()`.
- [ ] T027 [US3] Manually validate quickstart.md §2 (fake server) and §5 (real Kanata, SC-005: 10 start/quit cycles) and add the result to the Validation log in `specs/003-managed-kanata/quickstart.md`.

**Checkpoint**: All stories work.

---

## Phase 6: Polish & Cross-Cutting Concerns

- [ ] T028 [P] Update `README.md`:
  - **Quick start**: `just run` now starts Kanata itself if none is running; `just kanata` + `just run` still works (attached);
  - **New "Managed or attached" paragraph**: the header states, Restart, Show log, Start Kanata, `--kanata`;
  - **Escape hatch**: `pkill kanata`;
  - **Roadmap**: mark step 3 as done, noting that the `systemd --user` mode is deferred.
- [ ] T029 [P] Update `CLAUDE.md` › Conventions: "Spawn Kanata only on the supervisor thread (`PR_SET_PDEATHSIG` is per thread); Kanata output stays in the in-memory log, never `tracing`".
- [ ] T030 Run the privacy check in quickstart.md §6 (FR-011).
- [ ] T031 Run `just check` and fix every finding.
- [ ] T032 Review spec.md FR-001 – FR-014 against the implementation and the Validation log, and note any gaps in `specs/003-managed-kanata/quickstart.md`.

---

## Dependencies & Execution Order

- **Setup (T001–T003)** → **Foundational (T004–T009)** → stories.
- **US1 (T010–T014)**: needs T004, T006, T008 and T009. T011 → T012 → T013.
- **US2 (T015–T024)**: needs the T011 supervisor core. T018 and T019 before T020; T020 before T021; T022 before T023.
- **US3 (T025–T027)**: needs T009 and T012. It is independent of US2, apart from sharing `SettingsWindow.qml` with T023 (do those two sequentially).
- **Polish**: after the stories you want to ship.

### Parallel Opportunities

- Setup: T003 alongside T001/T002.
- Foundational: T005 (launcher tests), T006 (probe), T007 (main.rs) in parallel after T004.
- US2: T015/T016/T017 (tests) and T018/T019 (pure modules), plus T022 (QML), in parallel.

## Parallel Example: User Story 2

```bash
Task: "T015 backoff tests in crates/kanata-client/src/backoff.rs"
Task: "T016 failure classification tests in crates/kanata-client/src/failure.rs"
Task: "T019 Failure + classify in crates/kanata-client/src/failure.rs"   # after T016 if TDD
Task: "T022 LogWindow.qml"
```

---

## Implementation Strategy

### MVP First (User Story 1)

1. Setup → Foundational. The PDEATHSIG test is green **before** any real Kanata is started.
2. US1 → quickstart §3 with real Kanata, 10 kill trials → commit.

### Incremental Delivery

1. US2 → §4 → commit.
2. US3 → §2 and §5 → commit.
3. Polish → PR.

## Notes

- T014, T024 and T027 are the manual test notes constitution V requires. The agent runs only fake-Kanata smoke tests.
- If a manual test leaves Kanata running unexpectedly, that is a constitution III release blocker: fix it before merging.
