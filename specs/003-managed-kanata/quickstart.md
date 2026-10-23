# Quickstart & Validation: Managed Kanata

[data-model.md](data-model.md) · [contracts/interfaces.md](contracts/interfaces.md)

## Prerequisites

- `nix develop`.
- For §3–§5, real Kanata permissions: `just check-perms` shows ✓, and on NixOS you have done the README's NixOS setup.
- **Escape hatch, read first**: if anything goes wrong, the keyboard still works through Kanata, and `pkill -x kanata` from any terminal (the mouse works too) releases it.

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
| `just fake-kanata nav`, then `just run` | The header says "attached". No `kanata` process starts (`pgrep -ax kanata` is empty). |
| Quit the overlay | The fake server keeps running. |
| Stop the fake server while the overlay runs | "not connected", and the settings window shows **Start Kanata**. Nothing starts by itself (FR-014). |

## 3. Managed mode (US1; real Kanata)

| Step | Expected |
|------|----------|
| No Kanata running. `just run` | Within 5 s (SC-001): "checking config…", "starting Kanata…", then "managed". Tapping Caps gives Esc. |
| Close the overlay | Within 2 s, `pgrep -ax kanata` is empty and Caps is Caps again. |
| Run it again and press Ctrl+C in its terminal | The same (SC-002). |
| Run it again, then `pkill -9 kanata-overlay` | The same: the parent-death signal. |
| Repeat each quit path until there are 10 trials in total | 10/10 clean (SC-002). |

## 4. Failures and restarts (US2; real Kanata)

| Step | Expected |
|------|----------|
| `just run --config <broken copy>` | The notice "Kanata config error: …" with Kanata's own summary. No retries (FR-008). |
| A valid config, then `pkill -x kanata` from another terminal | "restarting Kanata (attempt 1)", then "managed" again within 5 s (SC-003). |
| Repeat `pkill -x kanata` 5 times within 2 minutes | "Kanata keeps stopping; see the log", and no more restarts (FR-007). |
| Settings → **Restart Kanata** | Kanata starts again, and the labels follow any config edits (US2 scenario 6). |
| Settings → **Show log** | At least the last 500 lines of Kanata's output, selectable. |
| `just run --kanata /nope` | "Kanata is not installed…". No retries. |
| Temporarily without permissions (e.g. in a session from before you joined the groups) | The permissions notice, pointing to the README. |

## 5. Attach to real Kanata (US3, SC-005)

`just kanata` in terminal 1, then start and quit `just run` 10 times. `pgrep -cx kanata`
stays at 1, and Kanata keeps running.

## 6. Privacy (FR-011)

Run `RUST_LOG=debug QT_FORCE_STDERR_LOGGING=1 just run 2>&1 | grep -c "\[INFO\]"`. Expected:
`0`, because Kanata's output never reaches the overlay's own log. No new files appear
under `~/.local/state` or `~/.cache`.

## Validation log

### 2026-10-23: automated checks and agent smoke tests (fake Kanata only)

| Check | Result |
|-------|--------|
| `just check`: 83 tests, clippy `-D warnings` | ✅ |
| `kanata-client` process tests: PDEATHSIG (child of an exited thread gets SIGTERM), `terminate` incl. a SIGTERM-ignoring child, start → ready → stop < 2 s, drop stops, crash → restart, give up after 5 + user restart, invalid config not retried (and never spawned), permissions not retried, never-answer → restart, waiting for a keyboard (timer paused), not installed, attach when another Kanata wins the port | ✅ |
| App, managed (fake via `--kanata`): "checking config…" → "starting Kanata…" → "managed" | ✅ |
| App, `kill -9` of the overlay → its Kanata's port closed within 1 s (parent-death signal through the fake's `exec`) | ✅ FR-003 |
| App, SIGTERM of the overlay → Kanata gone | ✅ |
| App, Kanata crash (`FAKE_CRASH_AFTER=2`) → "restarting Kanata (attempt 1)" → "managed" again, then attempt 2 | ✅ FR-006 |
| App, attached (`just fake-kanata` on the port): no Kanata started; the fake still runs after the overlay quits | ✅ FR-012, FR-013 (§2) |
| App, `--kanata /nope` → notice "Kanata is not installed or not on PATH (/nope)" | ✅ FR-008 |
| Privacy: Kanata's output (`fake kanata ready`) appears 0 times in the overlay's own log | ✅ FR-011 |

**Added beyond the tasks**: a **Quit Kanata Overlay** button in the settings window. On Wayland the overlay window can't be closed, so this is the "window closed / normal quit" path of US1 scenario 2. It goes through `aboutToQuit → shutdown()`.

**Still to do by hand with real Kanata** (the agent never runs it, because it grabs the keyboard): §3 (T014, including the 10 quit trials for SC-002), §4 (T024), §5 (T027, SC-005).

**Requirement review (T032)**: FR-001 – FR-014 are all implemented. The parts that depend on the real keyboard grab (SC-001, SC-002, SC-003, SC-005) are waiting for the manual tasks above.

### 2026-10-23: §3 with real Kanata (T014), checked by the user

| Check | Result |
|-------|--------|
| `just run` with no Kanata running: the header first shows "not connected" (during Kanata's start-up pause), then "managed · connected" | ✅ FR-001, FR-005 |
| Every quit path (Quit in settings, Ctrl+C, `pkill -9 kanata-overlay`) leaves no Kanata process behind | ✅ FR-003, SC-002 |

Also fixed: the log window used Controls' `TextArea`, and KDE's Breeze style warned at load ("Unable to assign TextArea… to QQuickTextInput", from its mobile text toolbar). It is now a plain `TextEdit` in a `Flickable`, with the same select, copy and follow-the-end behaviour, and no warning.

### 2026-10-23: §4 with real Kanata (T024), checked by the user (in progress)

| Check | Result |
|-------|--------|
| Invalid config (a stray `)`): not retried, labels fall back to legends | ✅ FR-008 |
| The reason is visible without a terminal | ❌ at first: the header showed only "Kanata failed", and the "Kanata config error: …" notice was cut off after "connecting to …" and the label parser's own message |
| After the fix: the header reads "Kanata config error: Unexpected closing parenthesis" (Kanata's own summary) | ✅ SC-004 |

**Fix**: a managed Kanata's failure is now the header's state text itself, not a separate notice. "connecting to …" is hidden while a managed Kanata is down. The settings window has a **Messages** field showing the state and all notices in full, wrapped.

- **Doc fix found by the user**: `pkill kanata` / `pgrep -a kanata` match any process whose name *contains* "kanata", so they also hit `kanata-overlay`. The quickstart, README and tasks now use `pkill -x kanata` / `pgrep -ax kanata` (exact name).
- `pkill -x kanata` with real Kanata: restarted automatically. From the log: killed at 09:44:46.12, `--check` at 47.71 (after the 1 s backoff), reconnected at 49.97, so **≈ 3.9 s** crash → connected ✅ SC-003. **Show log** showed this output ✅ FR-010.
- **Found in that log**: `[ERROR] Error writing response to RequestCurrentLayerName: Broken pipe` and `[WARN] failed to get peer address`. They were caused by the probe, which asked for the layer and hung up after Kanata's connect greeting. **Fix**: the probe now only listens for the greeting (Kanata 1.12 sends `LayerChange` to every new client); research R2 is updated. `just check` passes.
- 5 × `pkill -x kanata` within 2 minutes → the header reads "Kanata keeps stopping; see the log" and nothing restarts; the labels show the config's first layer while disconnected ✅ FR-007 (and feature 002's FR-010: the first layer is labelled while disconnected)
- From the "keeps stopping" state, settings → **Restart** → Kanata starts again and the header shows "managed" (the failure history is cleared) ✅ FR-009
- An edit to `nav` (`left` → `home`) before **Restart** → `⇱` on H after the restart (the config is re-read on reconnect) ✅ US2 scenario 6

T024 complete.

### 2026-10-23: §5 attach with real Kanata (T027), checked by the user

| Check | Result |
|-------|--------|
| Own `just kanata`, then `just run`: the header says "attached", and Restart and Show log are hidden | ✅ FR-012 |
| Repeated start and quit of the overlay: `pgrep -cx kanata` stays at 1, and the user's Kanata keeps running | ✅ FR-013, SC-005 |
| The user's Kanata stopped while attached → "not connected", nothing starts by itself, **Start Kanata** offered and switches to "managed" | ✅ FR-014, US3 scenario 4 |

All tasks complete. Every FR-001 – FR-014 and SC-001 – SC-005 is checked, automatically or by hand.
