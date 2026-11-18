# ROADMAP

Living plan for reaching parity. Sequencing, not commitment — updated as the
port progresses. Policies live in `decisions/`, what the code does lives in
`specs/`; this is the "what's next".

Related: `specs/parity.md` (the equivalence contract), `decisions/0002` (parser
engine), `decisions/0003` (crate layout), `decisions/0004` (bug-for-bug parity).

## Definition of done

A surface (and, for parse/write, each format) is done when its `R1` requirement
in `specs/parity.md` holds against the committed corpus, with no unexplained
differential-harness deltas.

## Testing plan — how equivalence is proven

The governing principle: **the Python implementation is the oracle, its verdict
is captured once into committed fixtures, and the Rust tests assert against those
fixtures.** Core CI never runs Python, `maturin` or WASM; the live-Python jobs
are separate and non-blocking.

### T0. Oracle extraction (one-time per re-vendor)

`tools/extract_oracle.py`, run with `uv` inside `vendor/morphnet`:

1. **Collect inputs** from upstream (`specs/parity.md` R2.1):
   - committed netlists `tests/netlists/{spectre,hspice,vams}/*`;
   - expected-IR YAML `tests/netlists/*.yaml`;
   - inline netlists in `tests/test_*.py`, extracted by **AST walk** (find string
     constants passed to `parse_*`/`write_*`, or assigned and then used), keyed by
     test name;
   - `examples/*.py` builder scripts.
2. **Run the oracle** (upstream Python) per case and record, as canonical output:
   - preprocess result (cleaned text, directives, analog blocks);
   - parsed IR as **deterministic protobuf bytes** (`to_proto_bytes`) and as
     canonical JSON/YAML;
   - writer output for every format from each IR;
   - round-trip results (`write(parse(text))`, stability, `parse(write(ir))`);
   - for malformed cases, the exception kind and position.
3. **Emit** `tests/corpus/**` fixtures plus `tests/corpus/manifest.json` with
   provenance: upstream commit SHA, source path, test name, and generator version.

Re-run only on re-vendor or when extending the corpus; the freestanding script
records the upstream commit it read so fixtures are auditable.

### T1. Golden-fixture tiers (required, offline)

Mirroring `specs/parity.md` R1, each tier is a Rust test that reads committed
fixtures:

| Tier | Asserts | Comparison |
|---|---|---|
| L0 helpers | `value_utils`, `net_utils`, `simulation_data` (R1.5) | exact value equality |
| L1 preprocess | `preprocess_*` (R1.4) | string/record equality |
| L2 parse | text → IR (R1.1) | decoded-message equality on canonical protobuf |
| L3 write | IR → text (R1.2) | **byte-exact** vs golden netlists; canonical otherwise |
| L4 round-trip | text → IR → text (R1.3) | stability + oracle match |
| L5 builder | `_api` construction → IR (R1.6) | message equality |
| L6 interchange | proto encode/decode + JSON/YAML (R1.7) | message- and shape-equal |
| L7 errors | malformed input (R1.8) | error kind + position |

L0/L1 land first: they are pure, cheap and de-risk the schema and engine before
any transformer exists.

### T2. Differential harness (non-blocking)

A CI job that builds the Rust side and runs the vendored Python live on the same
inputs, diffing outputs directly (parity R3.2):

- replays the whole corpus, so fixture drift is caught;
- **generates** inputs the corpus lacks — a netlist generator plus mutation of
  corpus inputs — and minimizes any divergence to a small repro;
- reports deltas against the R4 allow-list; any new delta is a bug (ours, or a
  new upstream issue to log).

A minimizer output is the preferred form for a new corpus case or an R4 row.

### T3. Upstream test replay (non-blocking, stretch)

Run upstream's own `pytest` suite against `oxymorph-python` through a thin
`morphnet`-shaped shim, turning upstream's assertions into an oracle we did not
write (parity R3.4). Tally pass/xfail into the coverage ledger. Optional because
it depends on upstream's private API shape and the not-API-compatible decision.

### T4. Coverage ledger

`tools/extract_oracle.py` also emits the R5 table in `specs/parity.md`, mapping
every upstream test function to a corpus case and a Rust test, so uncovered
behavior is visible rather than assumed absent.

### CI gating

- **Required for merge:** `cargo fmt --check`, `clippy -D warnings`,
  `cargo test --workspace` (T1 tiers against committed fixtures), the crate
  boundary tests, and the per-format feature builds.
- **Non-blocking:** T2 differential harness, T3 upstream replay, fuzzing.

## Phase 0 — Foundations

- [x] Workspace skeleton per `decisions/0003` (crate-only, no root `src/`).
- [x] `oxymorph-schema`: IR types hand-written with serde (`decisions/0006`);
      prost-first step dropped. Was:
      alias maps, oneof/union validation, JSON/YAML shape (`decisions/0005`).
- [x] Parser spike (`decisions/0002`) — **done, falsified the original decision.**
      Upstream mixes engines (LALR for spectre/vams, Earley+resolve for
      spice/hspice/xyce); `include_lark!`/standalone cover LALR + basic lexer
      only and mis-lex `spectre`; lark-rs **in-process** with per-format config
      matches Python 21/21. `decisions/0002` revised to lark-rs in-process and
      **decided**.
- [x] `tools/extract_oracle.py` (T0) and the `tests/corpus/` fixtures (357 cases).
- [x] T1 harness for L2/L3 (parse/write) wired into `cargo test` (`tests/parity.rs`).
- [x] Port the per-format preprocessing (`preprocess_*.py`: comment stripping,
      continuations, semicolon injection, vams directive/analog extraction).
- [x] Error model and the parser boundary (`specs/parsing.md`), mapping `lark_rs`
      errors to our structured type.

## Phase 1 — First vertical slice

- [x] `spectre` parse end-to-end to IR.
- [x] `vams` write (reproduces the README quickstart) and its round-trip
      (T1 L3/L4).
- [x] Live binding check (`tools/generate_equivalence_docs.py`).

## Phase 2 — SPICE family

Base `spice` parse/write first, then the derived formats (broadest coverage per
unit effort):

- [x] `spice` parse + write.
- [x] `hspice` (derived).
- [x] `xyce` (derived; adds `PARAMS:` and `{expr}`).
- [x] `spectre-spice` (derived).

## Phase 3 — Remaining formats

- [x] `vams` parse (longest transformer).
- [x] Remaining writers for every format.
- [x] `spectre` write.
- [x] Builder API (`_api`) parity. Interchange parity (protobuf) deferred.

## Phase 4 — Bindings and targets

- [x] `oxymorph-python` (PyO3, module `oxymorph`, same functionality as
      `morphnet`, not API-compatible).
- [ ] `oxymorph-cli` (if wanted).
- [x] `oxymorph-wasm` (wasm-bindgen, `wasm32-unknown-unknown`) and the docs
      playground (`decisions/0009`).
- [x] Live binding check against upstream (357 parse / 2142 write).

## Phase 5 — Migration / divergence

- [x] Hand-written idiomatic IR (`decisions/0006`); no prost migration needed.
- [x] Protobuf interchange (`decisions/0010`), message-equal on the corpus.
- [ ] Consider per-format crates (`decisions/0003` option 3) if needed.
- [ ] Begin deliberate divergence: batch-fix logged upstream issues
      (`specs/parity.md` R4) under a divergence ADR.

## Open items

- `specs/ir.md` — JSON/YAML shape, union/presence semantics.
- `specs/parsing.md` — parser trait, preprocessing, error mapping.
- `specs/architecture.md` — crate topology in prose.
- `specs/parity.md` Q1–Q3 (byte-exact writers, error-kind mapping, wire
  byte-exactness).
