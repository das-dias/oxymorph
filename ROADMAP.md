# ROADMAP

Living plan for reaching parity. Sequencing, not commitment — updated as the
port progresses. Policies live in `decisions/`, what the code does lives in
`specs/`; this is the "what's next".

Related: `specs/parity.md` (definition of done), `decisions/0002` (parser
engine, gated on the spike), `decisions/0003` (crate layout).

## Definition of done (per format/direction pair)

A pair is done when R1 in `specs/parity.md` holds for it against the committed
corpus, with no unexplained differential-harness deltas.

## Phase 0 — Foundations

- [ ] Workspace skeleton per `decisions/0003` (crate-only, no root `src/`).
- [ ] `oxymorph-schema`: prost codegen from the vendored `.proto`, IR types,
      alias maps, oneof/union validation, JSON/YAML shape (`decisions/0005`).
- [ ] Parity-harness scaffolding: corpus extraction from the vendored
      tests/`examples/`, canonical protobuf comparison, known-issue allow-list.
- [ ] Parser spike (`decisions/0002` acceptance gate): all six `.lark` files
      compile via `include_lark!`; trees match Python Lark under the **basic**
      lexer, probing keyword-vs-`IDENT` and `%ignore`/`_NL` cases.

If the spike fails, stop and revise `decisions/0002` before Phase 1.

## Phase 1 — First vertical slice

- [ ] `spectre` parse (smallest independent grammar) end-to-end to IR.
- [ ] `vams` write (reproduces the README quickstart) and its round-trip.
- [ ] Wire the slice into the differential harness.

## Phase 2 — SPICE family

Base `spice` parse/write first, then the derived formats (broadest coverage per
unit effort):

- [ ] `spice` parse + write.
- [ ] `hspice` (derived).
- [ ] `xyce` (derived; adds `PARAMS:` and `{expr}`).
- [ ] `spectre-spice` (derived).

## Phase 3 — Remaining formats

- [ ] `vams` parse (longest transformer).
- [ ] Remaining writers for every format.
- [ ] `spectre` write.

## Phase 4 — Bindings and targets

- [ ] `oxymorph-python` (PyO3, module `oxymorph`, same functionality as
      `morphnet`, not API-compatible).
- [ ] `oxymorph-cli` (if wanted).
- [ ] `oxymorph-wasm` (if wanted).

## Phase 5 — Migration / divergence

- [ ] Migrate prost-based IR to hand-written idiomatic types (`decisions/0005`
      follow-up; requires its own ADR).
- [ ] Consider SPLIT per-format crates (`decisions/0003` option 3) if needed.
- [ ] Begin deliberate divergence: batch-fix logged upstream issues
      (`specs/parity.md` R4) under a divergence ADR.

## Open items

- `specs/ir.md` — JSON/YAML shape, union/presence semantics.
- `specs/parsing.md` — parser trait, preprocessing, error mapping.
- `specs/architecture.md` — crate topology in prose.
- `specs/parity.md` Q1–Q3 (byte-exact writers, error-kind mapping, wire
  byte-exactness).
