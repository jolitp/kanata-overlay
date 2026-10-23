# Contract: Labels, CLI option, and QML bridge

## Label text (`kbd::format`)

| Action | Label `text` | Other fields |
|--------|--------------|--------------|
| `Key` letter `a`–`z` | upper case: `A` | |
| `Key` digit or punctuation | as is: `1`, `;`, `/` | |
| `Key` with a glyph | `KEY_ENTER ⏎`, `KEY_BACKSPACE ⌫`, `KEY_DELETE ⌦`, `KEY_TAB ⇥`, `KEY_SPACE ␣`, `KEY_ESC Esc`, `KEY_CAPSLOCK ⇪`, `KEY_LEFT ←`, `KEY_RIGHT →`, `KEY_UP ↑`, `KEY_DOWN ↓`, `KEY_PAGEUP ⇞`, `KEY_PAGEDOWN ⇟`, `KEY_HOME ⇱`, `KEY_END ⇲`, `KEY_INSERT Ins`, `KEY_F1…F24 F1…F24`, shift `⇧`, control `⌃`, alt `⌥`, super/meta `◆` (right-hand versions add `›`, e.g. `⇧›`), `KEY_VOLUMEUP 🔊+`, `KEY_VOLUMEDOWN 🔊−`, `KEY_MUTE 🔇`, `KEY_PLAYPAUSE ⏯` | |
| `Key`, any other | the name as written in the config | |
| `Combo` | modifier symbols in the order `⌃ ⌥ ⇧ ◆`, then the key label: `⌃C`, `⌃⇧T`, `⌥›E` | |
| `Transparent` | the inherited action's label | `style: inherited` |
| `NoOp` | empty | `style: disabled` |
| `UseDefsrc` | the source key's label | `style: inherited` |
| `Alias` | the resolved action's label | `alias: "name"` |
| `LayerHold` | `↥` + layer name, e.g. `↥nav` | |
| `LayerSwitch` | `⇒` + layer name, e.g. `⇒game` | |
| `TapHold` | the **tap** action's label | `hold: true` |
| `OneShot` | `¹` + the inner label, e.g. `¹⇧` | |
| `Unicode` | the character(s) | |
| `CapsWord` | `⇪W` | |
| `Other` | the raw form, shortened to 8 characters + `…`, e.g. `(macro…` | |
| Alias loop or unknown alias | `@name` | `style: normal`, plus a warning |

Shortening applies to layer names and raw text. The QML side also fits text to the key
(FR-007).

## CLI option

```
kanata-overlay … [--config PATH]
```

- **Read**: the file at PATH is read at start-up and again on each `ConfigFileReload`. The path is not saved (clarification Q1 = C).
- **Missing option**: printed legends, no notice.
- **Unreadable or unparseable file**: printed legends, plus the notice `config: cannot read PATH: <reason>`. The reason is an I/O error, or a syntax error as `line L, column C`, never config text.
- **`just run`**: passes `--config $KANATA_CFG` (default `examples/kanata/minimal.kbd`).

## QML bridge (`OverlayState`, extended)

| Property | Type | Meaning |
|----------|------|---------|
| `labelsJson` | QString | JSON object: canonical key name → `{ "text": str, "style": "normal"\|"inherited"\|"disabled", "hold": bool, "alias": str }`. `{}` when there is no config. |

**Key lookup**: each layout key carries a canonical name, `kanataCanonical`. It is added
to the layout JSON in Rust through the same table (research R5), so QML never
canonicalises.

`KeyCap.qml`:
- `label = labels[key.kanataCanonical]`;
- if there is no label, the key shows today's legend. It gets the unbound (orange) style if it has no binding, or if a config is loaded and its name isn't in `defsrc` (FR-008). With no config loaded, bound keys keep the normal style;
- **text**: `label.text`, with `fontSizeMode: Text.Fit`, `minimumPixelSize: 7` and `elide: Text.ElideRight`;
- **colour**: normal `#e8e8e8`, inherited `#8a8a8a`, disabled `#555`;
- **hold**: when `label.hold` is set, a 5 px dot in the top-right corner, coloured `#6aa0ff`;
- **alias**: when `label.alias` is set, a 9 px second line `@alias`, coloured `#6a6a6a`.
