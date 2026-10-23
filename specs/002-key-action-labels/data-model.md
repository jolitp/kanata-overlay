# Data Model: Key Action Labels

**Feature**: [spec.md](spec.md) | **Research**: [research.md](research.md)

All types live in the new Qt-free crate `crates/kbd` unless noted.

## Config

What the overlay understands of one `.kbd` file.

| Field | Type | Notes |
|-------|------|-------|
| `source` | list of `KeyName` | From `defsrc`, in order |
| `layers` | list of `Layer` | In file order; the first one is the default layer (FR-010) |
| `aliases` | map name → `Action` | From every `defalias` block; a later definition wins, with a warning |
| `delegate_to_first_layer` | bool | From `defcfg`. With the R6 approximation it changes nothing today; it is stored for a future stack-aware model. |
| `warnings` | list of `ConfigWarning` | Problems that didn't stop parsing |

**Validation**:
- No `defsrc` → `ConfigError::NoDefsrc`. No labels can be made, and the legends are shown.
- No layers → `ConfigError::NoLayers`.
- A syntax error (unbalanced parenthesis) → `ConfigError::Syntax { line, column }`. A parse with this error yields no `Config`; the previous one is kept on reload (FR-012).

## KeyName

A key name as written, plus its canonical form.

| Field | Type | Notes |
|-------|------|-------|
| `written` | string | E.g. `enter` |
| `canonical` | string | E.g. `KEY_ENTER`, from the generated table (research R5); unknown names are kept as written |

## Layer

| Field | Type | Notes |
|-------|------|-------|
| `name` | string | |
| `actions` | list of `Action` | One per `source` position. Short layers are padded with `Transparent`; extra entries are dropped, with a `ConfigWarning::LayerLength` |

## Action

| Variant | Data | From |
|---------|------|------|
| `Key` | `KeyName` | `a`, `ret`, `⏎` |
| `Combo` | modifiers (Ctrl, Shift, Alt, Super, each left or right) + `KeyName` | `C-c`, `‹⇧a`, or `(multi lctl c)` when it is only modifiers and a key |
| `Transparent` | — | `_` `‗` `≝` |
| `NoOp` | — | `XX` `✗` `∅` `•` |
| `UseDefsrc` | — | `use-defsrc` |
| `Alias` | name | `@name` |
| `LayerSwitch` | layer name | `(layer-switch nav)` |
| `LayerHold` | layer name | `(layer-while-held nav)`, `(layer-toggle nav)` |
| `TapHold` | tap `Action`, hold `Action` | any `tap-hold*` / `tap⬓*` (arguments 3 and 4) |
| `OneShot` | `Action` | `one-shot*` |
| `Unicode` | string | `(unicode λ)`, `🔣λ` |
| `CapsWord` | — | `caps-word*` |
| `Other` | raw text | anything else (macros, tap-dance, mouse…) |

## ConfigWarning

Everything that is shown in one notice, without config text (FR-013):
- `Skipped { forms }`, e.g. "include, deftemplate";
- `LayerLength { layer, expected, found }`;
- `DuplicateAlias { name }`;
- `UnknownAlias { name }`;
- `AliasLoop { name }`.

## KeyLabel

What one physical key shows for the active layer.

| Field | Type | Notes |
|-------|------|-------|
| `text` | string | Short label (contracts/labels.md); may be empty for `NoOp` |
| `style` | `normal` / `inherited` / `disabled` | `unbound` is decided in QML (the key has no Kanata name or isn't in `source`) |
| `hold` | bool | The key is a tap-hold with a hold action (Q2 = C: a marker only) |
| `alias` | string or empty | The alias name, when the action came from `@name` (US2 scenario 3) |

## Label resolution

```
labels(config, active_layer_name) -> map canonical-name → KeyLabel

layer  = config.layer(active_layer_name) or config.layers[0]   (FR-010; unknown name → warning)
for each position i, source key k:
    a = layer.actions[i]
    resolve aliases in a (depth ≤ 8; a loop shows "@name" with a warning)
    if a is Transparent (after alias resolution):
        a = (layer is first) ? Key(k) : first_layer.actions[i] (resolved);
        if that is Transparent too → Key(k)
        style = inherited
    if a is UseDefsrc → Key(k), style = inherited
    label = format(a)              (contracts/labels.md)
    out[k.canonical] = label       (two physical keys with the same name share it)
```

## Config lifecycle *(app crate)*

```
no --config ───────────────────────────────▶ Legends (today's behaviour, no notice)
--config P ── read OK ─────────────────────▶ Loaded(config)  ── LayerChange ─▶ relabel
           └─ read/parse fails ────────────▶ Legends + notice "cannot read P: …"
Loaded ── ConfigFileReload{new} ── re-read P OK ──▶ Loaded(new config)  (+ notice if new ≠ P)
                                └─ fails ─────────▶ Loaded(old config) + notice   (FR-012)
```
