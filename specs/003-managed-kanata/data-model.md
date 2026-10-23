# Data Model: Managed Kanata

**Feature**: [spec.md](spec.md) | **Research**: [research.md](research.md)

Types live in `crates/kanata-client` (no Qt) unless marked *(app)*.

## Mode *(app)*

How the overlay relates to Kanata. It is decided at start-up (research R2) and changed
only by a user action.

| Mode | Meaning | The overlay may stop or restart Kanata? |
|------|---------|----------------------------------|
| `None` | No Kanata was found and no `--config` was given | — |
| `Attached` | A Kanata already answered on the configured address | no (FR-013) |
| `Managed` | The overlay started Kanata and supervises it | yes |

```
start-up ── probe(address) answers ───────────────────────────────▶ Attached
         └─ no answer ── --config given ────────────────────────────▶ Managed
                      └─ no --config ───────────────────────────────▶ None

Attached ── Kanata gone ──▶ Attached, not connected (reconnecting; never takes over by itself, FR-014)
Attached, not connected ── user: "start Kanata" (only with --config) ──▶ Managed
```

## SessionState

The state of a managed Kanata. It is reported by the supervisor through events.

| State | Meaning | Shown in the header |
|-------|---------|-------------------|
| `Checking` | Running `kanata --check` | "checking config…" |
| `Starting` | Spawned, waiting for it to answer (≤ 10 s, FR-004) | "starting Kanata…" |
| `WaitingForKeyboard` | Kanata logs "no keyboard devices found"; the readiness timer is paused | "waiting for a keyboard" |
| `Running` | Answering; the overlay is connected | "managed" |
| `Restarting { attempt, in_secs }` | Waiting out the backoff before attempt N (FR-006) | "restarting Kanata (attempt N)" |
| `Failed(Failure)` | Won't retry (FR-007, FR-008) | the failure's notice |
| `Stopped` | Stopped by the overlay (on quit or before a user restart) | — |

```
Checking ── check ok ──▶ Starting ── answers ──▶ Running
   │                       │ exits / 10 s timeout      │ exits
   │ check fails           ▼                           ▼
   └──▶ Failed(InvalidConfig)   Failure ── retryable & not given up ──▶ Restarting ──▶ Checking
                                        └─ not retryable or 5 in 2 min ─▶ Failed(…)
any ── user restart ──▶ Stopped ──▶ Checking (failure history cleared)
any ── overlay quits ──▶ Stopped (SIGTERM, 1.5 s, SIGKILL; research R1)
```

## Failure

| Variant | Data | Retryable | Notice (plain words, FR-008) |
|---------|------|-----------|------------------------------|
| `NotInstalled` | the program name | no | "Kanata is not installed or not on PATH (kanata)" |
| `InvalidConfig` | `summary`: Kanata's first `help:` or `[ERROR]` line | no | "Kanata config error: {summary}" |
| `Permissions` | — | no | "Kanata may not read the keyboard or create its virtual keyboard: see README › Running Kanata without root" |
| `PortInUse` | port | yes, once, on a new port | — (handled silently) |
| `NoAnswer` | — | yes (backoff) | "Kanata did not answer within 10 s" |
| `Exited` | exit code or signal | yes (backoff) | "Kanata stopped (code N)" |
| `GaveUp` | last failure | no | "Kanata keeps stopping; see the log" |

## Backoff

| Field | Rule |
|-------|------|
| delay | 1, 2, 4, 8, 16, then 30 s ("starting around 1 second, at most 30 seconds", FR-006) |
| reset | after 60 s of continuous `Running` |
| give up | "After 5 failures within 2 minutes" → `GaveUp` (FR-007) |
| clear | on user restart (FR-009) |

## KanataLog

| Field | Rule |
|-------|------|
| lines | ring of the last 1000 lines ("at least the last 500 lines", FR-010), ANSI codes stripped |
| lifetime | memory only. It is never written to disk or `tracing`, and is dropped on exit (FR-011) |
| sources | the managed Kanata's stdout and stderr, plus the `--check` output |

## SupervisorEvent

Sent from the supervisor thread to the app; the app applies it with `qt_thread().queue(...)`.

| Event | Data |
|-------|------|
| `State` | `SessionState` |
| `Ready` | `SocketAddr`, where the connection loop should connect |
| `Attach` | `SocketAddr`: another Kanata took the port first; the app switches to attached and the supervisor ends |
| `LogChanged` | — (the app reads the shared log, throttled to every 250 ms) |
