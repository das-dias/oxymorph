# API reference

The installed module is named `oxymorph`. It exposes the same functionality as
upstream `morphnet` through its own API (it is *not* API-compatible).

## Parsers

Each returns a `Circuit` and raises `ValueError` on parse failure.

```python
oxymorph.parse_spice(text: str) -> Circuit
oxymorph.parse_hspice(text: str) -> Circuit
oxymorph.parse_xyce(text: str) -> Circuit
oxymorph.parse_spectre(text: str) -> Circuit
oxymorph.parse_spectre_spice(text: str) -> Circuit
oxymorph.parse_vams(text: str) -> Circuit
```

## Writers

Each takes a `Circuit` and returns netlist text.

```python
oxymorph.write_spice(circuit: Circuit) -> str
oxymorph.write_hspice(circuit: Circuit) -> str
oxymorph.write_xyce(circuit: Circuit) -> str
oxymorph.write_spectre(circuit: Circuit) -> str
oxymorph.write_spectre_spice(circuit: Circuit) -> str
oxymorph.write_vams(circuit: Circuit) -> str
```

## Builder

Mirrors `morphnet._api`.

```python
oxymorph.circuit(name, *, top_module="", properties=None) -> Circuit
oxymorph.module(name, *, class_name="", parameters=None, properties=None) -> Module
oxymorph.module_reference(name, module_name, *, class_name="", parameters=None,
                          properties=None, **param_kwargs) -> ModuleReference
oxymorph.ext_module(name, *, class_name="", library_name="", group_name="",
                    ports=None, parameters=None, properties=None) -> ExternalModule
oxymorph.port(name, *, direction=PortDirection.INOUT,
              domain=SignalDomain.UNSPECIFIED, width=0, discipline="",
              properties=None) -> Port
oxymorph.connection(name, src, tgt, *, domain=SignalDomain.UNSPECIFIED,
                    properties=None) -> Connection
oxymorph.parameter(name, *, default_value=None, description="", properties=None) -> Parameter
oxymorph.external_library(path, *, model_type="", external_model_names=None) -> ExternalLibrary
oxymorph.model_interface(name, *, function_name="", parameters=None, properties=None) -> ModelInterface
```

`parameters` accepts a mix of `Parameter` objects, plain names (`"l"`), and
`(name, default)` tuples.

## Chaining methods

```python
Module.add_terminal(name, *, direction=0, domain=0, width=0, discipline="", properties=None) -> Module
Module.add(*refs: ModuleReference) -> Module
Module.connect(net_name, *endpoints, domain=0, properties=None) -> Module
Circuit.add(*modules: Module) -> Circuit
Circuit.add_ext(*ext: ExternalModule) -> Circuit
Circuit.add_library(name, lib: ExternalLibrary) -> None
```

## IR types

`Circuit`, `Module`, `Port`, `PortReference`, `Connection`, `Bus`,
`ModuleReference`, `Parameter`, `ParameterValue`, `PrefixedValue`,
`ModelReference`, `ModelInterface`, `Directive`, `ExternalModule`,
`ExternalLibrary`, `Simulation`, `Analysis`, `OutputRequest`, `Measurement`,
`InitialCondition`.

`Circuit` supports serialization:

```python
Circuit.to_json() -> str
Circuit.to_json_pretty() -> str
Circuit.to_yaml() -> str
Circuit.from_json(s: str) -> Circuit
Circuit.from_yaml(s: str) -> Circuit
```

and equality: two circuits compare equal iff their IR is equal.

## Enums

Exposed as integer constants (like upstream `IntEnum`):

```python
oxymorph.PortDirection.INOUT   # 0
oxymorph.PortDirection.INPUT   # 1
oxymorph.PortDirection.OUTPUT  # 2

oxymorph.SignalDomain.UNSPECIFIED  # 0
oxymorph.SignalDomain.ELECTRICAL   # 1
oxymorph.SignalDomain.WAVEGUIDE    # 2
```
