#!/usr/bin/env python3
"""Regenerate crates/kbd/src/key_names.rs from Kanata's key-name table.

Usage (inside `nix develop`):

    python3 -I scripts/gen-key-names.py \
        "$(nix build --inputs-from . nixpkgs#kanata.src --no-link --print-out-paths)" \
        > crates/kbd/src/key_names.rs

Reads `DEFAULT_MAPPINGS` and `str_to_oscode` in parser/src/keys/mod.rs and emits every spelling Kanata
accepts with the OsCode it means, keeping arms without `#[cfg]` and arms whose
`#[cfg]` includes linux (research R5).
"""

import pathlib
import re
import sys

src = pathlib.Path(sys.argv[1])
version = re.search(r'^version = "([^"]+)"', (src / "Cargo.toml").read_text(), re.M).group(1)
text = (src / "parser/src/keys/mod.rs").read_text()
body = text[text.index("pub fn str_to_oscode"):]
body = body[: body.index("\n}\n")]

def unescape(name):
    return name.encode().decode("unicode_escape").encode("latin-1").decode("utf-8")

pairs = {}
# Punctuation and a few words live in a separate default table.
defaults = text[text.index("const DEFAULT_MAPPINGS"):]
defaults = defaults[: defaults.index("];")]
for name, code in re.findall(r'\("((?:[^"\\]|\\.)*)",\s*OsCode::(\w+)\)', defaults):
    pairs.setdefault(unescape(name), code)

pending = ""
cfg = None
for line in body.splitlines():
    stripped = line.strip()
    if stripped.startswith("//"):
        continue
    if stripped.startswith("#[cfg"):
        cfg = stripped
        continue
    pending += " " + stripped
    m = re.search(r"=>\s*OsCode::(\w+)", pending)
    if not m:
        continue
    keep = cfg is None or "linux" in cfg
    if keep:
        for name in re.findall(r'"((?:[^"\\]|\\.)*)"', pending[: m.start()]):
            pairs.setdefault(unescape(name), m.group(1))
    pending = ""
    cfg = None

def rust_str(s):
    return '"' + s.replace("\\", "\\\\").replace('"', '\\"') + '"'

print(f"// GENERATED from kanata {version} parser/src/keys/mod.rs by scripts/gen-key-names.py — do not edit.")
print("//! Every key name Kanata accepts, with the OsCode it means (sorted for binary search).")
print()
print("#[rustfmt::skip]")
print("pub(crate) static KEY_ALIASES: &[(&str, &str)] = &[")
for name in sorted(pairs, key=lambda s: s.encode()):
    print(f"    ({rust_str(name)}, {rust_str(pairs[name])}),")
print("];")
