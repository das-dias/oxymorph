# 0006. IR representation: hand-written serde types

- Status: implemented
- Date: 2026-02-11
- Supersedes: `0005-ir-and-protobuf.md` (the "start from prost-generated types" step)
- Related specs: `specs/ir.md`

## Context

Decision 0005 chose to start from prost-generated protobuf types as the concrete
IR and migrate later to hand-written idiomatic types. In practice the Pydantic
models are the observable behavior (field names, aliases, oneof validation,
JSON/YAML shape), and the parity oracle available in-repo is **JSON/YAML**
(the committed `tests/netlists/*.yaml` and `model_dump_json()`), not wire bytes.

proto3's weak typing (optional scalars, oneofs, maps) also fights decision
0001's typing goal, and prost codegen adds a `protoc` build dependency.

## Decision

Implement the IR as **hand-written Rust types** mirroring the Pydantic models
field-for-field, serialized with serde. JSON/YAML is the parity comparison
surface. Protobuf interchange (decision 0005 R1.7) is **deferred**; the `.proto`
files remain the documented wire contract for a future implementation.

Details:

- Enums are `#[repr(i32)]` + `serde_repr` so they serialize as integers, matching
  Pydantic `IntEnum`.
- Maps are `IndexMap` (insertion-ordered) because upstream dicts are ordered and
  writer output depends on that order.
- Field names match the Python field names (e.g. `ports`, `port_name`), so
  `to_json()` is interchangeable with upstream.

## Options considered

- **(b1) prost-generated types first** (decision 0005) — adds `protoc`, produces
  non-idiomatic types, and compares on a surface (wire bytes) the oracle does not
  exercise. Rejected in favour of the eventual target.
- **(b2) hand-written serde types (chosen)** — idiomatic, no `protoc`, matches the
  JSON oracle directly.

## Consequences

Easier: no protobuf toolchain; direct JSON/YAML parity; idiomatic enums/newtypes.

Harder / owed: protobuf wire compatibility (R1.7) is not implemented yet; when it
is, it must be added without changing the JSON shape.

## Verification

`crates/oxymorph-netlist/tests/parity.rs` compares `Circuit::to_json()` against
the oracle for 357 cases, and `tools/generate_equivalence_docs.py` compares it
live against upstream Pydantic `model_dump_json`.
