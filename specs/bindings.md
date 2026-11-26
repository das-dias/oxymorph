# Python bindings

The `oxymorph` Python module (`decisions/0008`).

## R1. Surface

- **Parsers.** `parse_{spice,hspice,xyce,spectre,spectre_spice}(text)`,
  `parse_{systemverilog,vams}(text, defines=, disciplines=, pin_domains=)`,
  `parse_gsch(text, *, models=, sim=, corners=, name=)` and
  `read_gsch(path, *, project_root=, models=, sim=, corners=)` each return a
  `Circuit`.
- **Writers.** `write_{spice,hspice,xyce,spectre,spectre_spice,vams,systemverilog,gsch}(circuit)`
  returns `str`.
- **Builders.** The builder functions mirror kfnetlist's `morphnet._api`, with
  oxymorph names.
- **Chaining.** `Circuit.add`, `add_ext` and `add_library`, and `Module.add_terminal`,
  `add` and `connect`, return `self`. `Circuit.add` sets `top_module` to the first
  module added when it is unset. `connect` chains its endpoints pairwise and needs
  at least two.
- **Expressions and paths.** `parse_expression`, `format_expression`,
  `posix_file`, `is_absolute` and `resolve_file`.
- **Errors.** Parse, write and serialization failures raise `ValueError`.

## R2. Object model (`decisions/0012`)

- **IR classes.** Every IR type is a Python dataclass with the IR's field names,
  order and defaults. Every attribute can be read and written. List and dict
  attributes are plain lists and dicts, and changing them changes what is
  written.
- **Enums.** The enums are `IntEnum`s with the IR's integer values: `SIPrefix`,
  `SignalDomain`, `PortDirection`, `DirectiveKind`, `ExternalModuleKind`,
  `AnalysisKind`, `OutputRequestKind`, `WaveformKind` and `Operator`.
- **Equality.** `==` compares the encoded JSON, so it is by value and sensitive
  to dict order. Objects are unhashable. `copy.deepcopy` and `pickle` work.
- **Serialization.** `Circuit.to_json()` is the IR's canonical serde JSON. It is
  deterministic, and equal circuits give equal JSON.
- **Conversion.** The native module `oxymorph._oxymorph` takes and returns IR
  JSON and holds no IR logic. The encoder raises `TypeError` with the path of a
  wrongly typed value. It raises `ValueError` beyond `MAX_NESTING` (128) levels.
  The decoder rejects unknown fields.

## R3. Equivalence

For the committed corpus, the binding's parse JSON and writer output equal
upstream `morphnet` exactly (357 parse cases, 2142 writer outputs).

## R4. WebAssembly

`crates/oxymorph-wasm` (wasm-bindgen, `wasm32-unknown-unknown`) exposes
`formats()` (including `gsch`), `convert(text, in_format, out_format)`,
`to_json(text, in_format)` and `version()`, delegating to `oxymorph-netlist`. The docs playground loads the
`wasm-pack --target web` bundle from `docs/assets/playground/`.
