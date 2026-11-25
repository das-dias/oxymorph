# IR

The intermediate representation (`decisions/0005`, `decisions/0006`).

## R1. Types

Hand-written Rust types mirror the upstream Pydantic models field-for-field:
`Circuit`, `Module`, `Port`, `PortReference`, `Connection`, `Bus`,
`ModuleReference`, `Parameter`, `ParameterValue`, `PrefixedValue`,
`ModelReference`, `ModelInterface`, `Directive`, `ExternalModule`,
`ExternalLibrary`, and the simulation types `Simulation`, `Analysis`,
`OutputRequest`, `Measurement`, `InitialCondition`.

Types and fields added by the kfnetlist port mirror kfnetlist's Pydantic models
at `e596bb5` (`decisions/0011`):

- `Expression` (one optional field per node: `literal`, `reference`, `unary`,
  `binary`, `call`, `ternary`, `raw`) with `UnaryOp`, `BinaryOp`, `Call`,
  `Ternary` and the `Operator` enum; `Expression::literal(f64)` and
  `Expression::reference(&str)` build the leaves. `raw` is dialect text no
  grammar parsed.
- `ParameterValue.expression`, a third oneof variant beside `prefixed_value` and
  `model_ref`.
- `Macro { name, language, body, properties }`, a verbatim block in a simulator
  language, on `Module.macros` and `Circuit.macros`.
- `Stimulus { name, dc, ac_magnitude, ac_phase, waveform }` on
  `Simulation.stimuli`, named `"<module>/<instance>"` after the source element
  whose `properties["stimulus"]` points at it; `Waveform { kind, args, points,
  file }` with `WaveformKind` (`Pulse`, `Sin`, `Exp`, `Sffm`, `Pwl`).

`validate()` mirrors the Pydantic validators, and is called explicitly:
`ParameterValue` and `Expression` (whole tree) reject more than one variant set
(`"… is a oneof: …"`); `Waveform` rejects `points` together with a `file`, and a
`file` on a non-PWL waveform.

## R2. Serialization

- Field names match the Python field names (`ports`, `port_name`, …).
- Enums serialize as integers (Pydantic `IntEnum`).
- Maps are insertion-ordered (`IndexMap`), matching Python dicts; writer output
  can depend on this order.
- `to_json` / `to_yaml` / `from_json` / `from_yaml` round-trip, and `Circuit`
  implements `PartialEq` on the IR.
- Fields added by the kfnetlist port are left out when unset or empty (`None`
  options, empty `macros` / `stimuli`), as kfnetlist's `to_json` does; the
  original fields keep morphnet's `null`s, which the pre-fork corpus pins.
  `Expression` JSON is flat (`{"binary": {"op": 3, "lhs": …, "rhs": …}}`).
- `Waveform.file` is stored POSIX (`paths::posix_file`) on read and write; `""`
  is no file.

## R3. Builder

`oxymorph_schema::builder` mirrors `morphnet/_api.py` (`circuit`, `module`,
`module_reference`, `ext_module`, `port`, `connection`, `parameter`,
`external_library`, `model_interface`), including mixed parameter specs
(`Parameter`, name, or `(name, default)`) and `module_reference` float kwargs.

## R4. Protobuf

Implemented (`decisions/0010`). The protos live in `crates/oxymorph-schema/proto/`
(`circuit.proto`, `simulation.proto`, `value.proto`): kfnetlist's, with message
and field names mapped back to oxymorph's and kfnetlist's field numbers
(`decisions/0011`). They are upstream's plus fields, so the vendored
`vendor/morphnet/protos/` stay the reference for the original corpus. They are
compiled with `prost-build`; `Circuit::to_proto_bytes()` and
`Circuit::from_proto_bytes()` round-trip through the wire format, mapping field
renames, the `ParameterValue` and `Expression` oneofs, and the enums.
`from_proto_bytes(upstream.to_proto_bytes())` equals the parsed IR for the whole
corpus (message-equality).

## R5. Paths

`oxymorph_schema::paths` (`posix_file`, `is_absolute`, `resolve_file`) gives one
answer on every OS, the same as kfnetlist's: `\` and `/` both separate, `.` and
`..` resolve lexically, `/x`, `C:/x`, `C:x` and `\\server\share` are absolute,
and `..` never climbs above a root (`/`, a drive, a share). The shared case table
is `crates/oxymorph-schema/tests/data/path_cases.json`.
