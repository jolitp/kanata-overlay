# Contract: Supervisor API, CLI and QML bridge

## `kanata_client::Supervisor` (library, no Qt)

```rust
pub struct Supervisor { /* owns the supervisor thread */ }

impl Supervisor {
    /// Start supervising: --check, spawn, readiness, restarts. Returns at once.
    /// `on_event` runs on the supervisor thread and must be cheap (e.g. queue to Qt).
    pub fn start(opts: LaunchOptions, on_event: impl Fn(SupervisorEvent) + Send + 'static) -> Self;
    /// Stop Kanata and start it again, clearing the failure history (FR-009).
    pub fn restart(&self);
    /// Stop Kanata (SIGTERM, 1.5 s, SIGKILL) and end the thread. Blocks ≤ 2 s.
    pub fn stop(self);
    /// A snapshot of the recent output (KanataLog).
    pub fn log(&self) -> Vec<String>;
}
// Drop calls stop(), so a managed Kanata never outlives its Supervisor.

pub fn probe(address: SocketAddr, timeout: Duration) -> Probe;   // research R2
pub enum Probe { Kanata, Nothing, NotKanata }
```

`LaunchOptions` gains `parent_death_signal: bool`, default `true`, which applies the
`pre_exec` hook from research R1. `spawn()` keeps its current signature.

## CLI

```
kanata-overlay … [--config PATH] [--kanata PROGRAM]
```

| Option | Meaning |
|--------|---------|
| `--config` (existing) | Also enables managed mode: started when no Kanata answers (FR-001) |
| `--kanata` (new) | The Kanata program to run. Default: `kanata`, looked up on `PATH` |
| `--connect` (existing) | The address to probe and attach to. A managed Kanata binds it when it is free, else a free local port |

## QML bridge: `KanataState` *(new QObject)*

| Property | Type | Meaning |
|----------|------|---------|
| `mode` | QString | `"none"`, `"attached"` or `"managed"` |
| `stateText` | QString | The header text from data-model › SessionState, or `"attached"` |
| `logText` | QString | The joined KanataLog, updated at most every 250 ms; empty when not managed |
| `canRestart` | bool | `mode == "managed"` |
| `canStart` | bool | `mode == "attached"`, not connected, and `--config` given (FR-014); recomputed by `setConnectedState` |

| Invokable | Effect |
|-----------|--------|
| `start()` | Called once by `Main.qml`: probe, then attach, manage or none |
| `restart()` | `Supervisor::restart()` (US2 scenario 6) |
| `startManaged()` | Attached and gone → managed (US3 scenario 4) |
| `shutdown()` | `Supervisor::stop()`; called from `Qt.application.aboutToQuit` |
| `setConnectedState(bool)` | Called by `Main.qml` on `overlayState.onConnectedChanged`; updates `canStart` |

`OverlayState`'s connection loop takes the address from a shared
`static ADDRESS: OnceLock<Arc<Mutex<String>>>` in `overlay_state.rs`, which `KanataState` updates on
`Ready`. On `Attach(addr)` (another overlay's Kanata won the port), `KanataState` drops its
supervisor and switches to `"attached"`. A failure's notice goes
to `OverlayState.addNotice`.

## UI

- **Header**: the status text shows `kanata.stateText`. For example: "● nav · managed", "● nav · attached", "○ restarting Kanata (attempt 2)".
- **Settings window**: a new "Kanata" row with the mode, a **Restart Kanata** button (enabled when `canRestart`), a **Start Kanata** button (shown when `canStart`) and a **Show log** button that opens `LogWindow.qml`.
- **`LogWindow.qml`**: an ordinary window with a read-only, selectable `TextArea` (monospace) showing `logText`. It auto-scrolls to the end unless you have scrolled up.
