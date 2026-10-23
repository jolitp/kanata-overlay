# Contract: Settings File

**Path**: `$XDG_CONFIG_HOME/kanata-overlay/settings.json`, or
`$HOME/.config/kanata-overlay/settings.json` when `XDG_CONFIG_HOME` is unset or empty. You
can change it for one run with `--settings PATH` (see [cli.md](cli.md)).

**Encoding**: UTF-8 JSON object, pretty-printed with two-space indentation and a trailing
newline.

## Version 1

```json
{
  "version": 1,
  "edge": "bottom",
  "margin": 16,
  "opacity": 85,
  "scale": 100,
  "screen": null
}
```

Field types, ranges and defaults are listed in [data-model.md › Settings](../data-model.md#settings).

## Guarantees

- **Read**: The app never refuses to start because of this file. Bad fields fall back to
  their defaults, and the app warns once.
- **Write**: The file is written atomically (temp file in the same directory, then rename),
  and the directory is created if it is missing. Every field is written, including the
  defaults.
- **Never written**: Command-line overrides, visibility state, and any key-press data
  (constitution VI).
- **Newer version**: A file with `version` > 1 is not overwritten unless the user saves from
  the settings window.
- **Compatibility**: Future versions only add fields or bump `version`. A v1 reader ignores
  unknown fields.
