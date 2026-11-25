# Parity

Living spec for functional equivalence between oxymorph and the vendored Python
morphnet. Authoritative for what "functionally the same" means, how it is
verified, and which upstream defects we knowingly reproduce.

Related decisions: `decisions/0001` (port, exact match), `decisions/0002`
(parser engine), `decisions/0004` (bug-for-bug parity policy), `decisions/0011`
(divergence for ported kfnetlist features).
The plan for reaching this contract lives in `ROADMAP.md` §Testing plan.

## Scope

Parity is defined per **surface**, not globally. A surface (and, for
parse/write, each format) counts as "ported" only when its requirements hold.

| Surface | Input → output |
|---|---|
| preprocess | raw text → cleaned text / preprocess result |
| parse | text → IR |
| write | IR → text |
| round-trip | text → IR → text (and IR → text → IR) |
| builder (`_api`) | programmatic construction → IR |
| helpers | value/net/simulation helpers (`value_utils`, `net_utils`, `simulation_data`) |
| interchange | IR ↔ protobuf bytes; IR ↔ JSON/YAML |
| errors | malformed input → error kind + position |

Formats: `spice`, `hspice`, `xyce`, `spectre`, `spectre-spice`, `vams`
(`spectre_spice` reuses the spice and spectre parsers/writers).

## R1. Definition of equivalence

- **R1.1 Parse parity** — for a given input, the Rust IR and the Python IR are
  **message-equal** after canonicalization. Canonical form is the deterministic
  protobuf encoding of the IR; comparison is on the decoded message, not on
  source text. (Wire bytes need not be identical; see Q3.)
- **R1.2 Write parity** — for a given IR, the Rust writer output equals the
  Python writer output. Upstream's own tests assert writer output **byte-equal**
  to committed golden netlists, so the corpus bar is byte-exact, not merely
  canonicalized. Where no golden exists, compare canonicalized text (whitespace
  and line-ending normalization only).
- **R1.3 Round-trip parity** — for the corpus: `write(parse(text))` is stable
  (`write(parse(text)) == write(parse(write(parse(text))))`) and matches the
  Python round-trip under R1.1/R1.2.
- **R1.4 Preprocess parity** — for a given raw input, the Rust preprocess result
  equals Python's (string-equal cleaned text, and equal directive/analog-block
  lists where the Python result is a record).
- **R1.5 Helper parity** — every public helper in `value_utils`, `net_utils` and
  `simulation_data` returns the same value as Python for the same input
  (including SI-value parsing/formatting and net maps).
- **R1.6 Builder parity** — the programmatic `_api` (`circuit`, `module`,
  `port`, `connection`, `parameter`, `module_reference`, `ext_module`,
  `external_library`, `model_interface`) produces the same IR as Python for the
  same calls, including defaults and validation.
- **R1.7 Interchange parity** — JSON/YAML dumps are shape-equal to Python's for
  the corpus, and alias/oneof semantics are reproduced. Protobuf: decoding
  upstream's `to_proto_bytes()` reproduces our IR message-equal for all corpus
  cases (`decisions/0010`). Wire byte-exactness is not required (map ordering
  differs).
- **R1.8 Error parity** — where Python raises for malformed input, Rust returns
  an error of the equivalent kind at the equivalent position. Exact message text
  is **not** required. (Open: which error kinds must correspond; see Q2.)

## R2. Corpus

- **R2.1 Sources** (provenance recorded per fixture — upstream commit + path +
  test name):
  - `tests/netlists/{spectre,hspice,vams}/*.{scs,sp,vams}` — netlist inputs and
    **writer goldens** (upstream asserts byte-equality against these).
  - `tests/netlists/*.yaml` — expected IR as Pydantic YAML
    (`rc_lowpass`, `resistor_divider`, `hierarchical_subckt`,
    `inverter_with_models`).
  - Inline netlists in `tests/test_*_parser.py`, `test_*_writer.py`,
    `test_*_roundtrip.py`, `test_*_simulation.py`, `test_value_utils.py`,
    `test_circuit_models.py` (extracted by AST walk).
  - `examples/inverter.py`, `examples/mzi.py` — builder-API scripts.
  - kfnetlist ports (`decisions/0011`) — the second oracle: tests and fixtures
    ported from `kfnetlist.morphnet` (kfnetlist `e596bb5` plus `f020af6`) pin the
    features added beyond the vendored morphnet. Ported fixtures live under
    `crates/oxymorph-netlist/tests/data/kf/**`, with names mapped to oxymorph's per
    `decisions/0011` (YAML/JSON keys only; netlist text is copied byte-for-byte).
  - `tests/corpus/overrides.json` — replaces the expected `json` / `writes[fmt]`
    of named original-corpus cases that a ported fix changes; every entry names
    an R4 row.
- **R2.2 Extraction** — a one-time step per re-vendor (`tools/extract_oracle.py`),
  run with `uv` against the vendored Python, emitting committed fixtures under
  `tests/corpus/` and a provenance manifest. Core tests then run offline with no
  Python.
- **R2.3 Coverage** — the corpus is **the** definition of covered behavior. An
  input not in the corpus is untested, not implicitly conformant. Coverage is
  tracked in R5.

## R3. Verification mechanisms

- **R3.1 Golden fixtures** — required for merge. Rust tests assert R1 against the
  committed corpus.
- **R3.2 Differential harness** — non-blocking CI job that runs the vendored
  Python (`uv`) and Rust on the same inputs and diffs them; catches drift the
  corpus misses. Covers randomized/fuzzed inputs, not just fixed cases.
- **R3.3 Known-issue allow-list** — both mechanisms may xfail entries recorded in
  R4. An entry without an enabling ADR is a review blocker.
- **R3.4 Upstream test replay** — non-blocking, stretch: run upstream's own
  `pytest` suite against `oxymorph-python` through a thin `morphnet`-shaped shim,
  converting upstream's assertions into an independent oracle we did not write.
  Optional because it depends on upstream's private API shape and the
  not-API-compatible decision (`decisions/0001`).

## R4. Known upstream issues

Policy: reproduce upstream behavior exactly, including bugs
(`decisions/0004`). The exception is a ported kfnetlist fix that changes an
original-corpus expectation (`decisions/0011`): it is a `diverged` row, its
cases are listed in `tests/corpus/overrides.json`, and the code carrying it is
marked `// parity: <id>`. Such ids are `kf-<short-slug>`. An override's `json` and `writes[fmt]` replace the
corpus values; when `json` is overridden the oracle protobuf (which encodes the old
IR) is not compared for that case, while the protobuf round-trip of the parsed IR
still is.
`overrides_reference_r4_rows_and_name_existing_cases` fails if an override names
an id that is not in this table or a case that is not in the corpus.

| id | upstream | symptom | our behavior | repro | status | ADR |
|---|---|---|---|---|---|---|
| kf-signal-domains | kfnetlist 1deb1e0 (#34) | every connection has domain `UNSPECIFIED` | a connection takes its net's domain: `WAVEGUIDE` when the net touches an `o<N>` terminal, else `ELECTRICAL` (`helpers::domain_of`); ports named `o<N>` are `WAVEGUIDE` | 190 cases in `tests/corpus/overrides.json` (IR only) | diverged | 0011 |
| kf-device-pin-template | kfnetlist 1deb1e0 (#34), 0a5184b (#36) | a built-in device with no external-module declaration is written `M1 ? ? ...` (SPICE) or in inferred pin order (Spectre) | SPICE writes the device pin template order (`d g s b` for `M`); Spectre does too when the instance has every template pin, else infers the order | `vams/netlist/vams/inverter_with_models.vams` (writes; its IR also carries kf-signal-domains) | diverged | 0011 |

No upstream defect required reproduction: the port matched upstream on all 357
corpus cases and all 2142 writer outputs before the kfnetlist divergences above.

Status values: `open` (reproduced, awaiting upstream fix), `fixed-upstream`
(upstream fixed it; port tracks the fix), `diverged` (we intentionally differ;
ADR required).

## R5. Coverage ledger

Maps upstream's test surface to Rust corpus cases and flags uncovered behavior.
Rows are added during extraction and updated as the port progresses.

| format | parse cases | writer outputs | status |
|---|---|---|---|---|
| spice | 67 | 402 | all identical |
| hspice | 78 | 468 | all identical |
| xyce | 70 | 420 | all identical |
| spectre | 27 | 162 | all identical |
| spectre_spice | 74 | 444 | all identical |
| vams | 41 | 246 | all identical |
| **total** | **357** | **2142** | **100% identical** |

Protobuf interchange (message-equal after decode): **357 / 357**.

Since the kfnetlist divergences in R4, 191 of these cases compare against
`tests/corpus/overrides.json` instead; their oracle protobuf is not compared.

The writer count is each parsed circuit written back in every format. Generated by
`tools/extract_oracle.py`; the live binding check
(`tools/generate_equivalence_docs.py`) reports the same tally.

## Invariants

- No surface/format is "ported" while its R1 requirements are unverified.
- Every intentional deviation in Rust carries `// parity: <id>` resolving to a
  row in R4.
- No corpus case may be deleted to make a test pass; it is removed only with a
  recorded reason.

## Open questions

- **Q1** — Is byte-exact writer output required everywhere, or only where a
  committed golden exists (R1.2)? Affects how much numeric-formatting work is
  owed.
- **Q2** — Which Python error kinds must map to which Rust error kinds for R1.8,
  and do parse errors need position parity?
- **Q3** — Resolved for now: message-equality after decode is the bar; wire
  byte-exactness differs only in map field ordering.
