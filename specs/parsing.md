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

## R5. Instances and signal domains

Ported from kfnetlist #34 and #36 (`decisions/0011`):

- Every parser marks ports and pins named `o<N>` (`o1`, `o12`, ...) as
  `WAVEGUIDE` and the rest as `ELECTRICAL` (`helpers::domain_of`). A connection
  takes its net's domain: `WAVEGUIDE` when the net touches an `o<N>` terminal,
  else `ELECTRICAL`.
- Spectre: the master decides the device, not the instance-name letter. A
  defined subckt makes an instance with the subckt's port names; a built-in
  master (`resistor`, `capacitor`, ...) makes that device; the name letter is
  the fallback. A built-in master with more or fewer nets than its pin template
  keeps every net positionally (`p0`, `p1`, ...). A subckt named like an
  analysis (`dc`) and defined earlier in the deck makes an instance, not an
  analysis.
- Spectre-SPICE: an instance of a subckt defined in another language section
  gets the definition's port names instead of positional `p<N>` pins.
- Writers: a built-in device with no external-module declaration is written in
  its pin-template order (`d g s b` for `M`); Spectre does so only when the
  instance has every template pin, else it infers the order from the
  connections.

## R6. Parameter expressions

Ported from kfnetlist d0daf3e, 1adb071, c12a71e, d94dc38 (`decisions/0011`);
`crates/oxymorph-netlist/src/expression.rs`:

- `parse_expression(text)` reads any dialect's expression text with one shared
  grammar (`grammars/expression.lark`, LALR): SI-suffixed numbers, references,
  unary/binary operators, comparisons, logic, calls and `?:`. One outer `{…}` or
  `'…'` is stripped first; text that does not parse is kept as `raw`. Text that
  could nest deeper than 128 levels (each parenthesised group and each operator
  character counts) is rejected with `Error::Validation`; the parsers keep such a
  value verbatim as `raw` instead.
- `format_expression(expr, dialect, resolve, named)` prints canonical infix text
  with minimal parentheses (`0.13 * 1e-6`). ngspice/Xyce wrap a compound
  expression in `{…}`, HSPICE in `'…'`; Spectre, VACASK, SystemVerilog and VAMS
  print it bare. A literal or reference is never wrapped. Raw text is wrapped only
  when `named` (a `k=v` value) and not empty, already wrapped, or a bare word or
  number. `resolve` substitutes a reference (or a word of raw text) once. Numbers
  keep their SPICE SI suffix (SystemVerilog: plain, 12 significant digits); a
  prefix without a SPICE suffix prints its bare mantissa (`format_si_value`).
- SPICE, HSPICE and Xyce read `{…}`, `'…'` and bare identifiers; Spectre reads a
  bare value (`w=2*l+1u`) up to the next `name=`, `;` or unmatched `)`. A number
  stays `prefixed_value`, an identifier becomes a `reference`, anything else an
  expression tree. `.param` / `parameters` directives and analysis options keep
  the value as dialect text.
- Xyce drops the `PARAMS:` keyword from X-call statements.
- Spectre reads `include "p" section=s` as a `LIB` directive (`"p s"`) and puts a
  `lang=` option in the directive name.

## R7. Writer dialects

Ported from kfnetlist 5924917, 9766e84, 1adb071, 5671e52, d94dc38:

- `write_spice` (ngspice), `write_hspice`, `write_xyce` and `write_spectre_spice`
  are separate writers over one SPICE implementation; expression params are
  printed in the writer's dialect.
- SPICE family: subckt params go on the `.subckt` header (Xyce: after `PARAMS:`,
  also on X calls with params); an instance name gets its device letter when it
  lacks it (`X` for a defined subckt, else the external module's
  `spice_prefix`); a net named `properties["ground_net"]` is written as `0`; a
  positional `value` expression is written verbatim, an empty raw expression is
  not written.
- Spectre: `properties["dialect"] == "vacask"` selects VACASK (instance names
  lowercased, the `value` of R/C/L written as `r`/`c`/`l`, `ground 0` first when a
  ground net is set); `ground_net` is node `0`; a `LIB` directive is written
  `include "p" section=s`, plus the `lang=` option kept in its name.
- Every simulation writer (SPICE family, Spectre, VAMS) leaves out instances with
  `properties["sim_skip"] == "1"` and params with `properties["role"] == "source"`,
  printing a reference to such a param as its value.
- `write_dispatch` routes `hspice`, `xyce` and `spectre_spice` to their writers.
