# hook

Floating widget that lets the user point at their screen and hand the feedback to coding agents over MCP.
See README.md (Spanish) for the product; this file is for whoever changes the code.

## Conventions

- Source code in English (identifiers, logs, errors, CLI, tests). Only the widget UI text is Spanish.
- No code comments. `rustfmt` at 200 columns (`cargo fmt --all`).
- Keep the binary small: add a dependency or a crate feature only when hook uses it.

## Workspace

- `crates/hook-core`: marks, sessions, threads, on-disk store, images, IPC.
- `crates/hook-mcp`: stdio MCP server (`hook_*` tools).
- `crates/hook-app`: the `hook` binary (pill, dock, overlay, capture, hotkeys) plus test tools
  (`--suite`, `--stress`, `--mark`, `--probe`, `--preview`, `--morph`).
- `crates/hook-ui`: hook's own minimal UI engine. It is a trimmed copy of the sherpa engine
  (`~/self/projects/sherpa`, github.com/hor4z/sherpa): core (Ui, State, animations), text (Geist fonts,
  shaping, raster), the wgpu painter and the text field. It only carries what hook uses.

## Growing hook-ui

hook-ui must not pull the whole sherpa engine back in. When hook needs something that is not there:

- **An icon**: copy only that Lucide SVG from sherpa with `scripts/add-icon.sh <name> [...]`
  (set `SHERPA=/path/to/sherpa` if it is not at `../sherpa`). Icons are embedded at build time from
  `crates/hook-ui/icons/`.
- **A widget or engine feature** (e.g. a slider, scrolling helpers, semantics): copy just that piece
  from sherpa's `crates/sherpa-widgets` or `crates/sherpa-core`, rewire `sherpa_core::`/`sherpa_text::`
  paths to `crate::core::`/`crate::text::`, and drop whatever it does not need.
- **A fix that also belongs in sherpa**: make it in sherpa too, so both engines stay close until they
  are replaced by a shared engine (see docs/IDEAS.md).

Never add sherpa as a dependency again; the release builds on CI must not need access to other repos.

## Build and test

```bash
cargo build --workspace --release
cargo test --workspace --release
```

Every build needs `cmake` (whisper.cpp for dictation). Linux also needs `libpipewire-0.3-dev libspa-0.2-dev libasound2-dev libclang-dev`.
