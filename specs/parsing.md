# Parsing

Implemented parser behavior (`decisions/0002`, `decisions/0004`).

## R1. Engines

Each format embeds the verbatim upstream `.lark` grammar and mirrors upstream's
engine configuration exactly:

| Format | Grammar | Engine | Lexer |
|---|---|---|---|
| `spectre` | `spectre.lark` | LALR | contextual |
| `vams` | `vams.lark` | LALR | contextual |
| `spice` | `spice.lark` | Earley, `ambiguity="resolve"` | auto/dynamic |
| `hspice` | `hspice.lark` | Earley, `ambiguity="resolve"` | auto/dynamic |
| `xyce` | `xyce.lark` | Earley, `ambiguity="resolve"` | auto/dynamic |

`spectre_spice` has no grammar: it splits input on `simulator lang=` and
delegates to the spice and spectre parsers (`preprocess_spectre_spice`).

## R2. Preprocessing

Per-format preprocessing runs before parsing and mirrors `*/preprocess.py`:
comment stripping, continuation joining, and (spectre) semicolon injection;
vams extracts backtick directives and replaces `analog` blocks with numbered
placeholders.

## R3. Boundary

`lark_rs` types never appear in the public API. `parse_*` returns
`Result<oxymorph_schema::Circuit, oxymorph_netlist::Error>`; `lark_rs` parse
errors are mapped to `Error::Parse { message, line, column }`.

## R4. Coverage

Every format parses at least the upstream `tests/netlists` corpus. Loss of
parity on any corpus case is a bug (see `specs/parity.md`).
