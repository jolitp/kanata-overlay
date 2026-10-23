---
description: "Task list for 002-key-action-labels"
---

# Tasks: Key Action Labels

**Input**: Design documents from `/specs/002-key-action-labels/`

**Prerequisites**: [plan.md](plan.md), [spec.md](spec.md), [research.md](research.md), [data-model.md](data-model.md), [contracts/labels.md](contracts/labels.md), [quickstart.md](quickstart.md)

**Tests**: Unit tests for the Qt-free `kbd` crate are included (plan, constitution V). Rendering and reload are checked manually with [quickstart.md](quickstart.md).

**Organization**: Tasks are grouped by user story, so each story can be built and tested on its own.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependency on unfinished tasks).
- **[Story]**: The user story the task belongs to (US1–US3 from spec.md).

## Conventions for every task

- Run everything inside `nix develop`. `just check` must pass before each commit.
- Logic lives in `crates/kbd`. QML only renders (constitution I). QObject updates from other threads go through `qt_thread().queue(...)`.
- Config text never goes into logs or notices; name the file, the form and the line/column only (FR-013).
- Kanata 1.12 source for reference: `nix build --inputs-from . nixpkgs#kanata.src --print-out-paths`. See `parser/src/cfg/list_actions.rs`, `cfg/mod.rs` (`KEYMODI`, lines ~1376–1383 for `_`/`XX`) and `keys/mod.rs` (`str_to_oscode`).

---

## Phase 1: Setup

- [ ] T001 Create the crate `crates/kbd/Cargo.toml`: name `kbd`, description "Kanata .kbd subset parser and key labels (no Qt)", workspace package fields, no dependencies, `[lints] workspace = true`. Create `crates/kbd/src/lib.rs` with a module doc and `mod lexer; mod reader; mod action; mod label; mod key_names;`, plus empty files for each module.
- [ ] T002 Add `kbd = { path = "crates/kbd" }` to `[workspace.dependencies]` in `Cargo.toml`, and `kbd.workspace = true` to `crates/app/Cargo.toml`.
- [ ] T003 [P] Write `scripts/gen-key-names.py` (run with `python3 -I`; argument: the path to Kanata's `parser/src/keys/mod.rs`). It:
  - reads the `str_to_oscode` match;
  - for each arm `"a" | "b" … => OsCode::NAME`, emits `("a", "NAME")` pairs;
  - keeps arms with no `#[cfg]` and arms whose `#[cfg]` mentions `linux`, and drops the others;
  - writes `crates/kbd/src/key_names.rs` with a `// GENERATED from kanata <version> parser/src/keys/mod.rs by scripts/gen-key-names.py — do not edit` header and `pub(crate) static KEY_ALIASES: &[(&str, &str)] = &[...]`, sorted by alias for binary search.

  Run it against Kanata 1.12.0 and commit the output.
- [ ] T004 [P] In `justfile`, change the `run` recipe to pass `--config {{kanata_cfg}}` (after `--layout {{layout}}`).

---

## Phase 2: Foundational (Blocking Prerequisites)

**⚠️ CRITICAL**: No user-story work can begin until this phase is complete.

- [ ] T005 [P] Implement the lexer in `crates/kbd/src/lexer.rs` (research R2): `pub(crate) enum SExpr { Atom { text: String, line: u32, col: u32 }, List { items: Vec<SExpr>, line: u32, col: u32 } }` and `pub(crate) fn parse(text: &str) -> Result<Vec<SExpr>, (Vec<SExpr>, SyntaxError)>`, where `SyntaxError { line, column }`. The error case returns the top-level forms read before the error.
  - **Line comments**: `;;` to the end of the line. A single `;` is an atom.
  - **Block comments**: `#|` … `|#`.
  - **Strings**: `"…"` becomes an atom holding the inner text, and so does `r#"…"#`.
  - **Atoms**: maximal runs of non-whitespace bytes other than `(` and `)`, handled as UTF-8.
- [ ] T006 [P] Write unit tests in `crates/kbd/src/lexer.rs`:
  - nested lists;
  - `;;` comment vs `;` atom;
  - a block comment spanning lines;
  - a quoted string with spaces;
  - `r#"…"#`;
  - Unicode atoms (`⏎`, `‹⇧a`);
  - unbalanced `(` → a `SyntaxError` with the right line and column, plus the earlier forms;
  - a stray `)`.
- [ ] T007 Define the public types in `crates/kbd/src/lib.rs`, following data-model.md:
  - `KeyName { written: String, canonical: String }`;
  - `Layer { name: String, actions: Vec<Action> }`;
  - `Config { source: Vec<KeyName>, layers: Vec<Layer>, aliases: HashMap<String, Action>, delegate_to_first_layer: bool, warnings: Vec<ConfigWarning> }`;
  - `enum Action { Key(KeyName), Combo { mods: Vec<Modifier>, key: KeyName }, Transparent, NoOp, UseDefsrc, Alias(String), LayerSwitch(String), LayerHold(String), TapHold { tap: Box<Action>, hold: Box<Action> }, OneShot(Box<Action>), Unicode(String), CapsWord, Other(String) }`;
  - `enum Modifier { Ctrl, Shift, Alt, Super }` paired with `Side { Left, Right }`;
  - `enum ConfigError { Syntax { line, column }, NoDefsrc, NoLayers, Io(String) }`, implementing `Display` without any config text;
  - `enum ConfigWarning { Skipped { forms: Vec<String> }, LayerLength { layer, expected, found }, DuplicateAlias { name }, UnknownAlias { name }, AliasLoop { name }, UnknownLayer { name } }`, implementing `Display`;
  - `KeyLabel { text: String, style: LabelStyle, hold: bool, alias: Option<String> }` with `enum LabelStyle { Normal, Inherited, Disabled }`.

  Add `pub fn parse(text: &str) -> Result<Config, ConfigError>` and `pub fn read(path: &Path) -> Result<Config, ConfigError>`.
- [ ] T008 [P] Add `pub fn canonical(name: &str) -> String` to `crates/kbd/src/lib.rs`. It binary-searches `KEY_ALIASES` from T003 and returns the `OsCode` name (e.g. `KEY_ENTER`), or the name unchanged if it isn't in the table. Tests: `canonical("ret") == canonical("enter") == canonical("⏎") == "KEY_ENTER"`, `canonical("lsft") == canonical("lshift")`, and `canonical("nonsense") == "nonsense"`. Depends on T003.
- [ ] T009 Implement the reader in `crates/kbd/src/reader.rs` (research R3). It walks the top-level `SExpr`s:
  - `defsrc` → `source`, each entry built with `KeyName { written, canonical }`;
  - `deflayer NAME a b …` → `Layer`. Pad missing positions with `Transparent`, drop extra ones, and record `LayerLength` for either;
  - `defalias n1 a1 n2 a2 …` (possibly several blocks) → `aliases`. A repeated name → `DuplicateAlias`; the later one wins;
  - `defcfg` → read `delegate-to-first-layer yes|no` only;
  - `include`, `deftemplate`, `defaliasenvcond`, `platform`, `environment` → collected into a single `Skipped { forms }` (each name once);
  - any other form → ignored.

  Actions are classified by `action::classify(&SExpr) -> Action` (T012; until then a stub returns `Key` or `Other`). Return `NoDefsrc` / `NoLayers` when they apply. On a lexer `SyntaxError`, return `ConfigError::Syntax`.
- [ ] T010 [P] Add `--config PATH` to `Args` in `crates/app/src/main.rs` (field `config: Option<PathBuf>`, parsed like `--layout`), and document it in the module doc comment.
- [ ] T011 [P] In `crates/app/src/overlay_state.rs`, after the layout loads, add `"kanataCanonical": kbd::canonical(kanata)` to every key that has a `kanata` name, before calling `set_layout_json`. Go through `serde_json::Value` (contracts/labels.md › Key lookup).

**Checkpoint**: `cargo test -p kbd` passes the lexer and canonicalisation tests, and the reader parses `examples/kanata/minimal.kbd` into 1 source list, 2 layers and 1 alias.

---

## Phase 3: User Story 1 - See what each key does on the active layer (Priority: P1) 🎯 MVP

**Goal**: Plain keys, transparent, no-op and alias keys are labelled from the config for the active layer, and the labels switch with layer changes.

**Independent Test**: quickstart.md §2. With `minimal.kbd`, the base layer shows `Q W E …`, `⌫`, `⏎`, `␣`, and `Esc` (plus a hold dot) on Caps. Holding `nav` shows `← ↓ ↑ →` on H J K L, with every other key inherited in grey.

### Tests for User Story 1

- [ ] T012 [P] [US1] Write unit tests in `crates/kbd/src/label.rs` (`#[cfg(test)]`), using `include_str!("../../../examples/kanata/minimal.kbd")` as a fixture:
  - base layer: `a` → `A`; `bspc` → `⌫`; `ret` → `⏎`; `spc` → `␣`; `esc` → `Esc`; `@cap` → text `Esc`, `hold: true`, `alias: Some("cap")`;
  - `nav` layer: `h j k l` → `← ↓ ↑ →` (style `Normal`), `q` → `Q` (style `Inherited`);
  - unknown layer name → first layer plus an `UnknownLayer` warning;
  - transparent on the first layer → the `defsrc` key, `Inherited`;
  - `XX` → empty text, `Disabled`;
  - `use-defsrc` → source key, `Inherited`;
  - an alias chain `a → b → key`;
  - an alias loop `a → b → a` → text `@a` plus `AliasLoop`;
  - an unknown alias → `@x` plus `UnknownAlias`;
  - two `defsrc` spellings of one key collapse to one canonical entry;
  - SC-005: a config containing `(include other.kbd)` and a `deftemplate` still labels every key it understands, and reports one `Skipped { forms: ["include", "deftemplate"] }`.

### Implementation for User Story 1

- [ ] T013 [US1] Implement `pub(crate) fn classify(expr: &SExpr) -> Action` in `crates/kbd/src/action.rs` for the US1 kinds:
  - atom `_` `‗` `≝` → `Transparent`;
  - `XX` `✗` `∅` `•` → `NoOp`;
  - `use-defsrc` → `UseDefsrc`;
  - `@name` → `Alias(name)`;
  - any other atom → `Key(KeyName)`;
  - `(tap-hold* …)` / `(tap⬓* …)` → `TapHold { tap: classify(arg[3]), hold: classify(arg[4]) }`, which is needed for `@cap`;
  - any other list → `Other(raw text)`.
- [ ] T014 [US1] Implement `pub fn labels(config: &Config, active_layer: &str) -> (HashMap<String, KeyLabel>, Vec<ConfigWarning>)` in `crates/kbd/src/label.rs`, using the algorithm in data-model.md › Label resolution:
  - layer lookup with a first-layer fallback;
  - alias resolution, "depth ≤ 8; a loop shows `@name` with a warning";
  - transparent fallback "active → first layer → defsrc", style `Inherited`;
  - `UseDefsrc` → source key, `Inherited`;
  - `TapHold` → the tap's label with `hold: true`;
  - an alias sets `alias: Some(name)`;
  - the output is keyed by canonical name.

  Re-export it from `lib.rs`.
- [ ] T015 [US1] Implement key label text in `crates/kbd/src/label.rs`, following contracts/labels.md:
  - letters `a`–`z` in upper case;
  - digits and punctuation as they are;
  - the glyph table keyed by `OsCode` name (`KEY_ENTER ⏎`, `KEY_BACKSPACE ⌫`, `KEY_DELETE ⌦`, `KEY_TAB ⇥`, `KEY_SPACE ␣`, `KEY_ESC Esc`, `KEY_CAPSLOCK ⇪`, the arrows `← → ↑ ↓`, `⇞ ⇟ ⇱ ⇲`, `Ins`, `F1`–`F24`, the modifier keys `⇧ ⌃ ⌥ ◆` with `›` for the right side, and the media keys);
  - otherwise, the name as written;
  - `Other(raw)` shortened to 8 characters + `…`.
- [ ] T016 [US1] Load the config in `crates/app/src/overlay_state.rs`:
  - store `Option<kbd::Config>` and `config_path: Option<PathBuf>` in `OverlayStateRust`;
  - add `#[qproperty(QString, labels_json)]` (default `{}`);
  - in `start()`, if `args.config` is set, read it with `kbd::read`. On `Ok`, store it, call `relabel()`, and `add_notice` each `ConfigWarning` once. On `Err(e)`, `add_notice(format!("config: cannot read {path}: {e}"))` and keep `labels_json` as `{}`;
  - `relabel()` calls `kbd::labels(config, current_layer)` (an empty layer name gives the first layer, FR-010), serialises the map to JSON `{canonical: {text, style, hold, alias}}` with `style` as `"normal"`, `"inherited"` or `"disabled"` and `alias` as `""` when unset, and calls `set_labels_json`;
  - call `relabel()` whenever `current_layer` changes, i.e. in the `LayerChange` / `CurrentLayerName` push;
  - on disconnect, in the existing `set_connected(false)` push, label the first layer again (spec edge case "Kanata is not connected"). `relabel()` takes the layer to use: `current_layer` while connected, `""` otherwise;
  - log at debug level how long each `relabel()` takes, from receiving the layer name to `set_labels_json` (SC-002).
- [ ] T017 [US1] Render the labels in `crates/app/qml/KeyCap.qml` and `crates/app/qml/Board.qml`:
  - `Board` parses `overlay.labelsJson` once per change into `property var labels` and passes `label: board.labels[modelData.kanataCanonical]` to each `KeyCap`;
  - in `KeyCap`, `property var label` (may be undefined). The text is `label ? label.text : (key.kanata || key.legend || "")`, with `fontSizeMode: Text.Fit`, `minimumPixelSize: 7`, `elide: Text.ElideRight` and `wrapMode: Text.NoWrap`;
  - colour: normal `#e8e8e8`, inherited `#8a8a8a`, disabled `#555555`;
  - keep the unbound orange border for keys with no `kanata` name, or whose canonical name isn't in `labels` while a config is loaded;
  - when `label.hold` is set, draw a 5 px circle at the top-right inside the cap, coloured `#6aa0ff`. This is needed for the US1 test (Caps shows `Esc` with a hold dot).
- [ ] T018 [US1] Manually validate quickstart.md §2 (SC-001, SC-002: the debug log from T016 shows relabel times well under 100 ms) and add a "Validation log" section to the end of `specs/002-key-action-labels/quickstart.md`.

**Checkpoint**: The MVP. Real actions are shown for plain keys and switch with the layer.

---

## Phase 4: User Story 2 - Readable labels for complex actions (Priority: P2)

**Goal**: Every action kind in FR-006 gets its short label. `deflayermap` is supported. Tap-hold keys show a hold dot and aliases a second line.

**Independent Test**: quickstart.md §3. `examples/kanata/all-actions.kbd` shows `⌃C`, `↥nav`, `⇒game`, a tap-hold with a dot, an alias with `@name`, `¹⇧`, `λ`, `⇪W`, a shortened macro, and a disabled key.

### Tests for User Story 2

- [ ] T019 [P] [US2] Write unit tests in `crates/kbd/src/action.rs` (`#[cfg(test)]`), one per row of research R4:
  - every modifier prefix in `KEYMODI` (`C-`, `RC-`, `S-`, `RS-`, `A-`, `RA-`, `AG-`, `M-`, `RM-`, and the symbol forms `‹⌃`, `⌃›`, `‹⇧`, `⇧›`, `‹⌥`, `⌥›`, `‹◆`, `◆›`, `‹⌘`, `⌘›`, `‹❖`, `❖›`, `‹⎈`, `⎈›`, `‹⎇`, `⎇›`, `⎈`, `⌥`);
  - stacked prefixes `C-S-t`;
  - `(multi lctl c)` → `Combo`, `(multi a b)` → `Other`;
  - `layer-switch`, `layer-while-held`, `layer-toggle`;
  - every `tap-hold*` / `tap⬓*` name from `list_actions.rs`;
  - `one-shot*`;
  - `(unicode λ)` and `🔣λ`;
  - `caps-word`, `caps-word-custom`;
  - `(macro a b c)` → `Other`.
- [ ] T020 [P] [US2] Write unit tests in `crates/kbd/src/label.rs` for every row of contracts/labels.md: `⌃C`, `⌃⇧T` (order `⌃ ⌥ ⇧ ◆`), `⌥›E`, `↥nav`, `⇒game`, `¹⇧`, `λ`, `⇪W`, the `(macro…` shortening, and a long layer name shortened.
- [ ] T021 [P] [US2] Write unit tests in `crates/kbd/src/reader.rs` for `deflayermap`:
  - pairs;
  - the wildcard `_` fills unmapped `defsrc` keys;
  - `___` fills all keys;
  - `__` is ignored;
  - keys missing from the map are `Transparent`;
  - a key not in `defsrc` is ignored.

### Implementation for User Story 2

- [ ] T022 [US2] Extend `classify` in `crates/kbd/src/action.rs` to every row of research R4:
  - modifier prefixes, by repeatedly stripping the longest matching prefix in `KEYMODI` order, longer symbol forms before `⎈` / `⌥`. A bare modifier key name (`lctl`) stays `Key`;
  - `multi` → `Combo` when every part but the last canonicalises to a modifier key (`KEY_LEFTCTRL` …), else `Other`;
  - `layer-switch` → `LayerSwitch`; `layer-while-held` and `layer-toggle` → `LayerHold`;
  - `one-shot*` → `OneShot(classify(last arg))`;
  - `unicode` / the `🔣` prefix → `Unicode`;
  - `caps-word*` → `CapsWord`;
  - everything else stays `Other(raw)`.
- [ ] T023 [US2] Extend the label text in `crates/kbd/src/label.rs`, following contracts/labels.md:
  - `Combo` → modifier symbols in the order `⌃ ⌥ ⇧ ◆`, each followed by `›` when it is right-sided, then the key label;
  - `LayerHold` → `↥` + name; `LayerSwitch` → `⇒` + name;
  - `OneShot` → `¹` + inner label;
  - `Unicode` → its characters;
  - `CapsWord` → `⇪W`;
  - layer names and raw text shortened to 8 characters + `…`.
- [ ] T024 [US2] Implement `deflayermap (NAME) key action …` in `crates/kbd/src/reader.rs` (research R3): build a `Layer` in `defsrc` order from the pairs, and apply the wildcards `_` / `___` to keys that are still unmapped. Unmapped keys become `Transparent`.
- [ ] T025 [P] [US2] Create `examples/kanata/all-actions.kbd`: the same `defsrc` as `minimal.kbd`, a base layer whose first row has one key per FR-006 kind (`C-c`, `(layer-while-held nav)`, `(layer-switch game)`, `@th` as a tap-hold, `@cp` as an alias for `C-v`, `(one-shot 500 lsft)`, `(unicode λ)`, `caps-word`, `(macro h e l l o)`, `XX`), plus `nav` and `game` layers defined with `deflayermap`. Check it with `just kanata-check examples/kanata/all-actions.kbd`, then add a fixture test in `crates/kbd/src/label.rs` that `include_str!`s it and asserts each label (SC-004).
- [ ] T026 [US2] In `crates/app/qml/KeyCap.qml`, when `label.alias` is set, add a second `Text` under the label: `"@" + label.alias`, 9 px, `#6a6a6a`, elided. It must stay inside the cap at every scale (FR-007). The hold dot is drawn in T017.
- [ ] T027 [US2] Manually validate quickstart.md §3 (SC-004) and add the result to the Validation log in `specs/002-key-action-labels/quickstart.md`.

**Checkpoint**: US1 and US2 work.

---

## Phase 5: User Story 3 - Labels follow config changes (Priority: P3)

**Goal**: A Kanata config reload re-reads the file. A failure keeps the old labels. Kanata switching to another file is reported.

**Independent Test**: quickstart.md §4 with real Kanata. Edit `nav`, reload, and see the new label within 2 s. A broken file keeps the old labels and shows a notice.

- [ ] T028 [US3] Handle `ServerMessage::ConfigFileReload { new }` in `spawn_connection` in `crates/app/src/overlay_state.rs`. Pass `config_path: Option<PathBuf>` into the thread, and when the message arrives:
  - if `config_path` is set, call `kbd::read(path)` on the connection thread;
  - push one `Update` that, on `Ok`, replaces the stored config, calls `relabel()` and adds new warnings as notices, and on `Err(e)` keeps the old config and adds the notice `config: cannot read {path}: {e}` (FR-012);
  - if `std::fs::canonicalize(new)` differs from `canonicalize(path)`, also add the notice `Kanata switched to {new}; the overlay still reads {path}` (spec US3 scenario 3).

  Log at debug level: "config reloaded". Never log config text.
- [ ] T029 [US3] Manually validate quickstart.md §4 with real Kanata (SC-003) and add the result to the Validation log in `specs/002-key-action-labels/quickstart.md`.

**Checkpoint**: All stories work.

---

## Phase 6: Polish & Cross-Cutting Concerns

- [ ] T030 [P] Update `README.md`:
  - the "Current state" note;
  - a short "Key labels" paragraph under "Using the overlay": `--config`, `just run` passing the example config, what the label symbols mean (`↥` momentary layer, `⇒` layer switch, the blue dot for a hold action, grey for inherited), and the supported subset (with `include` and `deftemplate` skipped);
  - roadmap step 2 marked as done.
- [ ] T031 [P] Update `CLAUDE.md` › Layout with `crates/kbd` ("Kanata .kbd subset parser and key labels; no Qt; `key_names.rs` is generated by `scripts/gen-key-names.py`").
- [ ] T032 Run the privacy check in quickstart.md §5 (FR-013). Also check FR-004: the config file's mtime and `sha256sum` are unchanged after a run that included a reload.
- [ ] T033 Run `just check` and fix every finding.
- [ ] T034 Review spec.md FR-001 – FR-013 against the implementation and the Validation log, and note any gaps in `specs/002-key-action-labels/quickstart.md`.

---

## Dependencies & Execution Order

- **Setup (T001–T004)** → **Foundational (T005–T011)** → stories.
- **US1 (T012–T018)**: needs the Foundational phase. T013 → T014 → T015 → T016 → T017.
- **US2 (T019–T027)**: needs US1's `classify` and `labels` (T013–T015). The QML task T026 needs T017.
- **US3 (T028–T029)**: needs T016 (stored config, `relabel()`). Independent of US2.
- **Polish (T030–T034)**: after the stories you want to ship.

### Within phases

- T003 → T008 (generated table before `canonical`).
- T005 → T009 (lexer before reader).
- T007 → T009, T013, T014 (types first).
- The test tasks (T006, T012, T019–T021) can be written before their implementations (TDD).

### Parallel Opportunities

- Setup: T003 and T004.
- Foundational: T005/T006 (lexer), T008 (after T003), T010 (main.rs) and T011 (overlay_state.rs layout JSON) touch different files.
- US2: T019, T020, T021 (tests in three files) and T025 (fixture) can run together.

## Parallel Example: Foundational

```bash
Task: "T005 Lexer in crates/kbd/src/lexer.rs"
Task: "T010 --config in crates/app/src/main.rs"
Task: "T011 kanataCanonical in crates/app/src/overlay_state.rs"
```

## Parallel Example: User Story 2

```bash
Task: "T019 classify tests in crates/kbd/src/action.rs"
Task: "T020 label tests in crates/kbd/src/label.rs"
Task: "T021 deflayermap tests in crates/kbd/src/reader.rs"
Task: "T025 examples/kanata/all-actions.kbd fixture"
```

---

## Implementation Strategy

### MVP First (User Story 1)

1. Setup → Foundational. `cargo test -p kbd` is green.
2. US1 → quickstart §2 → commit.

### Incremental Delivery

1. US2 (complex actions, `deflayermap`, hold dot, alias line) → §3 → commit.
2. US3 (reload) → §4 with real Kanata → commit.
3. Polish → docs, privacy, `just check` → PR to `main`.

## Notes

- The manual tasks T018, T027 and T029 are the manual test notes constitution V requires.
- T029 needs real Kanata (`just kanata`, which needs the input/uinput permissions, see `just check-perms`). The fake server doesn't send `ConfigFileReload`.
