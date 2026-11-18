# 0001. Rust port of the vendored morphnet library

- Status: implemented
- Date: 2026-02-10
- Supersedes: -
- Superseded by: -
- Related decisions: `0002-parser-engine.md`, `0003-workspace-layout.md`,
  `0004-parity-policy.md`, `0005-ir-and-protobuf.md`
- Related specs: `specs/parity.md`

## Context

`vendor/morphnet` is a Python library that bidirectionally parses SPICE-family
netlists (HSPICE, Xyce, Spectre, Spectre-SPICE) and Verilog-AMS through a shared
Protocol Buffers intermediate representation. oxymorph is a Rust port of it.

Motivation, in priority order:

1. **Embedding** — use netlist parsing inside Rust binaries/services with no
   Python runtime in the loop.
2. **Deployment** — a single self-contained artifact (static binary, WASM) for
   CLI, browser, and edge targets.
3. **Robustness/typing** — encode the IR and format rules in the type system
   instead of Python/Pydantic runtime models, so whole classes of malformed
   states cannot be constructed.

Performance is a welcome side effect, not the driver. Confidence in the current
morphnet implementation quality is low, so a stricter language is preferred for
a problem domain (parser + IR + writers) where correctness matters.

Relationship to upstream: **coexist and track**. `das-dias/morphnet` is a
personal project of one GDSFactory employee and is only partially under our
control; Python stays canonical and the Rust port tracks it, initially aiming for
an exact match. Bugs discovered while porting are recorded, not silently fixed
(see `0004-parity-policy.md`), so they can be addressed once we deliberately
diverge.

Constraints:

- Crate-only repository; no root `src/`.
- A separate thin PyO3 crate over the pure-Rust crate(s).
- Observable behavior must be **functionally equivalent** to Python morphnet:
  IR equality after canonicalization, writer output equal after canonicalization,
  and protobuf interchange ideally shared.

Non-goals:

- Reimplementing or improving upstream's behavior while the port is tracking it
  (deferred to deliberate divergence; see `0004`).
- Byte-exact reproduction of Python formatting quirks (float repr, dict
  insertion order) as a hard requirement.
- An API-compatible Python drop-in for `morphnet`; the PyO3 crate exposes the
  same functionality through its own API.

## Decision

Port the vendored morphnet library to Rust, as a tracking reimplementation that
coexists with upstream and matches it functionally before diverging.

The port is a crate-only Cargo workspace under `crates/` (`0003`), embeds the
upstream `.lark` grammars verbatim via lark-rs's compile-time `include_lark!`
(`0002`), models the IR per `0005`, and treats bug-for-bug parity as the initial
invariant (`0004`), verified by a golden corpus plus a non-blocking differential
harness (`specs/parity.md`).

## Options considered

- **Do nothing / keep using Python** — rejected: fails the embedding and
  deployment motivations outright.
- **Port to Rust (chosen)** — meets all three motivations; cost is a full
  reimplementation plus a maintained parity harness.
- **Thin FFI wrapper around the Python library** (embed CPython, or PyO3 in the
  reverse direction) — rejected: ships a Python runtime, contradicting embedding
  and single-artifact deployment.
- **Generate a Rust parser from the protobuf/gammar artifacts** — rejected: no
  such path exists that preserves upstream semantics (`0002`).
- **Replace upstream rather than track it** — rejected for now: upstream is only
  partially under our control; tracking keeps the option to diverge later open.

## Consequences

Easier: embedding, WASM/static deployment, compile-time-typed IR, and a
correctness story enforced by the compiler plus the parity harness.

Harder / owed:

- A maintained parity harness, a golden corpus extracted per re-vendor, and a
  live upstream-issue log (`specs/parity.md`).
- A grammar/parser dependency that is pre-1.0 and unpublished (`0002`), with a
  documented vendoring/fallback path.
- Periodic re-vendoring and re-porting as upstream moves.
- Intentional bug replication in places, each tagged with a `// parity: <id>`.

## Verification

- Every format/direction pair meets R1 in `specs/parity.md` against the committed
  corpus, with the differential harness reporting no unexplained deltas.
- The workspace builds crate-only, with the dependency direction asserted in CI
  (`0003`).
- The parser spike in `0002` passes its acceptance gate.
