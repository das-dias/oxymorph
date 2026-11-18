# 0003. Crate workspace and layering

- Status: decided
- Date: 2026-02-10
- Supersedes: -
- Superseded by: -
- Related decisions: `0001-rust-port-of-morphnet.md`, `0002-parser-engine.md`
- Related specs: `specs/architecture.md` (TBD)

## Context

Decision 0001 requires a **crate-only repository** (no root `src/`) that can be
embedded in Rust binaries and deployed as a single artifact (including WASM),
with the Python bindings as a **separate thin crate** over the pure-Rust
crate(s).

Decision 0001 also makes the IR **Pydantic-normative** while starting from
prost-generated types, and decision 0002 embeds the six `.lark` grammars and
returns generated per-struct trees. Both of these are easy to violate by
accident if parsing and the IR live in one crate.

## Decision

A Cargo workspace rooted at the repository (`Cargo.toml` with `[workspace]`,
no root `src/`), with members under `crates/`:

| Crate | Layer | Responsibility |
|---|---|---|
| `oxymorph-schema` | data | IR types mirroring the Pydantic models; prost-generated protobuf types and the conversions to/from the IR. **No parsing, no I/O.** |
| `oxymorph-netlist` | parsing | Embedded `.lark` grammars, generated parsers behind our parser trait, preprocessing, transformers, writers. Depends on `-schema`. |
| `oxymorph` | facade | Public Rust API and re-exports; the crate consumers depend on. |
| `oxymorph-python` | bindings | Thin PyO3 layer (`extension-module` cdylib) over `oxymorph`; built with `maturin`. |
| `oxymorph-cli` | binary | Optional CLI. |
| `oxymorph-wasm` | bindings | Optional `wasm-bindgen` crate. |

Rules:

- **Dependency direction is one-way**: `oxymorph-schema` ← `oxymorph-netlist`
  ← `oxymorph` ← {`oxymorph-python`, `oxymorph-cli`, `oxymorph-wasm`}.
  `oxymorph-schema` must never depend on a parser.
- **Formats are feature-gated** in `oxymorph-netlist` (`spice`, `hspice`,
  `xyce`, `spectre`, `spectre-spice`, `vams`), re-exported through `oxymorph`, so
  a Spectre-only consumer does not compile VAMS.
- `oxymorph-python`, `oxymorph-cli` and `oxymorph-wasm` are **excluded from the
  default workspace members** so `cargo test` on the core needs no Python, WASM
  or CLI toolchain (the same exclusion lark-rs uses for its `python/` and
  `wasm/` crates).
- The PyO3 crate provides a Python module named **`oxymorph`**. It is **not**
  API-compatible with upstream `morphnet`; it must expose the **same
  functionality** through its own API.

## Options considered

- **(1) Single core crate + bindings** — fewer moving parts, but nothing stops
  parsing concerns leaking into the IR, and format feature-gating becomes coarse.
- **(2) Layer split (chosen)** — compile-time enforcement of the IR/parsing
  boundary; clean feature gating; matches the Pydantic-normative constraint.
- **(3) Per-format crates on top of (2)** — strongest isolation, but premature;
  revisit only if a format needs separate versioning or heavy optional deps.
- **(4) Layer split now, per-format later** — the chosen option retains this
  path: a format can be promoted to its own crate without changing the layering.

## Consequences

Easier: evolving the IR independently of parsers; testing schema without
grammars; shipping format subsets; building the Python/WASM artifacts without
dragging a Python toolchain into core CI.

Harder / owed:

- More crates to keep coherent; versioning and a shared `[workspace.dependencies]`
  section are needed from day one.
- Feature-gate combinations owe CI coverage (each format compiles standalone and
  together).
- The generated per-struct tree types from `include_lark!` (decision 0002) must
  not leak past `oxymorph-netlist`; the parser trait boundary is what prevents it.

## Verification

- A CI check asserts the dependency direction: `oxymorph-schema`'s dependency
  graph contains no parser/prost-writing crate, using `cargo metadata` (or
  `cargo tree -p oxymorph-schema`).
- `cargo build -p oxymorph --no-default-features --features <fmt>` succeeds for
  each format individually.
- `cargo test` at the workspace root does not require Python, `maturin` or WASM
  targets.
