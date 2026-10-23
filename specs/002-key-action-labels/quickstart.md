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
