# kanata-overlay tasks. Run `just` to list them.
# Everything assumes the Nix dev shell (`nix develop`, or direnv with .envrc).

set shell := ["bash", "-euo", "pipefail", "-c"]

kanata_port := env("KANATA_PORT", "7070")
kanata_cfg := env("KANATA_CFG", "examples/kanata/minimal.kbd")
layout := env("LAYOUT", "examples/layouts/sample.kle.json")

# List available recipes
default:
    @just --list --unsorted

# ── build & run ──────────────────────────────────────────────

# Build the whole workspace
build *args:
    cargo build --workspace {{args}}

# Run the overlay against kanata on 127.0.0.1:$KANATA_PORT (default 7070), labelled from $KANATA_CFG
run *args:
    cargo run -p kanata-overlay -- --connect 127.0.0.1:{{kanata_port}} --layout {{layout}} --config {{kanata_cfg}} {{args}}

# Run a release build
release:
    cargo build --workspace --release

# ── quality ──────────────────────────────────────────────────

# Run all tests
test *args:
    cargo test --workspace {{args}}

# Lint with clippy, warnings as errors
lint:
    cargo clippy --workspace --all-targets -- -D warnings

# Format Rust and Nix sources
fmt:
    cargo fmt --all
    nix fmt flake.nix

# Check formatting without changing files
fmt-check:
    cargo fmt --all -- --check

# Everything CI should run
check: fmt-check lint test

# ── kanata ───────────────────────────────────────────────────

# Start kanata with its TCP server (needs input/uinput permissions, see README)
kanata cfg=kanata_cfg:
    kanata --cfg {{cfg}} --port 127.0.0.1:{{kanata_port}} --no-wait

# Validate a kanata config without grabbing the keyboard
kanata-check cfg=kanata_cfg:
    kanata --cfg {{cfg}} --check

# Print everything kanata sends over TCP
watch:
    cargo run -p kanata-client --example watch -- 127.0.0.1:{{kanata_port}}

# Run a fake kanata server that announces a layer change (no permissions needed)
fake-kanata layer="nav":
    python3 -c 'import socket,time; s=socket.socket(); s.setsockopt(socket.SOL_SOCKET,socket.SO_REUSEADDR,1); s.bind(("127.0.0.1",{{kanata_port}})); s.listen(); print("fake kanata on :{{kanata_port}}"); [ (c:=s.accept()[0]).sendall(b"{\"LayerChange\":{\"new\":\"{{layer}}\"}}\n") for _ in iter(int,1) ]'

# Check whether this user can run kanata without root
check-perms:
    #!/usr/bin/env bash
    ok=true
    for g in input uinput; do
        if id -nG | tr ' ' '\n' | grep -qx "$g"; then echo "✓ in group $g"; else echo "✗ not in group $g"; ok=false; fi
    done
    if [ -w /dev/uinput ]; then echo "✓ /dev/uinput writable"; else echo "✗ /dev/uinput not writable"; ok=false; fi
    $ok && echo "kanata can run as $USER" || echo "see README › Running kanata without root"

# ── spec kit ─────────────────────────────────────────────────

# Run the Spec Kit CLI (e.g. `just speckit check`)
speckit *args:
    uvx --from git+https://github.com/github/spec-kit.git specify {{args}}

# Refresh Spec Kit templates and skills to the latest release
speckit-upgrade:
    uvx --from git+https://github.com/github/spec-kit.git specify init --here --force --integration claude --script sh

# ── housekeeping ─────────────────────────────────────────────

# Remove build outputs
clean:
    cargo clean
