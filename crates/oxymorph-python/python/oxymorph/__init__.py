"""oxymorph: netlist parsing and writing through a shared IR (decisions/0012).

The object model is plain dataclasses and `IntEnum`s with the IR's field names, order and defaults; list and dict
attributes are ordinary Python lists and dicts, so mutating them changes what is written. The native module
`oxymorph._oxymorph` parses, writes and serializes the IR's JSON; this module converts between that JSON and the
dataclasses at each call.
"""

from __future__ import annotations

import dataclasses
import enum
import functools
import json
import os
import typing
from typing import Any, Dict, List, Optional, Sequence, Tuple, Union

from . import _oxymorph

__version__ = _oxymorph.__version__

# The deepest object graph converted, in attribute/item steps: one step is one JSON level, and serde_json reads at
# most 128. A cycle stops here instead of overflowing the stack.
MAX_NESTING = 128

# ---------------------------------------------------------------------------
# Enums (the IR's integer values)
# ---------------------------------------------------------------------------


class PortDirection(enum.IntEnum):
    INOUT = 0
    INPUT = 1
    OUTPUT = 2


class SignalDomain(enum.IntEnum):
    UNSPECIFIED = 0
    ELECTRICAL = 1
    WAVEGUIDE = 2


SIPrefix = enum.IntEnum(
    "SIPrefix",
    [
        (name, value)
        for value, name in enumerate(
            "UNSPECIFIED QUECTO RONTO YOCTO ZEPTO ATTO FEMTO PICO NANO MICRO MILLI CENTI DECI "
            "DECA HECTO KILO MEGA GIGA TERA PETA EXA ZETTA YOTTA RONNA QUETTA".split()
        )
    ],
    module=__name__,
)


class ExternalModuleKind(enum.IntEnum):
    UNSPECIFIED = 0
    DEVICE = 1
    MODEL = 2
    NATURE = 3
    DISCIPLINE = 4


class DirectiveKind(enum.IntEnum):
    UNSPECIFIED = 0
    INCLUDE = 1
    LIB = 2
    GLOBAL = 3
    OPTION = 4
    PARAM = 5
    DEFINE = 6
    TIMESCALE = 7


class AnalysisKind(enum.IntEnum):
    UNSPECIFIED = 0
    OP = 1
    DC = 2
    AC = 3
    TRAN = 4
    NOISE = 5
    TF = 6
    SENS = 7
    PZ = 8
    DISTO = 9
    FOUR = 10
    FFT = 11


class OutputRequestKind(enum.IntEnum):
    UNSPECIFIED = 0
    PRINT = 1
    PLOT = 2
    PROBE = 3
    SAVE = 4


class WaveformKind(enum.IntEnum):
    UNSPECIFIED = 0
    PULSE = 1
    SIN = 2
    EXP = 3
    SFFM = 4
    PWL = 5


Operator = enum.IntEnum(
    "Operator",
    [
        (name, value)
        for value, name in enumerate("UNSPECIFIED ADD SUB MUL DIV POW MOD NEG NOT EQ NE LT LE GT GE AND OR".split())
    ],
    module=__name__,
)

# ---------------------------------------------------------------------------
# Conversion between dataclasses and the IR's JSON
# ---------------------------------------------------------------------------


@functools.lru_cache(maxsize=None)
def _field_types(model_class: type) -> Tuple[Tuple[str, Any], ...]:
    hints = typing.get_type_hints(model_class)
    return tuple((model_field.name, hints[model_field.name]) for model_field in dataclasses.fields(model_class))


def _optional_inner(hint: Any) -> Any:
    """`X` of `Optional[X]`, else None."""
    if typing.get_origin(hint) is Union:
        return next(argument for argument in typing.get_args(hint) if argument is not type(None))
    return None


def _child_path(path: str, step: str) -> str:
    return f"{path}.{step}" if path else step


def _encode(value: Any, hint: Any, path: str, depth: int) -> Any:
    """`value` as JSON data, checked against `hint`; TypeError names the `path` of a wrong type."""
    if depth > MAX_NESTING:
        raise ValueError(f"{path}: nested deeper than {MAX_NESTING} levels (a cycle?)")
    inner = _optional_inner(hint)
    if inner is not None:
        if value is None:
            return None
        hint = inner
    origin = typing.get_origin(hint)
    if origin is list:
        if not isinstance(value, (list, tuple)):
            raise TypeError(f"{path}: expected a list, got {type(value).__name__}")
        (item_hint,) = typing.get_args(hint)
        return [_encode(item, item_hint, f"{path}[{index}]", depth + 1) for index, item in enumerate(value)]
    if origin is dict:
        if not isinstance(value, dict):
            raise TypeError(f"{path}: expected a dict, got {type(value).__name__}")
        _, item_hint = typing.get_args(hint)
        encoded = {}
        for key, item in value.items():
            if not isinstance(key, str):
                raise TypeError(f"{path}: expected str keys, got {type(key).__name__}")
            encoded[key] = _encode(item, item_hint, f"{path}[{key!r}]", depth + 1)
        return encoded
    if dataclasses.is_dataclass(hint):
        if not isinstance(value, hint):
            raise TypeError(f"{path or 'value'}: expected {hint.__name__}, got {type(value).__name__}")
        return {
            name: _encode(getattr(value, name), field_hint, _child_path(path, name), depth + 1)
            for name, field_hint in _field_types(hint)
        }
    if isinstance(hint, type) and issubclass(hint, enum.IntEnum):
        if isinstance(value, bool) or not isinstance(value, int):
            raise TypeError(f"{path}: expected {hint.__name__}, got {type(value).__name__}")
        try:
            return int(hint(value))
        except ValueError:
            raise ValueError(f"{path}: {value} is not a {hint.__name__}") from None
    if hint is float:
        if isinstance(value, bool) or not isinstance(value, (int, float)):
            raise TypeError(f"{path}: expected float, got {type(value).__name__}")
        return float(value)
    if hint is int:
        if isinstance(value, bool) or not isinstance(value, int):
            raise TypeError(f"{path}: expected int, got {type(value).__name__}")
        return value
    if not isinstance(value, hint):
        raise TypeError(f"{path}: expected {hint.__name__}, got {type(value).__name__}")
    return value


def _decode(data: Any, hint: Any) -> Any:
    """JSON data the native module produced (bounded by serde_json) as dataclasses."""
    inner = _optional_inner(hint)
    if inner is not None:
        return None if data is None else _decode(data, inner)
    origin = typing.get_origin(hint)
    if origin is list:
        (item_hint,) = typing.get_args(hint)
        return [_decode(item, item_hint) for item in data]
    if origin is dict:
        _, item_hint = typing.get_args(hint)
        return {key: _decode(item, item_hint) for key, item in data.items()}
    if dataclasses.is_dataclass(hint):
        field_types = dict(_field_types(hint))
        unexpected = sorted(set(data) - set(field_types))
        if unexpected:
            raise TypeError(f"{hint.__name__}: unexpected fields {unexpected}")
        return hint(**{name: _decode(item, field_types[name]) for name, item in data.items()})
    if isinstance(hint, type) and issubclass(hint, enum.IntEnum):
        return hint(data)
    if hint is float:
        return float(data)
    return data


def _dumps(value: Any) -> str:
    return json.dumps(_encode(value, type(value), "", 0), separators=(",", ":"))


class _Model:
    """Equality by value, sensitive to dict order (as written netlists are); unhashable (mutable)."""

    __hash__ = None  # type: ignore[assignment]

    def __eq__(self, other: object) -> bool:
        if type(self) is not type(other):
            return NotImplemented
        return _dumps(self) == _dumps(other)

    def to_dict(self) -> Dict[str, Any]:
        return _encode(self, type(self), "", 0)

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> Any:
        return _decode(data, cls)


def _model(model_class: type) -> type:
    return dataclasses.dataclass(eq=False)(model_class)


def _list() -> Any:
    return dataclasses.field(default_factory=list)


def _dict() -> Any:
    return dataclasses.field(default_factory=dict)


# ---------------------------------------------------------------------------
# IR classes
# ---------------------------------------------------------------------------


@_model
class PrefixedValue(_Model):
    double_value: float = 0.0
    prefix: SIPrefix = SIPrefix.UNSPECIFIED


@_model
class UnaryOp(_Model):
    op: Operator = Operator.UNSPECIFIED
    operand: Optional[Expression] = None


@_model
class BinaryOp(_Model):
    op: Operator = Operator.UNSPECIFIED
    lhs: Optional[Expression] = None
    rhs: Optional[Expression] = None


@_model
class Call(_Model):
    name: str = ""
    args: List[Expression] = _list()


@_model
class Ternary(_Model):
    cond: Optional[Expression] = None
    then: Optional[Expression] = None
    otherwise: Optional[Expression] = None


@_model
class Expression(_Model):
    """One node kind set at most (a oneof)."""

    literal: Optional[PrefixedValue] = None
    reference: Optional[str] = None
    unary: Optional[UnaryOp] = None
    binary: Optional[BinaryOp] = None
    call: Optional[Call] = None
    ternary: Optional[Ternary] = None
    raw: Optional[str] = None


@_model
class ModelReference(_Model):
    model_interface_name: str = ""
    arguments: Dict[str, ParameterValue] = _dict()


@_model
class ParameterValue(_Model):
    """One of `prefixed_value`, `model_ref`, `expression` set at most (a oneof)."""

    prefixed_value: Optional[PrefixedValue] = None
    model_ref: Optional[ModelReference] = None
    expression: Optional[Expression] = None


@_model
class Parameter(_Model):
    name: str = ""
    default_value: Optional[ParameterValue] = None
    description: str = ""
    properties: Dict[str, str] = _dict()


@_model
class Directive(_Model):
    kind: DirectiveKind = DirectiveKind.UNSPECIFIED
    name: str = ""
    value: str = ""


@_model
class Macro(_Model):
    """A multi-line block in a simulator `language`, written verbatim."""

    name: str = ""
    language: str = ""
    body: str = ""
    properties: Dict[str, str] = _dict()


@_model
class ModelInterface(_Model):
    name: str = ""
    function_name: str = ""
    parameters: List[Parameter] = _list()
    properties: Dict[str, str] = _dict()


@_model
class PortReference(_Model):
    """`instance_name` "" is the module's own port."""

    instance_name: str = ""
    port_name: str = ""


@_model
class Port(_Model):
    name: str = ""
    direction: PortDirection = PortDirection.INOUT
    domain: SignalDomain = SignalDomain.UNSPECIFIED
    width: int = 0
    properties: Dict[str, str] = _dict()
    discipline: str = ""


@_model
class Connection(_Model):
    name: str = ""
    source: Optional[PortReference] = None
    target: Optional[PortReference] = None
    domain: SignalDomain = SignalDomain.UNSPECIFIED
    weight: int = 0
    properties: Dict[str, str] = _dict()


@_model
class Bus(_Model):
    name: str = ""
    width: int = 0
    domain: SignalDomain = SignalDomain.UNSPECIFIED
    connections: List[Connection] = _list()
    properties: Dict[str, str] = _dict()


@_model
class ModuleReference(_Model):
    name: str = ""
    module_name: str = ""
    class_name: str = ""
    parameters: List[Parameter] = _list()
    properties: Dict[str, str] = _dict()


@_model
class Module(_Model):
    name: str = ""
    class_name: str = ""
    ports: List[Port] = _list()
    parameters: List[Parameter] = _list()
    model_interfaces: List[ModelInterface] = _list()
    module_references: List[ModuleReference] = _list()
    connections: List[Connection] = _list()
    buses: List[Bus] = _list()
    properties: Dict[str, str] = _dict()
    directives: List[Directive] = _list()
    macros: List[Macro] = _list()

    def add_terminal(
        self,
        name: str,
        *,
        direction: int = PortDirection.INOUT,
        domain: int = SignalDomain.UNSPECIFIED,
        width: int = 0,
        discipline: str = "",
        properties: Optional[Dict[str, str]] = None,
    ) -> Module:
        self.ports.append(
            port(name, direction=direction, domain=domain, width=width, discipline=discipline, properties=properties)
        )
        return self

    def add(self, *module_references: ModuleReference) -> Module:
        self.module_references.extend(module_references)
        return self

    def connect(
        self,
        net_name: str,
        *endpoints: Tuple[str, str],
        domain: int = SignalDomain.UNSPECIFIED,
        properties: Optional[Dict[str, str]] = None,
    ) -> Module:
        """Chained connections on net `net_name`: endpoints A, B, C give A→B and B→C. An endpoint is
        `(instance_name, port_name)`; instance "" is this module's own port. Pass `(a, a)` to name a lone terminal's
        net."""
        if len(endpoints) < 2:
            raise ValueError(f"connect({net_name!r}): need >= 2 endpoints, got {len(endpoints)}")
        for source, target in zip(endpoints, endpoints[1:]):
            self.connections.append(connection(net_name, source, target, domain=domain, properties=properties))
        return self


@_model
class ExternalModule(_Model):
    name: str = ""
    class_name: str = ""
    library_name: str = ""
    group_name: str = ""
    ports: List[Port] = _list()
    parameters: List[Parameter] = _list()
    properties: Dict[str, str] = _dict()
    kind: ExternalModuleKind = ExternalModuleKind.UNSPECIFIED


@_model
class ExternalLibrary(_Model):
    path: str = ""
    model_type: str = ""
    external_model_names: List[str] = _list()


@_model
class Analysis(_Model):
    kind: AnalysisKind = AnalysisKind.UNSPECIFIED
    name: str = ""
    arguments: List[str] = _list()
    options: Dict[str, str] = _dict()


@_model
class OutputRequest(_Model):
    kind: OutputRequestKind = OutputRequestKind.UNSPECIFIED
    analysis_type: str = ""
    variables: List[str] = _list()


@_model
class Measurement(_Model):
    name: str = ""
    analysis_type: str = ""
    body: str = ""


@_model
class InitialCondition(_Model):
    conditions: Dict[str, str] = _dict()


@_model
class Waveform(_Model):
    """A transient source shape; a PWL keeps `points` (t0 v0 t1 v1 ...) or a `file`, stored POSIX."""

    kind: WaveformKind = WaveformKind.UNSPECIFIED
    args: Dict[str, ParameterValue] = _dict()
    points: List[ParameterValue] = _list()
    file: Optional[str] = None


@_model
class Stimulus(_Model):
    """What drives one V/I source, named `"<module>/<instance>"`."""

    name: str = ""
    dc: Optional[ParameterValue] = None
    ac_magnitude: Optional[ParameterValue] = None
    ac_phase: Optional[ParameterValue] = None
    waveform: Optional[Waveform] = None


@_model
class Simulation(_Model):
    analyses: List[Analysis] = _list()
    output_requests: List[OutputRequest] = _list()
    measurements: List[Measurement] = _list()
    initial_conditions: Optional[InitialCondition] = None
    node_sets: Optional[InitialCondition] = None
    temperatures: List[float] = _list()
    stimuli: List[Stimulus] = _list()
    options: Dict[str, str] = _dict()


@_model
class Circuit(_Model):
    """The top-level container. `add` sets `top_module` to the first module added when unset."""

    name: str = ""
    top_module: str = ""
    modules: List[Module] = _list()
    ext_modules: List[ExternalModule] = _list()
    properties: Dict[str, str] = _dict()
    directives: List[Directive] = _list()
    simulation: Optional[Simulation] = None
    libraries: Dict[str, ExternalLibrary] = _dict()
    macros: List[Macro] = _list()

    def add(self, *modules: Module) -> Circuit:
        self.modules.extend(modules)
        if not self.top_module and modules:
            self.top_module = modules[0].name
        return self

    def add_ext(self, *ext_modules: ExternalModule) -> Circuit:
        self.ext_modules.extend(ext_modules)
        return self

    def add_library(self, name: str, library: ExternalLibrary) -> Circuit:
        self.libraries[name] = library
        return self

    def to_json(self) -> str:
        """Canonical IR JSON: deterministic, equal for equal circuits (a content hash input)."""
        return _oxymorph.circuit_to_json(_dumps(self), False)

    def to_json_pretty(self) -> str:
        return _oxymorph.circuit_to_json(_dumps(self), True)

    def to_yaml(self) -> str:
        return _oxymorph.circuit_to_yaml(_dumps(self))

    def to_proto_bytes(self) -> bytes:
        return _oxymorph.circuit_to_proto(_dumps(self))

    @classmethod
    def from_json(cls, text: str) -> Circuit:
        return _circuit(_oxymorph.circuit_to_json(text, False))

    @classmethod
    def from_yaml(cls, text: str) -> Circuit:
        return _circuit(_oxymorph.circuit_from_yaml(text))

    @classmethod
    def from_proto_bytes(cls, data: bytes) -> Circuit:
        return _circuit(_oxymorph.circuit_from_proto(data))


# String annotations resolve against this module, so forward references (Expression, ParameterValue) work.
for _model_class in list(_Model.__subclasses__()):
    _field_types(_model_class)


def _circuit(circuit_json: str) -> Circuit:
    return _decode(json.loads(circuit_json), Circuit)


# ---------------------------------------------------------------------------
# Builders
# ---------------------------------------------------------------------------

ParameterSpec = Union[Parameter, Tuple[str, float], str]


def _make_parameters(specs: Optional[Sequence[ParameterSpec]], values: Dict[str, float]) -> List[Parameter]:
    """`Parameter`s from a mix of `Parameter`, `(name, default)` and bare names, then `name=default` keywords."""
    parameters: List[Parameter] = []
    for spec in specs or []:
        if isinstance(spec, Parameter):
            parameters.append(spec)
        elif isinstance(spec, tuple):
            name, default = spec
            parameters.append(Parameter(name=name, default_value=ParameterValue(PrefixedValue(double_value=default))))
        elif isinstance(spec, str):
            parameters.append(Parameter(name=spec))
        else:
            raise TypeError(f"parameter must be a Parameter, a name or a (name, float) tuple, got {spec!r}")
    for name, default in values.items():
        parameters.append(Parameter(name=name, default_value=ParameterValue(PrefixedValue(double_value=default))))
    return parameters


def circuit(name: str, *, top_module: str = "", properties: Optional[Dict[str, str]] = None) -> Circuit:
    return Circuit(name=name, top_module=top_module, properties=dict(properties or {}))


def module(
    name: str,
    *,
    class_name: str = "",
    parameters: Optional[Sequence[ParameterSpec]] = None,
    properties: Optional[Dict[str, str]] = None,
) -> Module:
    return Module(
        name=name,
        class_name=class_name,
        parameters=_make_parameters(parameters, {}),
        properties=dict(properties or {}),
    )


def module_reference(
    name: str,
    module_name: str,
    *,
    class_name: str = "",
    parameters: Optional[Sequence[ParameterSpec]] = None,
    properties: Optional[Dict[str, str]] = None,
    **parameter_values: float,
) -> ModuleReference:
    """An instance of `module_name`; `name=default` keywords (`value=2.0`) become parameters."""
    return ModuleReference(
        name=name,
        module_name=module_name,
        class_name=class_name,
        parameters=_make_parameters(parameters, parameter_values),
        properties=dict(properties or {}),
    )


def ext_module(
    name: str,
    *,
    class_name: str = "",
    library_name: str = "",
    group_name: str = "",
    ports: Optional[Sequence[Port]] = None,
    parameters: Optional[Sequence[ParameterSpec]] = None,
    properties: Optional[Dict[str, str]] = None,
) -> ExternalModule:
    return ExternalModule(
        name=name,
        class_name=class_name,
        library_name=library_name,
        group_name=group_name,
        ports=list(ports or []),
        parameters=_make_parameters(parameters, {}),
        properties=dict(properties or {}),
    )


def port(
    name: str,
    *,
    direction: int = PortDirection.INOUT,
    domain: int = SignalDomain.UNSPECIFIED,
    width: int = 0,
    discipline: str = "",
    properties: Optional[Dict[str, str]] = None,
) -> Port:
    return Port(
        name=name,
        direction=PortDirection(direction),
        domain=SignalDomain(domain),
        width=width,
        properties=dict(properties or {}),
        discipline=discipline,
    )


def connection(
    name: str,
    src: Tuple[str, str],
    tgt: Tuple[str, str],
    *,
    domain: int = SignalDomain.UNSPECIFIED,
    properties: Optional[Dict[str, str]] = None,
) -> Connection:
    return Connection(
        name=name,
        source=PortReference(instance_name=src[0], port_name=src[1]),
        target=PortReference(instance_name=tgt[0], port_name=tgt[1]),
        domain=SignalDomain(domain),
        properties=dict(properties or {}),
    )


def parameter(
    name: str,
    *,
    default_value: Optional[ParameterValue] = None,
    description: str = "",
    properties: Optional[Dict[str, str]] = None,
) -> Parameter:
    return Parameter(name=name, default_value=default_value, description=description, properties=dict(properties or {}))


def external_library(
    path: str, *, model_type: str = "", external_model_names: Optional[Sequence[str]] = None
) -> ExternalLibrary:
    return ExternalLibrary(path=path, model_type=model_type, external_model_names=list(external_model_names or []))


def model_interface(
    name: str,
    *,
    function_name: str = "",
    parameters: Optional[Sequence[ParameterSpec]] = None,
    properties: Optional[Dict[str, str]] = None,
) -> ModelInterface:
    return ModelInterface(
        name=name,
        function_name=function_name,
        parameters=_make_parameters(parameters, {}),
        properties=dict(properties or {}),
    )


# ---------------------------------------------------------------------------
# Parsers and writers (ValueError on failure)
# ---------------------------------------------------------------------------


def parse_spice(text: str) -> Circuit:
    return _circuit(_oxymorph.parse("spice", text))


def parse_hspice(text: str) -> Circuit:
    return _circuit(_oxymorph.parse("hspice", text))


def parse_xyce(text: str) -> Circuit:
    return _circuit(_oxymorph.parse("xyce", text))


def parse_spectre(text: str) -> Circuit:
    return _circuit(_oxymorph.parse("spectre", text))


def parse_spectre_spice(text: str) -> Circuit:
    return _circuit(_oxymorph.parse("spectre_spice", text))


def _parse_hdl(
    format_name: str,
    text: str,
    defines: Optional[Dict[str, str]],
    disciplines: Optional[Dict[str, int]],
    pin_domains: Optional[Dict[str, Dict[str, int]]],
) -> Circuit:
    domains_by_cell = None
    if pin_domains is not None:
        domains_by_cell = {
            cell: {pin: int(SignalDomain(domain)) for pin, domain in pins.items()} for cell, pins in pin_domains.items()
        }
    return _circuit(
        _oxymorph.parse_hdl(
            format_name,
            text,
            json.dumps(dict(defines or {})),
            json.dumps({name: int(SignalDomain(domain)) for name, domain in (disciplines or {}).items()}),
            json.dumps(domains_by_cell),
        )
    )


def parse_systemverilog(
    text: str,
    defines: Optional[Dict[str, str]] = None,
    disciplines: Optional[Dict[str, int]] = None,
    pin_domains: Optional[Dict[str, Dict[str, int]]] = None,
) -> Circuit:
    """`defines` drive `` `ifdef `` (none: the `` `else `` branches); `disciplines` map a net type to its domain;
    `pin_domains` (cell → pin → domain) win over them."""
    return _parse_hdl("systemverilog", text, defines, disciplines, pin_domains)


def parse_vams(
    text: str,
    defines: Optional[Dict[str, str]] = None,
    disciplines: Optional[Dict[str, int]] = None,
    pin_domains: Optional[Dict[str, Dict[str, int]]] = None,
) -> Circuit:
    """As `parse_systemverilog`, for Verilog-AMS."""
    return _parse_hdl("vams", text, defines, disciplines, pin_domains)


def _corner_list(corners: Union[str, Sequence[str]]) -> List[str]:
    return [corners] if isinstance(corners, str) else list(corners)


def _models_json(models: Optional[Dict[str, Any]]) -> Optional[str]:
    return None if models is None else json.dumps(models)


def parse_gsch(
    text: str,
    *,
    models: Optional[Dict[str, Any]] = None,
    sim: str = "ngspice",
    corners: Union[str, Sequence[str]] = (),
    name: Optional[str] = None,
) -> Circuit:
    """A `.gsch` schematic as one module (named `name`, else `__top__`), devices resolved for `sim` from the
    nyanlib `models`."""
    return _circuit(_oxymorph.parse_gsch(text, _models_json(models), sim, _corner_list(corners), name))


def read_gsch(
    path: Union[str, os.PathLike],
    *,
    project_root: Union[str, os.PathLike, None] = None,
    models: Optional[Dict[str, Any]] = None,
    sim: str = "ngspice",
    corners: Union[str, Sequence[str]] = (),
) -> Circuit:
    """A `.gsch` file and the `.gsch` schematics its devices instantiate."""
    root = None if project_root is None else os.fspath(project_root)
    return _circuit(_oxymorph.read_gsch(os.fspath(path), root, _models_json(models), sim, _corner_list(corners)))


def _write(format_name: str, circuit_to_write: Circuit) -> str:
    return _oxymorph.write(format_name, _dumps(circuit_to_write))


def write_spice(circuit: Circuit) -> str:
    return _write("spice", circuit)


def write_hspice(circuit: Circuit) -> str:
    return _write("hspice", circuit)


def write_xyce(circuit: Circuit) -> str:
    return _write("xyce", circuit)


def write_spectre(circuit: Circuit) -> str:
    return _write("spectre", circuit)


def write_spectre_spice(circuit: Circuit) -> str:
    return _write("spectre_spice", circuit)


def write_vams(circuit: Circuit) -> str:
    return _write("vams", circuit)


def write_systemverilog(circuit: Circuit) -> str:
    return _write("systemverilog", circuit)


def write_gsch(circuit: Circuit) -> str:
    return _write("gsch", circuit)


# ---------------------------------------------------------------------------
# Expressions and paths
# ---------------------------------------------------------------------------


def parse_expression(text: str) -> Expression:
    """The expression tree of `text` (one outer `{…}` or `'…'` stripped); `Expression(raw=text)` when it does not
    parse."""
    return _decode(json.loads(_oxymorph.parse_expression(text)), Expression)


def format_expression(expression: Expression, dialect: str, *, named: bool = False) -> str:
    """Canonical text of `expression` for `dialect` (ngspice, xyce, hspice, spectre, vacask, systemverilog, vams,
    gsch); `named` is a `k=v` value, where raw text is wrapped too."""
    return _oxymorph.format_expression(_dumps(expression), dialect, named)


def posix_file(path: Union[str, os.PathLike]) -> str:
    """`path` in the stored form: POSIX separators, `.`/`..` resolved lexically."""
    return _oxymorph.posix_file(os.fspath(path))


def is_absolute(path: Union[str, os.PathLike]) -> bool:
    """`/x`, `//server/share`, `C:/x`, `C:x` (either separator) on every host."""
    return _oxymorph.is_absolute(os.fspath(path))


def resolve_file(path: Union[str, os.PathLike], base: Union[str, os.PathLike, None] = None) -> str:
    """`path` joined to `base` when relative, in the stored form."""
    return _oxymorph.resolve_file(os.fspath(path), None if base is None else os.fspath(base))


__all__ = [
    "MAX_NESTING",
    # enums
    "AnalysisKind",
    "DirectiveKind",
    "ExternalModuleKind",
    "Operator",
    "OutputRequestKind",
    "PortDirection",
    "SIPrefix",
    "SignalDomain",
    "WaveformKind",
    # IR classes
    "Analysis",
    "BinaryOp",
    "Bus",
    "Call",
    "Circuit",
    "Connection",
    "Directive",
    "Expression",
    "ExternalLibrary",
    "ExternalModule",
    "InitialCondition",
    "Macro",
    "Measurement",
    "ModelInterface",
    "ModelReference",
    "Module",
    "ModuleReference",
    "OutputRequest",
    "Parameter",
    "ParameterValue",
    "Port",
    "PortReference",
    "PrefixedValue",
    "Simulation",
    "Stimulus",
    "Ternary",
    "UnaryOp",
    "Waveform",
    # builders
    "circuit",
    "connection",
    "ext_module",
    "external_library",
    "model_interface",
    "module",
    "module_reference",
    "parameter",
    "port",
    # parsers and writers
    "parse_gsch",
    "parse_hspice",
    "parse_spectre",
    "parse_spectre_spice",
    "parse_spice",
    "parse_systemverilog",
    "parse_vams",
    "parse_xyce",
    "read_gsch",
    "write_gsch",
    "write_hspice",
    "write_spectre",
    "write_spectre_spice",
    "write_spice",
    "write_systemverilog",
    "write_vams",
    "write_xyce",
    # expressions and paths
    "format_expression",
    "is_absolute",
    "parse_expression",
    "posix_file",
    "resolve_file",
]
