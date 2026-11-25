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
