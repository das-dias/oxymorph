# Parsing

Implemented parser behavior (`decisions/0002`, `decisions/0004`).

## R1. Engines

Each format embeds the verbatim upstream `.lark` grammar and mirrors upstream's
engine configuration exactly:

| Format | Grammar | Engine | Lexer |
|---|---|---|---|
| `spectre` | `spectre.lark` | LALR | contextual |
| `vams` | `vams.lark` | LALR | contextual |
| `systemverilog` | `systemverilog.lark` | LALR | contextual |
| `spice` | `spice.lark` | Earley, `ambiguity="resolve"` | auto/dynamic |
| `hspice` | `hspice.lark` | Earley, `ambiguity="resolve"` | auto/dynamic |
| `xyce` | `xyce.lark` | Earley, `ambiguity="resolve"` | auto/dynamic |

`spectre_spice` has no grammar: it splits input on `simulator lang=` and
delegates to the spice and spectre parsers (`preprocess_spectre_spice`). With the
`spectre_spice` feature built, `parse_spectre` does the same for a file with a
`simulator lang=` line, starting in Spectre.

`lark_rs` builds its lexer as a DFA, which has no Unicode `\b`: the SystemVerilog
grammar's keyword boundaries are compiled as ASCII ones (`(?-u:\b)`); the grammar
file itself stays verbatim.

## R2. Preprocessing

Per-format preprocessing runs before parsing and mirrors `*/preprocess.py`:
comment stripping, continuation joining, and (spectre) semicolon injection;
vams and systemverilog keep the active `` `ifdef `` / `` `elsif `` / `` `else ``
branches for the given `defines` (with none, the `` `else `` branch), expand
object-like macros, extract backtick directives, and replace `analog` blocks
(vams) or behavioral constructs (systemverilog: `always`, `initial`, `function`,
`generate`, a non-trivial `assign`, …) with numbered placeholders kept as
`analog_block` / `behavioral_block_<N>` properties. systemverilog strips
`(* attributes *)`, except a `pwl_file` attribute, which becomes the source's
parameter.

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
  ground net is set, subckt params on a `parameters k=v` line after the `subckt`
  header); `ground_net` is node `0`; a `LIB` directive is written
  `include "p" section=s`, plus the `lang=` option kept in its name.
- Macros (kfnetlist e82518b): every writer prints the body of a `Macro` whose
  `language` it takes after the directives (the circuit's) or after the module header
  (a module's own): the SPICE family takes `spice`, `ngspice`, `xyce`, `hspice`;
  Spectre takes `spectre`, and SPICE inside `simulator lang=spice` …
  `simulator lang=spectre`; VACASK takes none; SystemVerilog takes `systemverilog`,
  `verilog`; VAMS takes `verilog-ams`, `verilog-a`, `vams` (the rest travel in its
  `spice_code` attributes). Any other macro is a `skipped macro <name> (<language>)`
  comment (`*` or `//`), never a silent drop; kfnetlist also warns. The name and
  language are echoed on that one line, with CR, LF, other control characters and `\`
  (a line continuation in Spectre and some SPICE dialects) as spaces
  (`helpers::one_line`).
- A macro body is raw simulator code and is written verbatim (NUL characters
  dropped), `simulator lang=` switches and all, as kfnetlist writes it. A deck
  converted from a third-party netlist or `.gsch` therefore carries that party's
  simulator code (`.control` blocks, `shell`, …): treat it as code, not data.
- Every simulation writer (SPICE family, Spectre, VAMS) leaves out instances with
  `properties["sim_skip"] == "1"` and params with `properties["role"] == "source"`,
  printing a reference to such a param as its value.
- `write_dispatch` routes `hspice`, `xyce` and `spectre_spice` to their writers.

## R8. Stimuli, PWL sources and open pins

Ported from kfnetlist 75c630f, b1153f6, b4dc1ec, 30a1621, 6db1e11, 650763b,
5559e43, 8f0a4b4, 7970b4f, b26cd6b, d5b82b2 (`decisions/0011`);
`crates/oxymorph-netlist/src/stimulus.rs`:

- Every V/I source is a `Stimulus` in `Simulation.stimuli`, named
  `<module>/<instance>`; the source's `properties["stimulus"]` names it
  (`stimulus_of`, `attach`). Waveform args use SPICE semantics with Spectre names
  (`waveform_args`); writers walk them in that order, so args read back from
  protobuf (no order) are written by position.
- SPICE family: the tokens after a V/I source's nodes are its stimulus
  (`DC d AC m p FUNC(args)`, a bare first value is the DC level). `PULSE`, `SIN`,
  `EXP`, `SFFM` and `PWL` take positional args, also with a space before `(`;
  `PWL FILE "path"`, `PWL FILE=path` and LTspice `PWL file=path` read a file. Any
  other function (`AM(...)`) is `Error::Validation`. The writers print the same
  form; a missing positional arg before a given one is `0`, and a reference is
  wrapped (`{vdd}`).
- Spectre: `dc`, `mag`, `phase`, `type` and the waveform's args (also Cadence's
  `damp`, `td1`) of a `vsource`/`isource` are its stimulus; an unknown `type`
  (`prbs`) and its params stay device params. `wave=[…]` (one line) holds PWL
  points, `file="path"` a PWL file. A quoted string is kept as raw text. VACASK
  writes InSpice's forms: quoted `type`, no zero `delay`/`theta`, no `rise <= 0`,
  `td2` counted from the delay.
- VAMS: an LRM Annex E primitive (`vpulse`, `isine`, `vexp`, …) is a
  `vsource`/`isource` with its stimulus, and is written back as one (a DC/AC
  source is a `vsine`). Numbers take Verilog-A scale factors (`1M` is mega, written
  `M`); identifier params are references, strings raw text. A `paramset <model>
  <primitive>` makes `<model>` instances that primitive with `model_name`; the
  writer names a device by its `model_name`, declares the `paramset`s, and writes
  the circuit's `ground_net` in a module that uses it.
- A PWL file is stored POSIX (`oxymorph_schema::paths::posix_file`): `\` and `/`
  separate, `.`/`..` resolve, and a drive, UNC share or `/` root is kept.
- A bare word, `{…}` or `'…'` after an R/C/L's nodes is its `value` (an identifier
  a reference) unless a `.model` of the deck has that name, a value precedes it, or
  `k=v` params follow it; then it is the model name.
- The deck's top-level elements (`__top__`) are the top module whenever they exist;
  Spectre-SPICE merges the top-level elements of every section into one `__top__`.
- A net with one pin is a source-only connection. Writers put a pin with no net on
  its own node `nc_<instance>_<pin>` (SPICE: the instance name as written); a
  parser drops a one-pin net named `nc_<instance>_…`.

## R7. HDL nets (SystemVerilog, Verilog-AMS)

Ported from kfnetlist 1deb1e0, 28b80e0, 4c98724 (#34, #35), 650763b, f886515,
b26cd6b, d5b82b2 (`decisions/0011`); `src/hdl.rs`, `src/formats/systemverilog.rs`,
`src/formats/vams.rs`:

- `parse_systemverilog(text, &HdlOptions)` and `parse_vams_with(text, &HdlOptions)`
  (`parse_vams(text)` uses the defaults). `HdlOptions { defines, disciplines,
  pin_domains }`: predefined macros; port/net type → domain; cell → pin → domain.
- Nets are bits. A bus port `input [3:0] in` is one `Port` with `width = 4` and
  `properties["range"] = "3:0"`; its bits `in[3]` … `in[0]` are the net members, so a
  bit-select meets the port. Concatenations `{a, b[1:0]}` and replications `{2{a}}`
  resolve msb first; a literal is a constant tie. An instance array `X I[3:0] (…)`
  expands to `I[3]` … `I[0]`, slicing or broadcasting each connection (IEEE 1800
  23.3.3.5). A positional pin of a defined module takes its port name, else `p<i>`.
  A select of an undeclared bus, a bit outside its range, a width mismatch or a
  multi-dimensional bus is `Error::Validation`.
- SystemVerilog: ANSI and non-ANSI ports (a bare ANSI name inherits the previous
  direction and type), header and body parameters (`localparam` →
  `properties["localparam"]`, a type → `properties["type"]`; sized literals such as
  `4'b1010` are numbers), named, ordered, `.name`, `.*` and empty connections,
  `assign` / `alias` / `wire a = b` merging nets (a port names the merged net). The
  top is the last module no other instantiates. A source primitive is a stimulus, as
  in VAMS.
- A connection records its net's declared type as `properties["discipline"]`.
  Domains: a typed port or net in `disciplines`, and a pin in `pin_domains` (which
  wins), take that domain; a net touching a waveguide terminal is waveguide, and so
  are the untyped ports on it, up the hierarchy until nothing changes; the rest keep
  the `o<N>` rule.
- Verilog-AMS: `(* spice_lib = "…", spice_code = "…" *)` on the top module and
  `spice_module_code` on any module carry `LIB` directives and the macros in other
  languages (JSON); `electrical b = 5;` is a nodeset (`Simulation.node_sets`,
  `V(b)`); `(* pwl_file = "…" *)` on a `vpwl` is its PWL file; an override value is
  any expression (`.l(0.13 * 1e-6)`).
- Writers: buses are written as `[m:l]` declarations and connections regrouped into
  `bus`, `bus[i]`, `bus[m:l]` or `{…}`; a name that is not an identifier, or is a
  keyword, is escaped (`\0 `, `sv_name`); an unconnected port is `.p()`. A bus port
  only partly connected is written with its open bits on their own `nc_<instance>_<pin>`
  nodes (kfnetlist raises). SystemVerilog writes numbers plainly (`2e-07`), behavioral
  blocks back in place and a stimulus as its Annex E primitive (`(* pwl_file *)` for a
  PWL file); Verilog-AMS writes the attributes and nodesets above.
- Untrusted text: a bus range, a replication, an instance array, and the pins of one
  array connection expand to at most `hdl::MAX_EXPANDED_WIDTH` (1,000,000) bits or
  instances. On top of that, each parse holds one `hdl::ExpansionBudget` of
  `max(1_000_000, 64 × input length in bytes)` units. It is charged for every declared
  bit, every connection pin an expression or array yields (resolved bits, broadcasts,
  replications, triples), every instance (itself, its master's port bits, which a
  writer lists even when unconnected, and its params), and every byte `` `define ``
  expansion outputs. A parse tree nests at most `ast::MAX_TREE_DEPTH` (256) levels and
  procedural statements at most 128. Beyond any of these, the parse is
  `Error::Validation` (the budget's message names it).
- A module header that names a port twice (`module c (a, a);`, ANSI or not) is
  `Error::Validation` in both parsers: it is illegal Verilog, and every occurrence would
  expand the port's bits again (kfnetlist keeps every occurrence).
- Writers keep infallible signatures and trust the IR only up to the same cap: a port
  whose `properties["range"]` parses to more than 1,000,000 bits is written as a scalar,
  and so is one whose `width` does when it has no parseable range (an unparseable range
  falls back to `width`); a range is never walked. IR built from
  JSON can still cost up to instances × 1,000,000 pin names per port when written; a
  fallible `try_write_*` would bound that.

