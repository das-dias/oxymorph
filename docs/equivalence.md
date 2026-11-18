# Equivalence

oxymorph is a port of upstream [morphnet](https://github.com/das-dias/morphnet), not a re-imagination of it. Equivalence is **verified**, not asserted: upstream Python is the oracle, its verdict is captured into a committed corpus, and every claim below is produced by running both implementations and comparing.

## How it is proven

1. **Oracle corpus.** `tools/extract_oracle.py` harvests every committed netlist and every inline netlist in upstream's test suite, runs the upstream Python as the oracle, and writes the expected IR (JSON) and writer output to `tests/corpus/cases.json`.
2. **Offline test.** `crates/oxymorph-netlist/tests/parity.rs` replays the corpus against the Rust implementation with no Python toolchain. It currently checks **357 parse cases** and **2142 writer outputs**, all identical. It also proves the **protobuf interchange**: decoding upstream's `to_proto_bytes()` reproduces our IR for **357 cases**.
3. **Binding check.** The Python binding (`oxymorph`) is compared against upstream `morphnet` live for the same corpus across all six parsers and writers.

## Examples

Each example runs the same input through upstream and through oxymorph and shows both results. Every pair below is byte-identical.

### Spectre → Verilog-AMS

Input (`spectre`):

```spectre
subckt rc_lowpass (in_ out gnd)
R1 (in_ out) resistor value=1k
C1 (out gnd) capacitor value=100p
ends rc_lowpass
```

```python
circuit = morphnet.parse_spectre(src)
print(morphnet.write_vams(circuit))
```

```text
module rc_lowpass (in_, out, gnd);
  inout in_, out, gnd;
  resistor #(.value(1k)) R1 (.p(in_), .n(out));
  capacitor #(.value(100p)) C1 (.p(out), .n(gnd));
endmodule

```

✅ upstream `morphnet` and `oxymorph` produce identical output (167 bytes).

### Verilog-AMS → Spectre

Input (`verilog`):

```verilog
module rc_lowpass (in_, out, gnd);
  inout in_, out, gnd;
  resistor #(.value(1k)) R1 (.p(in_), .n(out));
  capacitor #(.value(100p)) C1 (.p(out), .n(gnd));
endmodule
```

```python
circuit = morphnet.parse_vams(src)
print(morphnet.write_spectre(circuit))
```

```text
subckt rc_lowpass (in_ out gnd)
R1 (in_ out) resistor value=1k
C1 (out gnd) capacitor value=100p
ends rc_lowpass

```

✅ upstream `morphnet` and `oxymorph` produce identical output (113 bytes).

### SPICE round-trip

Input (`spice`):

```spice
.SUBCKT rc in out gnd
R1 in out 1k
C1 out gnd 100p
.ENDS rc
.END
```

```python
circuit = morphnet.parse_spice(src)
print(morphnet.write_spice(circuit))
```

```text
* untitled
.subckt rc in out gnd
R1 in out 1k
C1 out gnd 100p
.ends rc
.end

```

✅ upstream `morphnet` and `oxymorph` produce identical output (76 bytes).

### Programmatic builder

The factory API mirrors upstream's `morphnet._api`. The same calls, against either library, produce the same IR:

```python
inv = lib.module("inverter", parameters=["l", "w"])
inv.add_terminal("inp", direction=lib.PortDirection.INPUT)
inv.add_terminal("out", direction=lib.PortDirection.OUTPUT)
inv.add(lib.module_reference("mn", "nfet", l=100e-9, w=500e-9))
inv.connect("net_out", ("mn", "drain"), ("", "out"))
circuit = lib.circuit("ring"); circuit.add(inv)
```

```json
{"name":"ring","top_module":"inverter","modules":[{"name":"inverter","class_name":"","ports":[{"name":"inp","direction":1,"domain":0,"width":0,"properties":{},"discipline":""},{"name":"out","direction":2,"domain":0,"width":0,"properties":{},"discipline":""}],"parameters":[{"name":"l","default_value":null,"description":"","properties":{}},{"name":"w","default_value":null,"description":"","properties":{}}],"model_interfaces":[],"module_references":[{"name":"mn","module_name":"nfet","class_name":"","parameters":[{"name":"l","default_value":{"prefixed_value":{"double_value":1e-7,"prefix":0},"model_ref":null},"description":"","properties":{}},{"name":"w","default_value":{"prefixed_value":{"double_value":5e-7,"prefix":0},"model_ref":null},"description":"","properties":{}}],"properties":{}}],"connections":[{"name":"net_out","source":{"instance_name":"mn","port_name":"drain"},"target":{"instance_name":"","port_name":"out"},"domain":0,"weight":0,"properties":{}}],"buses":[],"properties":{},"directives":[]}],"ext_modules":[],"properties":{},"directives":[],"simulation":null,"libraries":{}}
```

✅ identical IR from `morphnet` and `oxymorph`.

## Corpus

Parse cases per input format (all identical):

| format | cases |
|---|---|
| `hspice` | 78 |
| `spectre` | 27 |
| `spectre_spice` | 74 |
| `spice` | 67 |
| `vams` | 41 |
| `xyce` | 70 |
| **total** | **357** |

Writer outputs checked: **2142** (each parsed circuit written back in every format), all identical.

Protobuf interchange: **357** circuits decoded from upstream's `to_proto_bytes()` equal our parsed IR.
