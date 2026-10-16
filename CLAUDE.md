# kanata-overlay — agent guide

On-screen keyboard overlay for the Kanata remapper. Rust + Qt 6/QML (cxx-qt), KDE Plasma first.
Project principles: `.specify/memory/constitution.md` (read before planning).

## Layout

- `crates/kanata-client` — Kanata TCP client (`Client`, `Incoming`) and process launcher (`LaunchOptions`). No Qt.
- `crates/layout` — physical layouts: KLE import, own JSON format, legend → Kanata key-name guessing. No Qt.
- `crates/app` — the `kanata-overlay` binary: cxx-qt bridge (`src/overlay_state.rs`) + QML (`qml/`).
- `examples/` — sample KLE layout and Kanata config used by `just run` / `just kanata`.
- `specs/` — Spec Kit feature specs (created by `/speckit-specify`).

## Commands (inside `nix develop`)

- `just check` — fmt-check, clippy `-D warnings`, tests. Must pass before committing.
- `just run` — run the overlay against `127.0.0.1:7070`.
- `just fake-kanata [layer]` — fake Kanata server, no permissions needed.
- `just kanata` / `just watch` — real Kanata with TCP, and a raw event printer.

## Conventions

- Logic in library crates, QML only renders (constitution I).
- QObject updates from other threads go through `qt_thread().queue(...)`.
- Bridge blocks use `#[auto_cxx_name]` so QML sees camelCase names.
- Never re-define Kanata protocol types; use `kanata_client::protocol`.
- Never log or persist pressed-key data (constitution VI).
