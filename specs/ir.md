# IR

The intermediate representation (`decisions/0005`, `decisions/0006`).

## R1. Types

Hand-written Rust types mirror the upstream Pydantic models field-for-field:
`Circuit`, `Module`, `Port`, `PortReference`, `Connection`, `Bus`,
`ModuleReference`, `Parameter`, `ParameterValue`, `PrefixedValue`,
`ModelReference`, `ModelInterface`, `Directive`, `ExternalModule`,
`ExternalLibrary`, and the simulation types `Simulation`, `Analysis`,
`OutputRequest`, `Measurement`, `InitialCondition`.

## R2. Serialization

- Field names match the Python field names (`ports`, `port_name`, …).
- Enums serialize as integers (Pydantic `IntEnum`).
- Maps are insertion-ordered (`IndexMap`), matching Python dicts; writer output
  can depend on this order.
- `to_json` / `to_yaml` / `from_json` / `from_yaml` round-trip, and `Circuit`
  implements `PartialEq` on the IR.

## R3. Builder

`oxymorph_schema::builder` mirrors `morphnet/_api.py` (`circuit`, `module`,
`module_reference`, `ext_module`, `port`, `connection`, `parameter`,
`external_library`, `model_interface`), including mixed parameter specs
(`Parameter`, name, or `(name, default)`) and `module_reference` float kwargs.

## R4. Protobuf

Deferred (`decisions/0006`): the `.proto` files remain the documented wire
contract, but protobuf encoding of the IR is not implemented. JSON/YAML is the
comparison surface.

## Open

- Add protobuf interchange parity (`specs/parity.md` R1.7) without changing the
  JSON shape.
