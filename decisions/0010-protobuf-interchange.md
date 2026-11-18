# 0010. Protobuf interchange implemented

- Status: implemented
- Date: 2026-02-11
- Related decisions: `0005-ir-and-protobuf.md`, `0006-ir-representation.md`
- Related specs: `specs/ir.md`, `specs/parity.md`

## Context

Decision 0006 chose hand-written serde IR types and **deferred** protobuf
interchange, because the parity oracle available in-repo is JSON/YAML. Upstream
nevertheless exposes `ProtoModel.to_proto_bytes()` / `from_proto_bytes()`, so a
full port owes the wire format.

## Decision

Implement protobuf interchange against the vendored `.proto` files:

- `crates/oxymorph-schema/build.rs` compiles `vendor/morphnet/protos/*.proto`
  with `prost-build` (requires `protoc`); the generated types live in
  `oxymorph_schema::proto::wire`.
- `proto.rs` maps the IR to/from the wire types, including the field renames
  (`ports` ↔ `terminals`, `port_name` ↔ `terminal_name`), the `ParameterValue`
  oneof, and the enums (numeric values match 1:1).
- `Circuit::to_proto_bytes()` / `from_proto_bytes()` mirror upstream.
- The corpus records upstream's `to_proto_bytes()` per case; the parity test
  asserts `from_proto_bytes(upstream_bytes) == parsed_ir` (message-equality, not
  byte-equality — prost serializes maps in a different order than Python).

## Consequences

The `protoc` build dependency is required to build `oxymorph-schema`.

## Verification

`tests/parity.rs` checks 357 protobuf cases; the docs equivalence page reports
357/357 message-equal. `to_proto_bytes` is byte-equal on 352/357 (the rest differ
only in map field ordering).
