# Research: Key Action Labels

**Feature**: [spec.md](spec.md) | **Plan**: [plan.md](plan.md) | **Date**: 2026-10-23

Kanata facts below come from the Kanata 1.12.0 source in nixpkgs (the version the dev
shell runs): `parser/src/cfg/{mod.rs,list_actions.rs,sexpr.rs,deflayer.rs}`,
`parser/src/keys/mod.rs` and `docs/config.adoc`.

## R1. Parse the config ourselves, with a hand-written subset parser

- **Decision**: A new Qt-free crate, `crates/kbd`, with three parts:
  - a small lexer for Kanata's s-expressions;
  - a reader for the top-level forms the overlay needs;
  - an action classifier that turns each action into a label.
- **Rationale**:
  - Constitution VII names this approach explicitly ("hand-written parsers for the small subset of `.kbd` the overlay needs before depending on Kanata's full parser").
  - Kanata's `kanata-parser` crate pulls in the whole keyberon state machine and per-OS key tables, and it rejects a config outright on the first error. We want lenient, best-effort labels (FR-003, SC-005).
- **Alternatives considered**:
  - The `kanata-parser` crate. Heavy, version-locked to Kanata, and all-or-nothing on errors.
  - Kanata's `RequestCurrentLayerInfo`, which returns the active layer's `cfg_text`. It contains no `defsrc` or aliases, so it can't tell what `@cap` means. It also requires a connection, but FR-010 wants labels without one.

## R2. Lexing

- **Decision**: The lexer handles:
  - `(` and `)`;
  - atoms (any run of non-space, non-paren bytes);
  - `"…"` strings and `r#"…"#` multi-line strings, which become atoms holding their text;
  - `;;` line comments and `#| … |#` block comments.

  An unbalanced parenthesis is an error that carries a line and column. The forms read
  before it are kept.
- **Rationale**: This matches `sexpr.rs`. Kanata treats a single `;` as part of an atom (`;` is a key name), so only `;;` starts a comment.

## R3. Supported top-level forms

| Form | Support |
|------|---------|
| `defsrc` | ✅ An ordered list of key names. Names are canonicalised (R5). |
| `deflayer NAME …` | ✅ Actions by position. Missing positions count as transparent. Extra positions are ignored with a warning (spec edge case). |
| `deflayermap (NAME) key action …` | ✅ Pairs. The wildcards `_` (unmapped `defsrc` keys), `__` (keys not in `defsrc`, irrelevant here) and `___` (all keys) are applied to `defsrc` keys. Anything left unmapped is transparent (`config.adoc` line 330). |
| `defalias name action …` | ✅ Pairs. Several `defalias` blocks are allowed. |
| `defcfg` | Read only for `delegate-to-first-layer` (R6). Everything else is ignored silently. |
| `defvar` | ❌ Not substituted. An action that is a `$var` is shown as raw text. Variables in timeout positions of `tap-hold` don't matter, because only the tap and hold actions are read. |
| `include`, `deftemplate`, `defaliasenvcond`, `platform`, `environment` | ❌ Reported once in a notice: "skipped: include, deftemplate". |
| Everything else (`defchords`, `defseq`, `deffakekeys`, `defoverrides`, …) | Ignored silently. These forms don't decide what a key does on a layer. |

## R4. Action classification (for labels)

Kanata 1.12's action names (`list_actions.rs`) are grouped like this:

| Kanata form | Kind |
|-------------|------|
| key name, e.g. `a`, `ret`, `⏎` | `Key` |
| `C-`, `S-`, `A-`, `M-`, `RC-`, `RS-`, `RA-`/`AG-`, `RM-` prefixes, and the symbol forms `‹⌃ ⌃› ‹⇧ ⇧› ‹⌥ ⌥› ‹◆ ◆› ‹⌘ ⌘› ‹❖ ❖› ‹⎈ ⎈› ‹⎇ ⎇› ⎈ ⌥ …` (`KEYMODI` in `cfg/mod.rs`) | `Combo { mods, key }` |
| `_`, `‗`, `≝` | `Transparent` |
| `XX`, `✗`, `∅`, `•` | `NoOp` |
| `use-defsrc` | `UseDefsrc` |
| `@name` | `Alias(name)` |
| `layer-switch` | `LayerSwitch(name)` |
| `layer-while-held`, `layer-toggle` | `LayerHold(name)` |
| `tap-hold*`, `tap⬓*` (every variant; the tap is argument 3 and the hold argument 4) | `TapHold { tap, hold }` |
| `one-shot*` | `OneShot(action)` |
| `multi` | `Combo` if every part but the last is a modifier key, else `Other` |
| `unicode X`, `🔣X` | `Unicode(X)` |
| `caps-word*` | `CapsWord` |
| `macro*`, `tap-dance*`, `fork`, `switch`, `chord`, `cmd`, mouse actions, anything else | `Other(raw)` |

- **Rationale**: These are the forms most configs use (layers, home-row mods, shortcuts). Everything else still gets a shortened raw label (FR-006, last bullet). This keeps the parser small, as constitution VII asks.

## R5. Key name canonicalisation

- **Decision**: A generated table, `crates/kbd/src/key_names.rs`, maps every spelling Kanata 1.12 accepts (`str_to_oscode` in `parser/src/keys/mod.rs`, 181 match arms) to its `OsCode` name. For example, `ret`, `enter` and `⏎` all map to `KEY_ENTER`. `defsrc` entries, deflayermap keys and the layout's per-key Kanata names are all compared through this table. Names that aren't in the table are compared literally.
- **Rationale**:
  - The layout import produces short names (`ret`, `bspc`, `lsft`), but configs often use other spellings (`enter`, `lshift`), and FR-005 needs them to match.
  - Generating the table once from the pinned Kanata source is more accurate than a hand list, and stays reviewable. A script, `scripts/gen-key-names.py`, regenerates it whenever Kanata is bumped, much like the `kanata-tcp-protocol` pin.
  - `#[cfg(target_os)]` arms: the Linux and `any(..., "linux", ...)` ones are kept.
- **Alternatives considered**: Extending `layout::guess_kanata_name`. It maps legends, not config spellings, which is a different job.

## R6. Transparent keys and the layer stack

- **Kanata semantics** (`config.adoc` §Transparent key): a transparent key falls through a stack. The stack is the temporary `layer-while-held` layers, then the base layer (set by `layer-switch`), then the first layer if `delegate-to-first-layer` is set, then `defsrc`.
- **Problem**: The TCP `LayerChange` event only names the new top layer. The overlay can't see the stack.
- **Decision**: Approximate the stack as **active layer → first layer → `defsrc`**:
  - transparent on the active layer → the first layer's action at that position;
  - if the active layer *is* the first layer, or the first layer is also transparent → the `defsrc` key itself.

  The label is styled `inherited` (FR-006).
- **Rationale**: This is exact for the common case: the base layer is the first layer, and one layer is held on top of it. It is a documented approximation for deeper stacks.
- **Alternative**: Tracking the stack from layer events. It isn't possible reliably, because the events don't say whether a change was a switch or a hold.

## R7. Label text

- **Decision**: Labels are built in Rust (constitution I). The rules are listed in [contracts/labels.md](contracts/labels.md). In short:
  - **Glyph table**: a hand-made table maps common `OsCode` names to glyphs (`⏎ ⌫ ⌦ ⇥ ␣ ⇪ ← → ↑ ↓ ⇞ ⇟ ⇱ ⇲`, `Esc`, `F1`…). Letters show in upper case, as on key caps.
  - **Fallback**: any other key shows its name as written.
  - **Modifiers**: `⌃` Ctrl, `⇧` Shift, `⌥` Alt, `◆` Super. A right-hand modifier gets a `›` suffix, e.g. `⌥›E` for `RA-e`.
  - **Layers**: momentary layers show `↥name` and switched layers `⇒name`.
  - **Length**: raw text is shortened to 8 characters plus `…`.
- **Rationale**: Labels are short and recognisable, and the rules are unit-testable without Qt (SC-004).

## R8. Where the path comes from, and reloads

- **Decision**:
  - A new `--config PATH` option, parsed in `main.rs` next to `--connect` and `--layout`. Per the clarification, it is not saved.
  - The file is read at start-up on a background thread. The parsed config is pushed to the QObject with `qt_thread().queue(...)`.
  - The connection thread already receives `ServerMessage::ConfigFileReload { new }`. It re-reads the given path and, if `new` differs from it (compared as canonical paths), adds the notice "Kanata switched to <new>; the overlay still reads <path>".
  - A failed re-read keeps the old config (FR-012).
  - `just run` passes `--config {{kanata_cfg}}`, so the example layout and the example config go together.
- **Rationale**: This reuses the existing connection thread and the Qt-thread rule (constitution I) and adds no new thread type.

## R9. Rendering

- **Decision**: `OverlayState` gains a `labels_json` property: a map from canonical Kanata key name to `{text, style, hold, alias}`. It is recomputed in Rust on a layer change, a config load and a reload.
  - `KeyCap.qml` looks up its key and shows `text`, falling back to today's legend.
  - The text uses `fontSizeMode: Text.Fit` (minimum 7 px) plus elide, so it never overflows (FR-007).
  - `style` picks the colour: normal, inherited (dimmer), disabled (very dim), or unbound (orange outline, as today).
  - `hold` draws a small dot in the top-right corner (Q2 = C).
  - `alias` is a tiny second line under the label, when present (US2 scenario 3).
- **Rationale**: QML only renders (constitution I). Recomputing on a layer change is a few microseconds for ~100 keys, well within SC-002.

## R10. Privacy

- The config text stays in memory. Logs and notices name the file and the form or line that failed, never the content (FR-013). The overlay still never sees key presses (constitution VI).
