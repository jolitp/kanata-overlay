# Quickstart & Validation: Managed Kanata

[data-model.md](data-model.md) · [contracts/interfaces.md](contracts/interfaces.md)

## Prerequisites

- `nix develop`.
- For §3–§5, real Kanata permissions: `just check-perms` shows ✓, and on NixOS you have done the README's NixOS setup.
- **Escape hatch, read first**: if anything goes wrong, the keyboard still works through Kanata, and `pkill kanata` from any terminal (the mouse works too) releases it.

## 1. Automated (no Kanata, no permissions)

```sh
just check
cargo test -p kanata-client
```

Expected: everything passes. The tests cover:
- backoff delays, the 60 s reset and the 5-in-2-minutes give-up;
- failure classification from captured Kanata output;
- the probe (Kanata, nothing, not Kanata);
- with `tests/fake-kanata.sh`: start → ready, crash → restart, `InvalidConfig`, `NotInstalled`, stop within 1.5 s, and **a child that dies with its parent thread** (PDEATHSIG).

## 2. Attach mode with the fake server (US3; no permissions)

| Step | Expected |
|------|----------|
| `just fake-kanata nav`, then `just run` | The header says "attached". No `kanata` process starts (`pgrep -a kanata` is empty). |
| Quit the overlay | The fake server keeps running. |
| Stop the fake server while the overlay runs | "not connected", and the settings window shows **Start Kanata**. Nothing starts by itself (FR-014). |

## 3. Managed mode (US1; real Kanata)

| Step | Expected |
|------|----------|
| No Kanata running. `just run` | Within 5 s (SC-001): "checking config…", "starting Kanata…", then "managed". Tapping Caps gives Esc. |
| Close the overlay | Within 2 s, `pgrep -a kanata` is empty and Caps is Caps again. |
| Run it again and press Ctrl+C in its terminal | The same (SC-002). |
| Run it again, then `pkill -9 kanata-overlay` | The same: the parent-death signal. |
| Repeat each quit path until there are 10 trials in total | 10/10 clean (SC-002). |

## 4. Failures and restarts (US2; real Kanata)

| Step | Expected |
|------|----------|
| `just run --config <broken copy>` | The notice "Kanata config error: …" with Kanata's own summary. No retries (FR-008). |
| A valid config, then `pkill kanata` from another terminal | "restarting Kanata (attempt 1)", then "managed" again within 5 s (SC-003). |
| Repeat `pkill kanata` 5 times within 2 minutes | "Kanata keeps stopping; see the log", and no more restarts (FR-007). |
| Settings → **Restart Kanata** | Kanata starts again, and the labels follow any config edits (US2 scenario 6). |
| Settings → **Show log** | At least the last 500 lines of Kanata's output, selectable. |
| `just run --kanata /nope` | "Kanata is not installed…". No retries. |
| Temporarily without permissions (e.g. in a session from before you joined the groups) | The permissions notice, pointing to the README. |

## 5. Attach to real Kanata (US3, SC-005)

`just kanata` in terminal 1, then start and quit `just run` 10 times. `pgrep -c kanata`
stays at 1, and Kanata keeps running.

## 6. Privacy (FR-011)

Run `RUST_LOG=debug QT_FORCE_STDERR_LOGGING=1 just run 2>&1 | grep -c "\[INFO\]"`. Expected:
`0`, because Kanata's output never reaches the overlay's own log. No new files appear
under `~/.local/state` or `~/.cache`.
