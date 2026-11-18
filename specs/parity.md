# Parity

Living spec for functional equivalence between oxymorph and the vendored Python
morphnet. Authoritative for what "functionally the same" means, how it is
verified, and which upstream defects we knowingly reproduce.

Related decisions: `decisions/0001` (port, exact match), `decisions/0002`
(parser engine), `decisions/0004` (bug-for-bug parity policy).

## Scope

Parity is defined per **(format, direction)** pair, not globally:

| Axis | Values |
|---|---|
| Format | spice, hspice, xyce, spectre, spectre-spice, vams |
| Direction | parse (text → IR), write (IR → text) |
| Interchange | protobuf encode/decode of the IR |

A pair counts as "ported" only when its parity requirements below hold.

## R1. Definition of equivalence

- **R1.1 Parse parity** — for a given input, the Rust IR and the Python IR are
  equal after canonicalization. Canonical form is the protobuf encoding of the
  IR with deterministic field ordering; comparison is on the decoded message,
  not on source text.
- **R1.2 Write parity** — for a given IR, Python's writer output and Rust's
  writer output are equal after canonicalization (whitespace/line-ending
  normalization and numeric formatting reconciliation), unless a stricter
  requirement is recorded for that format.
- **R1.3 Round-trip parity** — parse-then-write and write-then-parse agree with
  the Python round-trip for the same corpus, under R1.1/R1.2.
- **R1.4 Error parity** — where Python raises for malformed input, Rust returns
  an error of the equivalent kind at the equivalent position. Exact message text
  is **not** required. (Open: which error kinds must correspond; see below.)

## R2. Corpus

- Inputs and expected outputs are extracted from upstream's own tests and
  `examples/` and committed as fixtures, with provenance recorded (upstream
  commit/path).
- The corpus is **the** definition of covered behavior. An input not in the
  corpus is untested, not implicitly conformant.
- Corpus extraction is a one-time step per re-vendor and is checked in, so tests
  run offline with no Python toolchain.

## R3. Verification mechanisms

- **R3.1 Golden fixtures** — required for merge. Rust tests assert R1 against
  committed fixtures.
- **R3.2 Differential harness** — non-blocking CI job that runs the vendored
  Python (uv) and Rust on the same inputs and diffs them; catches drift the
  corpus misses.
- **R3.3 Known-issue allow-list** — the harness may xfail entries recorded
  below. An entry without an enabling ADR is a review blocker.

## R4. Known upstream issues

Policy: reproduce upstream behavior exactly, including bugs
(`decisions/0004`).

| id | upstream | symptom | our behavior | repro | status | ADR |
|---|---|---|---|---|---|---|
| _none yet_ | | | | | | |

Status values: `open` (reproduced, awaiting upstream fix), `fixed-upstream`
(upstream fixed it; port tracks the fix), `diverged` (we intentionally differ;
ADR required).

## Invariants

- No format is "ported" while R1 is unverified for it.
- Every intentional deviation in Rust carries `// parity: <id>` resolving to a
  row in R4.

## Open questions

- **Q1** — Is byte-exact writer output required for any consumer, or is R1.2
  canonicalization sufficient everywhere? (Affects numeric formatting work.)
- **Q2** — Which Python error kinds must map to which Rust error kinds for R1.4,
  and do parse errors need position parity?
- **Q3** — Does protobuf interchange parity (R1.1) need to be byte-exact on the
  wire, or only message-equal after decode?
