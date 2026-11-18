# Python bindings

The `oxymorph` Python module (`decisions/0008`).

## R1. Surface

- Parsers `parse_{spice,hspice,xyce,spectre,spectre_spice,vams}(text) -> Circuit`.
- Writers `write_{...}(circuit) -> str`.
- Builder functions mirroring `morphnet._api`.
- IR classes with getters, `to_json`/`to_yaml`/`from_json`/`from_yaml`, and
  `__eq__`; enums as integer constants.
- `Circuit.add`/`add_ext`/`add_library`; `Module.add_terminal`/`add`/`connect`
  (chaining).

## R2. Thin

The crate contains no parsing or IR logic; it converts arguments and delegates to
`oxymorph-schema` / `oxymorph-netlist`. Errors surface as `ValueError`.

## R3. Equivalence

For the committed corpus, the binding's parse JSON and writer output equal
upstream `morphnet` exactly (357 parse cases, 2142 writer outputs).
