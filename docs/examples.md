# Examples

## Convert between formats

All parsers produce the same `Circuit`, and all writers consume it, so any format
can be converted to any other:

```python
import oxymorph

spectre = """\
subckt rc_lowpass (in_ out gnd)
R1 (in_ out) resistor value=1k
C1 (out gnd) capacitor value=100p
ends rc_lowpass
"""

circuit = oxymorph.parse_spectre(spectre)
print(oxymorph.write_spice(circuit))
```

```spice
* untitled
.subckt rc_lowpass in_ out gnd
R1 in_ out resistor 1k
C1 out gnd capacitor 100p
.ends rc_lowpass
.end
```

## Inspect the IR

```python
circuit = oxymorph.parse_spectre(spectre)
print(circuit.top_module)          # rc_lowpass
print(circuit.to_json()[:80])      # {"name":"","top_module":"rc_lowpass",...
module = circuit.modules[0]
print([p.name for p in module.ports])            # ['in_', 'out', 'gnd']
print([r.name for r in module.module_references])  # ['R1', 'C1']
```

The IR round-trips through JSON or YAML:

```python
circuit2 = oxymorph.Circuit.from_yaml(circuit.to_yaml())
assert circuit2 == circuit
```

## Build a circuit programmatically

The builder API mirrors upstream's `morphnet._api`:

```python
import oxymorph

inv = oxymorph.module("inverter", parameters=["l", "w"])
inv.add_terminal("vdd", direction=oxymorph.PortDirection.INPUT)
inv.add_terminal("gnd", direction=oxymorph.PortDirection.INPUT)
inv.add_terminal("inp", direction=oxymorph.PortDirection.INPUT)
inv.add_terminal("out", direction=oxymorph.PortDirection.OUTPUT)

inv.add(
    oxymorph.module_reference("mn", "nfet", l=100e-9, w=500e-9),
    oxymorph.module_reference("mp", "pfet", l=100e-9, w=1000e-9),
)

inv.connect("net_out", ("mn", "drain"), ("mp", "drain"), ("", "out"))
inv.connect("net_inp", ("", "inp"), ("mn", "gate"), ("mp", "gate"))
inv.connect("vdd", ("", "vdd"), ("mp", "source"))
inv.connect("gnd", ("", "gnd"), ("mn", "source"))

circuit = oxymorph.circuit("ring")
circuit.add(inv)

print(oxymorph.write_vams(circuit))
```

## Exposed types

The Python module exposes the whole IR: `Circuit`, `Module`, `Port`,
`PortReference`, `Connection`, `Bus`, `ModuleReference`, `Parameter`,
`ParameterValue`, `PrefixedValue`, `ModelReference`, `ModelInterface`,
`Directive`, `ExternalModule`, `ExternalLibrary`, `Simulation`, `Analysis`,
`OutputRequest`, `Measurement`, `InitialCondition`, and the enums
`PortDirection` / `SignalDomain` as integer constants.
