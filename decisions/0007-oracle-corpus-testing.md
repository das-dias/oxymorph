# 0007. Oracle-corpus testing strategy

- Status: implemented
- Date: 2026-02-11
- Related specs: `specs/parity.md`

## Context

Decision 0004 requires bug-for-bug parity, and `specs/parity.md` requires a
golden corpus plus a non-blocking differential harness. Core tests must run
without Python.

## Decision

**Capture the oracle once into committed fixtures; assert offline.**

- `tools/extract_oracle.py` (run against the vendored Python) harvests committed
  netlists and every inline netlist in upstream's test suite, runs upstream as
  the oracle, and writes `tests/corpus/cases.json` with expected IR (JSON) and
  writer output for every format.
- `crates/oxymorph-netlist/tests/parity.rs` replays the corpus against the Rust
  implementation with **no Python toolchain**.
- `tools/generate_equivalence_docs.py` additionally compares the Python binding
  against upstream live, and generates `docs/equivalence.md`.

## Options considered

- **Run upstream Python in core CI** — rejected: core tests must not need Python
  (`decisions/0003`).
- **Hand-written expectations** — rejected: does not scale and is not the oracle.

## Consequences

Easier: offline, fast, deterministic; the corpus is the definition of coverage.

Harder / owed: the corpus is refreshed by re-running the extractor per re-vendor;
the live binding check and any fuzzing remain separate, non-blocking work.

## Verification

`cargo test --workspace` runs the parity test (357 parse cases, 2142 writer
outputs); the live check reports the same tally.
