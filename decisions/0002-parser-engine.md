# 0002. Parser engine and lexing

- Status: proposed (acceptance gated on the grammar-parity spike in *Verification*)
- Date: 2026-02-10
- Supersedes: -
- Superseded by: -
- Related specs: `specs/parsing.md` (TBD)
- Related decisions: `decisions/0001-rust-port-of-morphnet.md`

## Context

Upstream morphnet parses with Python Lark, `Lark(grammar_text, parser="lalr")`,
using six LALR grammars under `src/morphnet/netlist/grammars/` (~692 lines
including a shared `common.lark`) plus a per-format `preprocess.py`
(comment stripping, line-continuation joining, semicolon injection). Parsed
trees are consumed by `lark.Transformer` subclasses (~128 rule methods across
spice 49, vams 48, spectre 24, hspice 3, xyce 4) that build the Pydantic IR.

Decision 0001 requires functional equivalence with the Python implementation,
and requires the result to be embeddable (no Python runtime) and deployable as a
single artifact (including WASM).

### Grammar feature inventory (evidence)

The grammars use a plain Lark subset:

- Only `%ignore` appears as a directive — no `%import`, `%override`, `%extend`,
  `%declare`, and no parameterized templates.
- Exactly two terminal priorities: `PORT_DIR.2`, `PARAM_TYPE_KW.2` (vams).
- Regex terminals per format: `SI_NUMBER` (with SI suffix), `IDENT`,
  `NET_NAME`, `QSTRING`, `FILE_PATH`, `BRACE_EXPR` (xyce), `OUTPUT_VAR`.
- `%ignore` is format-specific: `[ \t]+` (spice/hspice/xyce, where `_NL: NL`
  makes newlines significant) vs `[ \t\r\n]+` (spectre/vams).
- Pervasive tree-shaping modifiers: `?rule` (inline if single child) and
  `_rule` (always inline), e.g. `?statement`, `?param_value`, `?toplevel_item`,
  `_SUBCKT_KW`.

### Why not translate to LALRPOP

There is no Lark→LALRPOP converter, and the two differ exactly where the port
costs most:

- **Lexing model.** Lark uses a separate, contextual lexer over prioritized
  regex terminals with `%ignore`; LALRPOP's built-in lexer is a non-contextual
  DFA over literal tokens, with regex only via `match`/`extern`. Our regex
  terminals and per-format ignore policy mean a hand-written lexer per format.
- **Tree shaping.** LALRPOP emits *typed ASTs*; it has no tree/inlining layer.
  The `?rule`/`_rule` semantics are load-bearing: the transformers consume
  `items: list[Any]` whose shape those modifiers define. Porting means rewriting
  all ~128 methods as AST→IR matches, with no mechanical path and no structural
  guarantee of remaining equivalent to upstream.

Only the surface BNF (EBNF `?`/`*`/`+`, `-> alias`, literal keyword terminals)
translates mechanically; the lexing and tree-shaping halves do not. Under the
criterion "cheap/automated → LALRPOP, manual → a Lark-compatible Rust engine",
this is manual.

## Decision

**Use `lark-rs`** (`okalldal/lark`, `lark-rs/`, MIT, currently `0.1.0`,
unpublished, consumed via git) as the parsing engine, keeping the upstream
`.lark` files as the authoritative grammar source (used verbatim), and consume
it via its **compile-time `include_lark!` proc-macro** (`lark_proc`) so no
parser engine ships at runtime.

Concretely:

- Each of the six grammars is embedded with `include_lark!("grammars/<fmt>.lark")`,
  producing a typed parser per format. No `ParseTable` construction at runtime.
- The generated parser is wrapped behind **our own parser trait**, and its
  `String` errors are mapped to our structured `ParseError` at that boundary.
- `lark_rs` is kept as a **`[dev-dependencies]`** entry (not shipped) to A/B the
  same grammars basic-vs-contextual and to generate oracle trees in tests.
- `lark_proc` is a git dependency pinned by `Cargo.lock`; vendoring is the
  fallback if reproducibility or fork availability becomes a problem.

## Options considered

- **(a) `lark_rs` in-process at runtime** — full engine (contextual lexer,
  Earley, postlex, lookaround), fastest to a near-1:1 transformer port. Rejected
  as the *shipping* model because it puts a pre-1.0 engine in the artifact, but
  retained as a dev-dependency for debugging and oracle work.
- **(b) `generate-parser` CLI, vendored output** — emits a self-contained Rust
  LALR parser (`regex` + std only). No lark-rs dependency at all, but makes
  regeneration a manual step. Viable later; kept as an escape hatch.
- **(c) `include_lark!` proc-macro** — compile-time validation and baked tables,
  self-contained output. **Chosen.**
- **(d) LALRPOP** — mature and fully ours, but requires a manual parser + AST +
  transformer rewrite with drift risk against upstream. Rejected for now;
  documented fallback if `lark-rs` proves problematic.

## Consequences

Easier: verbatim grammars, a near-1:1 transformer port, compile-time grammar
errors, no runtime engine dependency, WASM/PyO3 friendliness.

Harder / owed:

- We accept **LALR + basic lexer only** for the shipped parsers; no postlex and
  no lookaround grammars. If a grammar requires the contextual lexer, this
  decision must be revisited (fall back to (a) for that format, or to (b)/(d)).
- Parse errors arrive as `String`; we owe a structured error type at the wrapper.
- `lark_proc` is a git dependency; we owe lockfile discipline and a vendoring
  plan.
- Transformers must be written against the generated per-struct `Tree`/`Token`/
  `Child` types; freezing that surface behind our trait prevents leakage.
- A grammar/lexer divergence from upstream is possible and must be caught by the
  parity harness, not by inspection.

## Verification

Acceptance is gated on a **grammar-parity spike**:

1. Embed all six `.lark` files via `include_lark!`; each must compile (a bad
   grammar fails the build by construction).
2. Run the golden corpus (inputs + expected outputs extracted from the vendored
   Python tests/`examples/`) through the baked parsers.
3. Diff resulting trees against Python Lark's trees (via the `lark_rs`
   dev-dependency and/or captured fixtures). Specifically probe the
   keyword-vs-`IDENT` collision shape (`_SUBCKT_KW: "subckt"` vs `IDENT`) and the
   per-format `%ignore`/`_NL` cases.
4. Any grammar that does not reproduce Python Lark under the **basic** lexer
   fails the spike and forces a documented change to this decision.

If the spike passes, status moves to Accepted.
