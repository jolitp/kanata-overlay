# Data Model: Overlay Window

**Feature**: [spec.md](spec.md) | **Research**: [research.md](research.md)

All types below live in the Qt-free `settings` crate (`crates/settings`) unless noted.

## Edge

The screen edge the overlay is centred along.

| Value | JSON | CLI |
|-------|------|-----|
| Top | `"top"` | `top` |
| Bottom (default) | `"bottom"` | `bottom` |
| Left | `"left"` | `left` |
| Right | `"right"` | `right` |

## Settings

The persisted user preferences (FR-007 – FR-011, FR-019). No key-press data, ever.

| Field | Type | Range / values | Default | Requirement |
|-------|------|----------------|---------|-------------|
| `version` | integer | `1` | `1` | — |
| `edge` | Edge | see above | `bottom` | FR-007 |
| `margin` | integer (logical px) | 0 – 500 | `16` | FR-007 |
| `opacity` | integer (%) | 10 – 100 | `85` | FR-008 |
| `scale` | integer (%) | 50 – 300 | `100` | FR-009 |
| `screen` | string or null | Qt screen name (e.g. `"DP-1"`); `null` = primary | `null` | FR-010 |

**Validation (lenient, per field)**:
- Each field is checked on its own. A missing field takes its default silently. A field with
  the wrong type or out of range takes its default and adds a `SettingsWarning::InvalidField
  { field, reason }`.
- Unknown fields are ignored. They are not preserved on save, which is acceptable for v1.

## LoadOutcome

The result of reading the settings file.

| Field | Type | Meaning |
|-------|------|---------|
| `settings` | Settings | What to use. |
| `warnings` | list of SettingsWarning | Shown once as a notice (and logged). |
| `writable` | bool | `false` when the file is unreadable, isn't JSON, or has `version` > 1. The file is then not overwritten until an explicit save from the settings window. |

**States**:

```
file missing ──────────────▶ defaults, no warning, writable
file valid v1 ─────────────▶ parsed, writable
file v1 with bad fields ───▶ parsed + per-field defaults + warnings, writable
file unreadable / not JSON ▶ defaults + warning, NOT writable
file version > 1 ──────────▶ defaults + warning, NOT writable
NOT writable ── user saves from settings window ──▶ writable (file replaced)
```

## Overrides

One run's command-line values (FR-011d). Each field is optional and has the same type and
range as in Settings.

- `effective = settings ⊕ overrides`: the override wins wherever one is set.
- Editing a field in the settings window clears that field's override and updates
  `settings`. Only `settings` is ever saved.
- An invalid CLI value is rejected at parse time with an error message, and the app exits
  with status 2. This applies only to the command line; the file is always lenient.

## Placement

The output of the pure placement functions. QML uses it on X11 and for scale capping.

| Input | Type |
|-------|------|
| screen geometry | `Rect { x, y, width, height }` (logical px) |
| base content size | `Size { width, height }` at 100% scale (logical px) |
| edge, margin, scale | from effective Settings |

| Output | Type | Meaning |
|--------|------|---------|
| `scale` | f64 | The effective scale factor: `scale% / 100`, reduced to the largest value that fits `screen − 2·margin` along both axes. |
| `capped` | bool | True if the scale was reduced, which raises a notice. |
| `size` | Size | `base × scale`, rounded. |
| `x`, `y` | i32 | The top-left corner: centred along the edge, `margin` from it, inside the screen. Used only by the X11 / fallback window; layer-shell uses anchors and margins instead. |

## Capabilities *(app crate)*

Detected once at start-up (R4) and used to pick the overlay variant and to fill FR-013 notices.

| Field | Type | Meaning |
|-------|------|---------|
| `platform` | `Wayland` / `X11` / `Other(String)` | From Qt's platform name. |
| `layer_shell` | bool | `zwlr_layer_shell_v1` was advertised (Wayland only). |

The "missing features" list is derived from these values:
- On X11, click-through needs XShape, and "on top" is best-effort.
- On Wayland without layer-shell: not always on top, not on all desktops, no edge placement,
  and click-through only.

## OverlayVisibility *(app, not persisted)*

```
            toggle shortcut                   toggle shortcut
  Shown ───────────────────────▶ Hidden ───────────────────────▶ Shown

  Hidden ── settings window opens ──▶ Previewing (shown) ── settings window closes ──▶ Hidden
  Shown  ── settings window opens/closes ──▶ Shown (no change)
```

- On start-up the state is always **Shown** (spec assumption).
- The Kanata connection and layer state keep updating while the overlay is hidden (FR-016).

## GlobalShortcut *(app)*

| Field | Value |
|-------|-------|
| `id` | `toggle-overlay` / `open-settings` |
| `text` | "Show/hide overlay" / "Open overlay settings" (shown in System Settings) |
| `default keys` | `Meta+Alt+K` / `Meta+Alt+Shift+K` |
| `registered` | bool. When false, a notice is shown (FR-017). |
