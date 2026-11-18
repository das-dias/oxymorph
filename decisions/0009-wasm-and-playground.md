# 0009. WebAssembly target and docs playground

- Status: implemented
- Date: 2026-02-11
- Related specs: `specs/bindings.md`

## Context

Decision 0001 lists WASM as a deployment target. The goal additionally requires a
`wasm32-unknown-unknown` build and an in-browser playground on the docs site:
a textarea input, a read-only textarea output, and a dropdown for each netlist
flavor.

## Decision

- Add `crates/oxymorph-wasm`, a `wasm-bindgen` crate compiled to
  `wasm32-unknown-unknown` (excluded from the default workspace, like the PyO3
  crate). It exposes `formats()`, `convert(text, in_format, out_format)`,
  `to_json(text, in_format)` and `version()`, delegating to
  `oxymorph-netlist` — no logic of its own.
- The docs site ships a **Playground** page. The built `wasm-pack --target web`
  module is committed under `docs/assets/playground/` so the static site works
  without a Rust build step; `tools/build_playground.sh` regenerates it.
- CI builds the crate for `wasm32-unknown-unknown` and the `wasm-pack` bundle, so
  the target does not silently rot.

## Consequences

Easier: the parser is usable in the browser and the playground doubles as a live
demonstration of the port.

Harder / owed: the committed wasm artifact must be regenerated when the Rust
changes (`tools/build_playground.sh`); the CI build keeps it honest.

## Verification

`cargo build --manifest-path crates/oxymorph-wasm/Cargo.toml --target wasm32-unknown-unknown`
succeeds; a Node build (`wasm-pack --target nodejs`) round-trips a netlist through
`convert`; the built site contains `site/playground/index.html` and
`site/assets/playground/oxymorph_wasm_bg.wasm`.
