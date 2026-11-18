# 0005. IR representation and protobuf strategy

- Status: implemented
- Date: 2026-02-10
- Supersedes: -
- Superseded by: -
- Related decisions: `0001-rust-port-of-morphnet.md`, `0003-workspace-layout.md`
- Related specs: `specs/ir.md` (TBD), `specs/parity.md`

## Context

Upstream describes the IR three ways: `protos/circuit.proto` +
`protos/simulation.proto` (the wire contract), the generated `*_pb2` Python
messages, and the Pydantic models in `morphnet_schema/_circuit.py` /
`_simulation.py` that wrap them via `ProtoModel` (`_base.py`).

Evidence that the Pydantic layer is not a thin mirror of the proto:

- **Renames.** Pydantic `Port` / `PortReference` / `PortDirection` map to proto
  `Terminal` / `TerminalReference` / `TerminalDirection`, mediated by field
  **aliases** (`ProtoModel` builds an alias map in both directions).
- **Invariants.** `@model_validator` methods enforce constraints the proto does
  not, e.g. `ParameterValue._check_oneof` (a oneof in the proto is a validated
  tagged union in Pydantic).
- **Hand-rolled conversion.** `to_proto` / `from_proto` are bespoke, with
  special handling for oneofs (`containing_oneof` + `HasField`), maps, repeated
  messages, and default-vs-set scalars. Presence semantics are non-trivial.
- **Serialization.** JSON/YAML are defined by Pydantic (`model_dump_json`,
  `pydantic_yaml`), not by the proto. Enums are `IntEnum`s mirroring proto enum
  values.

So the proto and the Pydantic models overlap but neither is a subset of the
other in terms of *behavior*. Note: proto3's weak typing (optional
scalars, oneofs, maps, no algebraic data types) is a poor fit for decision 0001's
"encode the IR in the type system" motivation.

## Decision

1. **The Pydantic models are normative for behavior.** When proto and Pydantic
   disagree, Python's observable behavior (validators, aliases, presence,
   JSON/YAML) wins, because that is what the parity corpus will exercise.
2. **The `.proto` files remain the shared interchange contract**, consumed from
   the vendored subtree, used for protobuf encode/decode and as the structural
   skeleton.
3. **Start from prost-generated types** (`prost-build` in a `build.rs` inside
   `oxymorph-schema`) as the concrete IR representation, with our own
   conversions to/from the generated messages doing the alias mapping and
   validator replication.
4. **Plan to migrate to hand-written idiomatic Rust types** (enums with payloads,
   `NonZero`/newtype invariants, typed connections) once parity is reached, with
   the prost types kept behind the conversion boundary as the wire types. This
   is the intended realization of decision 0001's typing motivation.

The behavior that prost cannot express — alias mapping, oneof/union validation,
presence semantics, JSON/YAML shape — is implemented explicitly in
`oxymorph-schema` and gated by `specs/parity.md`.

## Options considered

- **(a1) Proto normative** — simplest to generate, but contradicts observed
  behavior (validators, renames, serialization) and would make parity
  unwinnable. Rejected.
- **(a2) Pydantic normative (chosen)** — matches what actually runs; proto stays
  the contract.
- **(a3) Behavior/grammar normative** — no artifact to codegen from; would make
  proto sharing aspirational. Rejected.
- **(b1) prost-generated types as the IR (chosen to start)** — fastest path to
  parity; risk of non-idiomatic types leaking (proto3 optional/oneof/map shapes).
- **(b2) Hand-written IR + prost wire types (planned)** — idiomatic and
  type-safe, at the cost of maintaining conversions.
- **(b3) Proto deferred** — no interchange until parity; rejected because proto
  sharing is an explicit goal.

## Consequences

Easier: protobuf interchange works from day one; the schema crate can be
generated and validated quickly; the parity harness can compare canonical
protobuf encodings.

Harder / owed:

- The alias map, oneof validation, and presence rules must be reimplemented and
  tested; prost will not produce them.
- JSON/YAML output shape is a parity surface and must be specified in
  `specs/ir.md`, not assumed from the proto.
- The prost types are expected to be temporary; the migration to (b2) needs its
  own ADR and must not change the wire contract.
- proto3 has no algebraic data types, so tagged unions (e.g. `ParameterValue`)
  are modeled as oneofs in Rust and need exhaustive matching helpers.

## Verification

- Protobuf round-trips (Rust → bytes → Python and the reverse) are
  message-equal for the corpus (`specs/parity.md` R1.1).
- Every `@model_validator` in `_circuit.py`/`_simulation.py` has a corresponding
  Rust check and a test that a violating input is rejected identically.
- Field alias coverage is complete: no alias in `_circuit.py`/`_simulation.py`
  lacks a Rust mapping (checked by a generated/derived test).
