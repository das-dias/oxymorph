# 0012. Mutable Python object model

- Status: decided
- Date: 2026-11-26
- Supersedes: the "bindings are thin" part of `0008-python-bindings-and-docs.md`
- Superseded by: -
- Related decisions: `0008-python-bindings-and-docs.md`, `0011-extend-beyond-upstream-morphnet.md`
- Related specs: `specs/bindings.md`

## Context

`decisions/0008` made `oxymorph-python` a thin PyO3 layer: read-only `#[pyclass]`
wrappers whose getters clone the IR. gfpex and gflvs, which move from
`kfnetlist.morphnet` to oxymorph, change netlists in place:

- `module.module_references.extend(...)`, `module.connections = [...]`;
- `circuit.properties.update(...)` and `circuit.properties[key] = value`;
- `copy.deepcopy(circuit)`;
- content hashing with deepdiff `DeepHash`, or by walking dataclass fields.

With getters that clone, every one of these silently changes a copy. The
bindings also lack the kfnetlist surface: expressions, macros, stimuli,
SystemVerilog, `.gsch`, and the path helpers.

The controller recommended a graph of `#[pyclass]` objects that hold `Py<PyList>`
and `Py<PyDict>`, converted to the IR in a `convert.rs`. That needs, for each of
about 30 classes:

- a getter and a setter per field;
- `__eq__`, `__deepcopy__` and `__repr__`;
- `__reduce__` or `__getstate__` so that DeepHash and pickle can see inside;
- a conversion in each direction.

## Decision

**The object model is plain Python. The native module is a JSON bridge to the
Rust IR.**

- The package `oxymorph` lives in `crates/oxymorph-python/python/oxymorph/`, a
  maturin mixed project. Its classes are `dataclasses`, one per IR type, with the
  IR's field names, order and defaults. List and dict fields are real Python
  `list` and `dict` objects, so `extend`, `append`, item assignment and `update`
  change what is written.
- Enums are `enum.IntEnum`, with the IR's integer values. They compare equal to
  the IR integers and work on Python 3.9 (abi3-py39).
- The native module `oxymorph._oxymorph` has no classes. Its functions take and
  return the IR's JSON (`Circuit::to_json` / `from_json`, `decisions/0006`): parse,
  write, YAML, protobuf, expressions and paths.
- The conversion happens at the call. A parse or a `from_*` decodes the JSON into
  dataclasses. A write or a `to_*` encodes the dataclasses to JSON, and Rust reads
  that JSON back into an `oxymorph_schema::Circuit`. One encoder and one decoder,
  both in `__init__.py`, drive the conversion from the dataclass type hints.
- The encoder checks every value against its field's type. A wrong type raises
  `TypeError` and names the path, e.g.
  `modules[0].module_references[3]: expected ModuleReference, got str`. The walk is
  bounded at `MAX_NESTING` (128) levels, the JSON nesting serde_json reads. A deeper graph, or a cycle, raises
  `ValueError`, not `RecursionError`. serde_json bounds the Rust side.
- Equality is by value and is order-sensitive. `a == b` compares the encoded JSON
  text, so dict key order counts, as it does in written netlists. Equal objects
  therefore give equal `to_json()`, and `to_json()` is deterministic. Objects are
  unhashable (they are mutable).
- `copy.deepcopy`, `pickle`, deepdiff `DeepHash` and the gflvs
  dataclass-walking normalizer work on plain dataclasses with no extra code.

This is materially simpler than the `#[pyclass]` graph. It satisfies the same
tests with one Python module and a Rust module of about 200 lines, instead of
about 30 classes with hand-written accessors, protocols and conversions. The Rust
layer is thinner than before: no IR logic, only JSON in and out.

## Consequences

Easier:

- gfpex and gflvs keep their mutation, copying and hashing code.
- A new IR field is one dataclass line. There is no Rust accessor to add.

Harder:

- Each parse, write or serialization pays a JSON encode and decode. This is linear,
  and small next to the parse itself.
- The Python classes must track the IR by hand. `test_every_ir_field_round_trips`
  pins them through a full JSON round trip of a parsed corpus deck.
- An expression nested more than about 60 levels does not cross the bridge, as
  for `Circuit::from_json` in Rust: serde_json stops at 128 JSON levels.

Owed: the binding tests run with pytest (`just test-python`), separate from
`cargo test`, which stays Python-free.

## Verification

- `crates/oxymorph-python/tests/test_oxymorph.py`, run with `just test-python`:
  - `test_nested_mutation_is_written`;
  - `test_deepcopy_is_independent`;
  - `test_equality_by_value`;
  - `test_to_json_is_stable_for_hashing`;
  - `test_deephash_is_stable`;
  - the gfpex-style and gflvs-style consumer tests.
- `cargo clippy --manifest-path crates/oxymorph-python/Cargo.toml -- -D warnings`.
