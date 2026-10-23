# Contract: Command Line

```
kanata-overlay [--connect ADDR] [--layout FILE]
               [--settings FILE]
               [--edge top|bottom|left|right|top-left|top-right|bottom-left|bottom-right] [--margin PX]
               [--opacity PERCENT] [--scale PERCENT] [--screen NAME]
```

| Option | Existing / new | Value | Effect |
|--------|----------------|-------|--------|
| `--connect`, `-c` | existing | `host:port` | Kanata TCP address (default `127.0.0.1:7070`). |
| `--layout`, `-l` | existing | path | KLE or own-format layout. |
| `--settings` | new | path | Use this settings file instead of the default path, for both reading and saving. |
| `--edge` | new | `top` `bottom` `left` `right` `top-left` `top-right` `bottom-left` `bottom-right` | Override for this run only. |
| `--margin` | new | integer 0–500 | Override for this run only. |
| `--opacity` | new | integer 10–100 | Override for this run only. |
| `--scale` | new | integer 50–300 | Override for this run only. |
| `--screen` | new | Qt screen name, or `primary` | Override for this run only. |

## Rules

- An override never changes the settings file (FR-011d). If the user edits the same field in
  the settings window, the edit replaces the override and is saved.
- An invalid value (unknown edge, a number out of range or not a number, a missing value)
  prints `kanata-overlay: invalid --FLAG value 'X': <reason>` to stderr and exits with status
  **2** before any window opens.
- Unknown arguments are still ignored, because Qt consumes its own flags such as `-platform`.
- Parsing lives in the `settings` crate (`Overrides::parse`) so that it can be unit-tested.
  `main.rs` only forwards `std::env::args()`.
