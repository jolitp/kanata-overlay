# Quickstart & Validation: Key Action Labels

Rules: [contracts/labels.md](contracts/labels.md). Data: [data-model.md](data-model.md).

## Prerequisites

- `nix develop`.
- Two configs:
  - `examples/kanata/minimal.kbd` (existing);
  - `examples/kanata/all-actions.kbd` (new; one key per action kind in FR-006, for SC-004).
- Check both with Kanata itself: `just kanata-check examples/kanata/all-actions.kbd`.

## 1. Automated

```sh
just check
cargo test -p kbd
```

Expected: everything passes. The `kbd` tests cover:
- the lexer: `;;` comments, `#| |#` blocks, strings, and an unbalanced-paren error with line and column;
- `defsrc`, `deflayer` (short and long layers), `deflayermap` with wildcards, and several `defalias` blocks;
- alias chains, unknown aliases and alias loops;
- transparent fallback: active layer → first layer → `defsrc`;
- every row of the label table in contracts/labels.md;
- skipped forms being reported;
- canonical names (`enter` = `ret` = `⏎`);
- both example configs as fixtures: every bound key gets the expected label (SC-001).

## 2. Labels on the base layer and while holding a layer (US1, SC-001, SC-002)

```sh
just fake-kanata base      # terminal 1
just run                   # terminal 2 (passes --config examples/kanata/minimal.kbd)
```

| Step | Expected |
|------|----------|
| Look at the board | Letters show as `Q W E …`. Caps shows `Esc` with a blue hold dot. Backspace shows `⌫`, Enter `⏎`, Space `␣`. |
| Restart the fake server with `just fake-kanata nav` | Within 100 ms (SC-002): `H J K L` show `← ↓ ↑ →`, and every other key shows its base action in grey (inherited). |
| Run `just run` without Kanata running | The base layer (the first layer) is labelled, and the header says "kanata not connected". |
| `cargo run -p kanata-overlay -- --layout examples/layouts/sample.kle.json` (no `--config`) | Today's legends, with no notice. |
| `just run --config /nope.kbd` | Legends, plus the notice `config: cannot read /nope.kbd: …`. |

## 3. Complex actions (US2, SC-004)

```sh
just run --config examples/kanata/all-actions.kbd
```

Every key in the "action kinds" row of the config matches the table in
contracts/labels.md: `⌃C`, `↥nav`, `⇒game`, a tap-hold with a dot, an alias with its
`@name` line, `¹⇧`, `λ`, `⇪W`, a shortened macro, and a blank, disabled key.

## 4. Reload (US3, SC-003) — needs real Kanata

```sh
just kanata                # terminal 1 (real Kanata with TCP)
just run                   # terminal 2
```

| Step | Expected |
|------|----------|
| Change `h` in `deflayer nav` to `home`, save, and reload Kanata (its live-reload key, or restart it) | The overlay shows `⇱` on H within 2 s. |
| Introduce an unbalanced `(`, then reload | The old labels stay, and the notice says `config: cannot read …: line L, column C`. |

## 5. Privacy (FR-013)

Run `RUST_LOG=debug QT_FORCE_STDERR_LOGGING=1 just run 2>&1 | grep -i defalias`.
Expected: no output; config text is never logged.

## Validation log

### 2026-10-23: automated checks and agent smoke test (Plasma 6 Wayland)

| Check | Result |
|-------|--------|
| `just check` (66 tests, 35 in `kbd`) and clippy `-D warnings` | ✅ |
| `just kanata-check examples/kanata/all-actions.kbd` → "config file is valid" | ✅ |
| §2 base layer, `minimal.kbd` (screenshot): `Esc Q W … P`, `;`, Caps = `Esc` + blue hold dot + `@cap`, LShift unbound (orange; the sample layout's legend is a bare "Shift") | ✅ SC-001 |
| §2 `nav` layer (screenshot): `H J K L` = `← ↓ ↑ →`, every other key grey (inherited), Caps inherits `Esc @cap` | ✅ |
| SC-002: debug log "labels updated 21.9 ms / 23.3 ms after the layer change arrived" | ✅ < 100 ms |
| §2 `--config /nope.kbd` → notice "config: cannot read /nope.kbd: No such file or directory" | ✅ |
| §3 `all-actions.kbd` (screenshot): `⌃C ↥nav ⇒game Esc(dot, @th) ⌃V(@cp) ¹⇧ λ ⇪W (macro h…` and a blank, dimmed `XX` | ✅ SC-004 |
| §5 privacy: no config text in the debug logs; `minimal.kbd` sha256 unchanged after runs (FR-004, FR-013) | ✅ (without a reload; see below) |

**Found and fixed along the way**: the connection thread logged every unhandled Kanata message with `{:?}`. That included `HoldActivated { key }` and `TapActivated { key }`, which carry pressed-key names, against constitution VI. It now logs only the message kind.

**Still to do by hand**: §4 reload with real Kanata (T029). The agent didn't run real Kanata, because it grabs the keyboard (constitution III).

**Requirement review (T034)**: FR-001 – FR-013 are all implemented. FR-011 and FR-012 (reload, keep the old labels on failure) and US3 scenario 3 are implemented but unverified until T029.

### 2026-10-23: reload with real Kanata (§4, T029), checked by the user

| Check | Result |
|-------|--------|
| Real Kanata: holding Caps switches the overlay to `nav` | ✅ |
| Edited `nav` (`left` → `home`): the label changed | ✅ (see the note below) |
| Unbalanced `(`: the notice with line and column is shown, and the old labels stay | ✅ FR-012 |

- **Gap found**: Kanata 1.12 has no file watching. It sends `ConfigFileReload` only after a live reload (`lrld`); restarting Kanata is just a new connection. The overlay used to re-read the config only on `ConfigFileReload`, so edit-and-restart could leave stale labels.
- **Fix**: the overlay also re-reads the config each time it (re)connects to Kanata. Verified with the fake server on a copy of the config: after the edit and a server restart, `H` showed `⇱` on `nav`, and the log showed two reads. `just check` passes.
- The keyboard itself only changes after Kanata reloads or restarts. That's Kanata's behaviour, not the overlay's.
