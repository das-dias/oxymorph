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
  at least two: fewer raise `ValueError`, as in kfnetlist.
- **Expressions and paths.** `parse_expression`, `format_expression`,
  `posix_file`, `is_absolute` and `resolve_file`.
- **Errors.** Parse, write and serialization failures raise `ValueError`.

## R2. Object model (`decisions/0012`)

- **IR classes.** Every IR type is a native `#[pyclass]` with the IR's field
  names, order and defaults. The constructors take keywords only. Every attribute
  can be read and written.
  - List and dict attributes are live: the getter returns the stored list or
    dict, so changing it in place changes what is written.
  - Assigning a list or dict attribute stores a new list or dict from any
    iterable or mapping.
  - A nested object is shared, so editing it changes what is written.
- **Enums.** The enums are `IntEnum`s with the IR's integer values: `SIPrefix`,
  `SignalDomain`, `PortDirection`, `DirectiveKind`, `ExternalModuleKind`,
  `AnalysisKind`, `OutputRequestKind`, `WaveformKind` and `Operator`. An enum
  attribute reads as the member. Setting it to an int that is no member's value
  raises `ValueError`.
- **Equality.** `==` compares the IR values. Objects are unhashable.
  `copy.copy`, `copy.deepcopy` and `pickle` work.
- **Serialization.** Every class has `to_json()` and `from_json()`, using the
  IR's serde JSON. The JSON is deterministic, and equal values give equal JSON.
- **Conversion.** The graph is converted to the IR at each parse, write,
  serialization and comparison. A wrongly typed list or dict item raises
  `TypeError` and names its path. A graph nested deeper than `MAX_NESTING` (128)
  steps, such as a cycle, raises `ValueError`.

## R3. Equivalence

For the committed corpus, the binding's parse JSON and writer output equal
upstream `morphnet` exactly (357 parse cases, 2142 writer outputs).

## R4. WebAssembly

`crates/oxymorph-wasm` (wasm-bindgen, `wasm32-unknown-unknown`) exposes
`formats()` (including `gsch`), `convert(text, in_format, out_format)`,
`to_json(text, in_format)` and `version()`, delegating to `oxymorph-netlist`. The docs playground loads the
`wasm-pack --target web` bundle from `docs/assets/playground/`.
