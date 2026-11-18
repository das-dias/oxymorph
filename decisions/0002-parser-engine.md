# 0002. Parser engine and lexing

- Status: proposed
- Date: 2026-02-10
- Supersedes: -
- Superseded by: -
- Related specs: `specs/parsing.md` (TBD)
- Related decisions: `0001-rust-port-of-morphnet.md`, `0003-workspace-layout.md`

## Context

Upstream morphnet parses with Python Lark using **five grammars** (not six:
`common.lark` is unused, and `spectre_spice` has no grammar — it splits input on
`simulator lang=` and delegates to the spice and spectre parsers). Each format
uses a *different* engine configuration:

| format | `Lark(...)` configuration |
|---|---|
| `spectre` | `parser="lalr"` (contextual lexer) |
| `vams` | `parser="lalr"` (contextual lexer) |
| `spice`, `hspice`, `xyce` | `parser="earley", ambiguity="resolve"` (dynamic lexer) |

The SPICE-family grammars are **genuinely LALR-ambiguous**: they produce
reduce/reduce conflicts in the EBNF lowering of `start`/`_NL`, reproduced
identically in Python Lark 1.3.1 (`GrammarError: Reduce/Reduce collision …`).
Upstream therefore falls back to Earley for those three.

Parsed trees are consumed by `lark.Transformer` subclasses (~128 rule methods:
spice 49, vams 48, spectre 24, hspice 3, xyce 4) that build the Pydantic IR.
Decision 0001 requires functional equivalence, embedding, and single-artifact
deployment.

Decision 0003 requires the parser to sit behind our own trait, with generated
tree/error types not leaking into the IR.

### Grammar-parity spike (evidence)

Baked all five grammars with lark-rs's compile-time `include_lark!` and the
in-process engine, and diffed trees against Python Lark over upstream's 12
committed netlists plus 5 inline samples:

- `include_lark!` / `generate-parser` (standalone: **LALR + basic lexer only**):
  - `spectre`, `hspice`, `spice`, `xyce` fail to build — the three SPICE-family
    grammars have unresolvable LALR conflicts, and `spectre` builds but **mis-lexes**
    (`rc_lowpass` read as `FILE_PATH` instead of `IDENT`; `include_lark!` is
    LALR-only and its baked parser uses the basic lexer).
  - Only `vams` bakes and parses correctly.
- lark-rs **in-process**, configured to mirror upstream per format:
  - `spectre` LALR + **contextual**: 4/4 netlists identical to Python.
  - `vams` LALR + contextual and LALR + basic: 4/4 identical.
  - `hspice` Earley + `ambiguity="resolve"`: 4/4 identical.
  - `spice` (2) and `xyce` (3): 5/5 identical.
  - Total **21/21 identical**; the only mismatches were `include_lark!`'s basic
    lexer on `spectre`.

`lark-rs` = `okalldal/lark`, `lark-rs/`, MIT, currently `0.1.0`, unpublished,
consumed via git.

## Decision

**Use `lark-rs` in-process** as the parsing engine, keeping the upstream `.lark`
files as the authoritative grammar source (used verbatim), and configure it **per
format to mirror upstream exactly**:

- `spectre`: `ParserAlgorithm::Lalr` + `LexerType::Contextual`
- `vams`: `ParserAlgorithm::Lalr` + `LexerType::Contextual`
- `spice`, `hspice`, `xyce`: `ParserAlgorithm::Earley` + `Ambiguity::Resolve`
  (+ the auto/dynamic lexer)

Concretely:

- The engine is hidden behind **our own parser trait**; `lark_rs` tree/error types
  are mapped to our own at that boundary and never leak into `oxymorph-netlist`'s
  public surface or the IR.
- `lark_rs` is a pinned git dependency (`Cargo.lock`); vendoring is the fallback
  if reproducibility or fork availability becomes a problem.
- Compile-time baking (`include_lark!`) and standalone generation
  (`generate-parser`) are **not** used, because they cover LALR + basic lexer
  only. They remain a **future de-risking option** if lark-rs gains
  contextual/Earley standalone support.
- LALRPOP remains the documented fallback if `lark-rs` proves problematic; it
  would require a manual parser + transformer rewrite (see below).

## Options considered

- **(a) `lark_rs` in-process, per-format config (chosen)** — evidenced to match
  Python on all sampled inputs; the only option that covers the three ambiguous
  Earley grammars without rewriting them. Cost: a pre-1.0 engine ships in the
  artifact.
- **(b) `generate-parser` standalone, vendored** — no lark-rs dependency, but
  **LALR + basic lexer only**: cannot build the SPICE-family grammars and
  mis-lexes `spectre`. Rejected on evidence.
- **(c) `include_lark!` proc-macro** — compile-time baked parsers, but the same
  LALR + basic-lexer restriction as (b). Rejected on evidence. (This was the
  original decision; the spike falsified it.)
- **(d) LALRPOP** — mature and fully ours, but the ambiguous SPICE-family
  grammars would need rewriting or an Earley engine, and the ~128 transformer
  methods are rewritten against a typed AST. Documented fallback only.
- **(e) Hybrid: LALRPOP for spectre/vams + lark-rs Earley for the rest** —
  possible, but still ships lark-rs for three formats while adding a second
  parser stack. Not worth it.

## Consequences

Easier: exact reproduction of upstream's per-format engine behavior; verbatim
grammars; Earley ambiguity handled by the engine rather than by us.

Harder / owed:

- A pre-1.0, unpublished engine ships at runtime behind our trait; API churn and
  correctness are external risks mitigated by the pin, the boundary, and the
  parity harness.
- We must replicate upstream's per-format engine/lexer configuration; changing it
  is a divergence and needs an ADR (it changes trees).
- `lark_rs`'s Earley/SPPF ambiguity resolution must match Python's
  `ambiguity="resolve"` on ambiguous inputs; the corpus sweep covers this only
  partially and the differential harness is the safety net.
- The `// parity: <id>` mechanism (decision 0004) applies to any intentional
  engine-behavior divergence.
- Still no compile-time grammar validation (lost from the `include_lark!` path);
  malformed grammars fail at parser construction, which we surface as a structured
  error and cover with a test.

## Verification

- **Spike (done):** 21/21 tree-identical to Python Lark across the 12 committed
  netlists and 5 inline samples, with per-format config above.
- **Ongoing:** `specs/parity.md` R1 per format against the golden corpus, plus the
  differential harness with no unexplained deltas.
- **Boundary:** a test asserts `lark_rs` types do not appear in
  `oxymorph-netlist`'s public API.
- Re-run the spike on every re-vendor or `lark-rs` pin bump.

Acceptance: this decision moves to **decided** once the user accepts shipping the
pinned `lark-rs` engine in-process, and to **implemented** once the parser trait
and first format are wired and covered by a spec.
