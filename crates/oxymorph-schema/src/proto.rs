//! Protobuf interchange: convert the IR to/from the upstream `.proto` wire
//! format (`vendor/morphnet/protos/*.proto`), from `decisions/0005`/`0006`.
//!
//! The generated types are `prost` structs; the field naming (e.g. `terminals`
//! for our `ports`) and the `ParameterValue` oneof are mapped here.

use prost::Message;

use crate::*;

/// prost-generated wire types.
pub mod wire {
    include!(concat!(env!("OUT_DIR"), "/circuit.rs"));
}

// ---------------------------------------------------------------------------
// Enum conversion (numeric values match the proto enums 1:1)
// ---------------------------------------------------------------------------

fn pdir_to(v: PortDirection) -> i32 {
    v as i32
}
fn pdir_from(v: i32) -> PortDirection {
    match v {
        1 => PortDirection::Input,
        2 => PortDirection::Output,
        _ => PortDirection::Inout,
    }
}
fn sdom_to(v: SignalDomain) -> i32 {
    v as i32
}
fn sdom_from(v: i32) -> SignalDomain {
    match v {
        1 => SignalDomain::Electrical,
        2 => SignalDomain::Waveguide,
        _ => SignalDomain::Unspecified,
    }
}
fn siprefix_from(v: i32) -> Siprefix {
    match v {
        1 => Siprefix::Quecto,
        2 => Siprefix::Ronto,
        3 => Siprefix::Yocto,
        4 => Siprefix::Zepto,
        5 => Siprefix::Atto,
        6 => Siprefix::Femto,
        7 => Siprefix::Pico,
        8 => Siprefix::Nano,
        9 => Siprefix::Micro,
        10 => Siprefix::Milli,
        11 => Siprefix::Centi,
        12 => Siprefix::Deci,
        13 => Siprefix::Deca,
        14 => Siprefix::Hecto,
        15 => Siprefix::Kilo,
        16 => Siprefix::Mega,
        17 => Siprefix::Giga,
        18 => Siprefix::Tera,
        19 => Siprefix::Peta,
        20 => Siprefix::Exa,
        21 => Siprefix::Zetta,
        22 => Siprefix::Yotta,
        23 => Siprefix::Ronna,
        24 => Siprefix::Quetta,
        _ => Siprefix::Unspecified,
    }
}
fn extkind_from(v: i32) -> ExternalModuleKind {
    match v {
        1 => ExternalModuleKind::Device,
        2 => ExternalModuleKind::Model,
        3 => ExternalModuleKind::Nature,
        4 => ExternalModuleKind::Discipline,
        _ => ExternalModuleKind::Unspecified,
    }
}
fn dkind_from(v: i32) -> DirectiveKind {
    match v {
        1 => DirectiveKind::Include,
        2 => DirectiveKind::Lib,
        3 => DirectiveKind::Global,
        4 => DirectiveKind::Option,
        5 => DirectiveKind::Param,
        6 => DirectiveKind::Define,
        7 => DirectiveKind::Timescale,
        _ => DirectiveKind::Unspecified,
    }
}
fn akind_from(v: i32) -> AnalysisKind {
    match v {
        1 => AnalysisKind::Op,
        2 => AnalysisKind::Dc,
        3 => AnalysisKind::Ac,
        4 => AnalysisKind::Tran,
        5 => AnalysisKind::Noise,
        6 => AnalysisKind::Tf,
        7 => AnalysisKind::Sens,
        8 => AnalysisKind::Pz,
        9 => AnalysisKind::Disto,
        10 => AnalysisKind::Four,
        11 => AnalysisKind::Fft,
        _ => AnalysisKind::Unspecified,
    }
}
fn okind_from(v: i32) -> OutputRequestKind {
    match v {
        1 => OutputRequestKind::Print,
        2 => OutputRequestKind::Plot,
        3 => OutputRequestKind::Probe,
        4 => OutputRequestKind::Save,
        _ => OutputRequestKind::Unspecified,
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

type PropMap = std::collections::HashMap<String, String>;

fn props_to(m: &Properties) -> PropMap {
    m.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
}

// ---------------------------------------------------------------------------
// Conversions
// ---------------------------------------------------------------------------

impl PrefixedValue {
    fn to_wire(&self) -> wire::PrefixedValue {
        wire::PrefixedValue {
            double_value: self.double_value,
            prefix: self.prefix as i32,
        }
    }
    fn from_wire(p: wire::PrefixedValue) -> Self {
        PrefixedValue {
            double_value: p.double_value,
            prefix: siprefix_from(p.prefix),
        }
    }
}

impl ParameterValue {
    fn to_wire(&self) -> wire::ParameterValue {
        let value = if let Some(pv) = &self.prefixed_value {
            Some(wire::parameter_value::Value::PrefixedValue(pv.to_wire()))
        } else {
            self.model_ref
                .as_ref()
                .map(|mr| wire::parameter_value::Value::ModelRef(mr.to_wire()))
        };
        wire::ParameterValue { value }
    }
    fn from_wire(p: wire::ParameterValue) -> Self {
        match p.value {
            Some(wire::parameter_value::Value::PrefixedValue(pv)) => ParameterValue {
                prefixed_value: Some(PrefixedValue::from_wire(pv)),
                model_ref: None,
            },
            Some(wire::parameter_value::Value::ModelRef(mr)) => ParameterValue {
                prefixed_value: None,
                model_ref: Some(ModelReference::from_wire(mr)),
            },
            None => ParameterValue::default(),
        }
    }
}

impl ModelReference {
    fn to_wire(&self) -> wire::ModelReference {
        wire::ModelReference {
            model_interface_name: self.model_interface_name.clone(),
            arguments: self
                .arguments
                .iter()
                .map(|(k, v)| (k.clone(), v.to_wire()))
                .collect(),
        }
    }
    fn from_wire(p: wire::ModelReference) -> Self {
        ModelReference {
            model_interface_name: p.model_interface_name,
            arguments: p
                .arguments
                .into_iter()
                .map(|(k, v)| (k, ParameterValue::from_wire(v)))
                .collect(),
        }
    }
}

impl Parameter {
    fn to_wire(&self) -> wire::Parameter {
        wire::Parameter {
            name: self.name.clone(),
            default_value: self.default_value.as_ref().map(ParameterValue::to_wire),
            description: self.description.clone(),
            properties: props_to(&self.properties),
        }
    }
    fn from_wire(p: wire::Parameter) -> Self {
        Parameter {
            name: p.name,
            default_value: p.default_value.map(ParameterValue::from_wire),
            description: p.description,
            properties: p.properties.into_iter().collect(),
        }
    }
}

impl Directive {
    fn to_wire(&self) -> wire::Directive {
        wire::Directive {
            kind: self.kind as i32,
            name: self.name.clone(),
            value: self.value.clone(),
        }
    }
    fn from_wire(p: wire::Directive) -> Self {
        Directive {
            kind: dkind_from(p.kind),
            name: p.name,
            value: p.value,
        }
    }
}

impl ModelInterface {
    fn to_wire(&self) -> wire::ModelInterface {
        wire::ModelInterface {
            name: self.name.clone(),
            function_name: self.function_name.clone(),
            parameters: self.parameters.iter().map(Parameter::to_wire).collect(),
            properties: props_to(&self.properties),
        }
    }
    fn from_wire(p: wire::ModelInterface) -> Self {
        ModelInterface {
            name: p.name,
            function_name: p.function_name,
            parameters: p.parameters.into_iter().map(Parameter::from_wire).collect(),
            properties: p.properties.into_iter().collect(),
        }
    }
}

impl Port {
    fn to_wire(&self) -> wire::Terminal {
        wire::Terminal {
            name: self.name.clone(),
            direction: pdir_to(self.direction),
            domain: sdom_to(self.domain),
            width: self.width,
            properties: props_to(&self.properties),
            discipline: self.discipline.clone(),
        }
    }
    fn from_wire(p: wire::Terminal) -> Self {
        Port {
            name: p.name,
            direction: pdir_from(p.direction),
            domain: sdom_from(p.domain),
            width: p.width,
            properties: p.properties.into_iter().collect(),
            discipline: p.discipline,
        }
    }
}

impl PortReference {
    fn to_wire(&self) -> wire::TerminalReference {
        wire::TerminalReference {
            instance_name: self.instance_name.clone(),
            terminal_name: self.port_name.clone(),
        }
    }
    fn from_wire(p: wire::TerminalReference) -> Self {
        PortReference {
            instance_name: p.instance_name,
            port_name: p.terminal_name,
        }
    }
}

impl Connection {
    fn to_wire(&self) -> wire::Connection {
        wire::Connection {
            name: self.name.clone(),
            source: self.source.as_ref().map(PortReference::to_wire),
            target: self.target.as_ref().map(PortReference::to_wire),
            domain: sdom_to(self.domain),
            weight: self.weight,
            properties: props_to(&self.properties),
        }
    }
    fn from_wire(p: wire::Connection) -> Self {
        Connection {
            name: p.name,
            source: p.source.map(PortReference::from_wire),
            target: p.target.map(PortReference::from_wire),
            domain: sdom_from(p.domain),
            weight: p.weight,
            properties: p.properties.into_iter().collect(),
        }
    }
}

impl Bus {
    fn to_wire(&self) -> wire::Bus {
        wire::Bus {
            name: self.name.clone(),
            width: self.width,
            domain: sdom_to(self.domain),
            connections: self.connections.iter().map(Connection::to_wire).collect(),
            properties: props_to(&self.properties),
        }
    }
    fn from_wire(p: wire::Bus) -> Self {
        Bus {
            name: p.name,
            width: p.width,
            domain: sdom_from(p.domain),
            connections: p
                .connections
                .into_iter()
                .map(Connection::from_wire)
                .collect(),
            properties: p.properties.into_iter().collect(),
        }
    }
}

impl ModuleReference {
    fn to_wire(&self) -> wire::ModuleReference {
        wire::ModuleReference {
            name: self.name.clone(),
            module_name: self.module_name.clone(),
            class_name: self.class_name.clone(),
            parameters: self.parameters.iter().map(Parameter::to_wire).collect(),
            properties: props_to(&self.properties),
        }
    }
    fn from_wire(p: wire::ModuleReference) -> Self {
        ModuleReference {
            name: p.name,
            module_name: p.module_name,
            class_name: p.class_name,
            parameters: p.parameters.into_iter().map(Parameter::from_wire).collect(),
            properties: p.properties.into_iter().collect(),
        }
    }
}

impl Module {
    fn to_wire(&self) -> wire::Module {
        wire::Module {
            name: self.name.clone(),
            class_name: self.class_name.clone(),
            terminals: self.ports.iter().map(Port::to_wire).collect(),
            parameters: self.parameters.iter().map(Parameter::to_wire).collect(),
            model_interfaces: self
                .model_interfaces
                .iter()
                .map(ModelInterface::to_wire)
                .collect(),
            module_references: self
                .module_references
                .iter()
                .map(ModuleReference::to_wire)
                .collect(),
            connections: self.connections.iter().map(Connection::to_wire).collect(),
            buses: self.buses.iter().map(Bus::to_wire).collect(),
            properties: props_to(&self.properties),
            directives: self.directives.iter().map(Directive::to_wire).collect(),
        }
    }
    fn from_wire(p: wire::Module) -> Self {
        Module {
            name: p.name,
            class_name: p.class_name,
            ports: p.terminals.into_iter().map(Port::from_wire).collect(),
            parameters: p.parameters.into_iter().map(Parameter::from_wire).collect(),
            model_interfaces: p
                .model_interfaces
                .into_iter()
                .map(ModelInterface::from_wire)
                .collect(),
            module_references: p
                .module_references
                .into_iter()
                .map(ModuleReference::from_wire)
                .collect(),
            connections: p
                .connections
                .into_iter()
                .map(Connection::from_wire)
                .collect(),
            buses: p.buses.into_iter().map(Bus::from_wire).collect(),
            properties: p.properties.into_iter().collect(),
            directives: p.directives.into_iter().map(Directive::from_wire).collect(),
        }
    }
}

impl ExternalModule {
    fn to_wire(&self) -> wire::ExternalModule {
        wire::ExternalModule {
            name: self.name.clone(),
            class_name: self.class_name.clone(),
            library_name: self.library_name.clone(),
            group_name: self.group_name.clone(),
            terminals: self.ports.iter().map(Port::to_wire).collect(),
            parameters: self.parameters.iter().map(Parameter::to_wire).collect(),
            properties: props_to(&self.properties),
            kind: self.kind as i32,
        }
    }
    fn from_wire(p: wire::ExternalModule) -> Self {
        ExternalModule {
            name: p.name,
            class_name: p.class_name,
            library_name: p.library_name,
            group_name: p.group_name,
            ports: p.terminals.into_iter().map(Port::from_wire).collect(),
            parameters: p.parameters.into_iter().map(Parameter::from_wire).collect(),
            properties: p.properties.into_iter().collect(),
            kind: extkind_from(p.kind),
        }
    }
}

impl ExternalLibrary {
    fn to_wire(&self) -> wire::ExternalLibrary {
        wire::ExternalLibrary {
            path: self.path.clone(),
            model_type: self.model_type.clone(),
            external_model_names: self.external_model_names.clone(),
        }
    }
    fn from_wire(p: wire::ExternalLibrary) -> Self {
        ExternalLibrary {
            path: p.path,
            model_type: p.model_type,
            external_model_names: p.external_model_names,
        }
    }
}

impl Analysis {
    fn to_wire(&self) -> wire::Analysis {
        wire::Analysis {
            kind: self.kind as i32,
            name: self.name.clone(),
            arguments: self.arguments.clone(),
            options: props_to(&self.options),
        }
    }
    fn from_wire(p: wire::Analysis) -> Self {
        Analysis {
            kind: akind_from(p.kind),
            name: p.name,
            arguments: p.arguments,
            options: p.options.into_iter().collect(),
        }
    }
}

impl OutputRequest {
    fn to_wire(&self) -> wire::OutputRequest {
        wire::OutputRequest {
            kind: self.kind as i32,
            analysis_type: self.analysis_type.clone(),
            variables: self.variables.clone(),
        }
    }
    fn from_wire(p: wire::OutputRequest) -> Self {
        OutputRequest {
            kind: okind_from(p.kind),
            analysis_type: p.analysis_type,
            variables: p.variables,
        }
    }
}

impl Measurement {
    fn to_wire(&self) -> wire::Measurement {
        wire::Measurement {
            name: self.name.clone(),
            analysis_type: self.analysis_type.clone(),
            body: self.body.clone(),
        }
    }
    fn from_wire(p: wire::Measurement) -> Self {
        Measurement {
            name: p.name,
            analysis_type: p.analysis_type,
            body: p.body,
        }
    }
}

impl InitialCondition {
    fn to_wire(&self) -> wire::InitialCondition {
        wire::InitialCondition {
            conditions: props_to(&self.conditions),
        }
    }
    fn from_wire(p: wire::InitialCondition) -> Self {
        InitialCondition {
            conditions: p.conditions.into_iter().collect(),
        }
    }
}

impl Simulation {
    fn to_wire(&self) -> wire::Simulation {
        wire::Simulation {
            analyses: self.analyses.iter().map(Analysis::to_wire).collect(),
            output_requests: self
                .output_requests
                .iter()
                .map(OutputRequest::to_wire)
                .collect(),
            measurements: self.measurements.iter().map(Measurement::to_wire).collect(),
            initial_conditions: self
                .initial_conditions
                .as_ref()
                .map(InitialCondition::to_wire),
            node_sets: self.node_sets.as_ref().map(InitialCondition::to_wire),
            temperatures: self.temperatures.clone(),
            options: props_to(&self.options),
        }
    }
    fn from_wire(p: wire::Simulation) -> Self {
        Simulation {
            analyses: p.analyses.into_iter().map(Analysis::from_wire).collect(),
            output_requests: p
                .output_requests
                .into_iter()
                .map(OutputRequest::from_wire)
                .collect(),
            measurements: p
                .measurements
                .into_iter()
                .map(Measurement::from_wire)
                .collect(),
            initial_conditions: p.initial_conditions.map(InitialCondition::from_wire),
            node_sets: p.node_sets.map(InitialCondition::from_wire),
            temperatures: p.temperatures,
            options: p.options.into_iter().collect(),
        }
    }
}

impl Circuit {
    fn to_wire(&self) -> wire::Circuit {
        wire::Circuit {
            name: self.name.clone(),
            top_module: self.top_module.clone(),
            modules: self.modules.iter().map(Module::to_wire).collect(),
            ext_modules: self
                .ext_modules
                .iter()
                .map(ExternalModule::to_wire)
                .collect(),
            properties: props_to(&self.properties),
            directives: self.directives.iter().map(Directive::to_wire).collect(),
            simulation: self.simulation.as_ref().map(Simulation::to_wire),
            libraries: self
                .libraries
                .iter()
                .map(|(k, v)| (k.clone(), v.to_wire()))
                .collect(),
        }
    }
    fn from_wire(p: wire::Circuit) -> Self {
        Circuit {
            name: p.name,
            top_module: p.top_module,
            modules: p.modules.into_iter().map(Module::from_wire).collect(),
            ext_modules: p
                .ext_modules
                .into_iter()
                .map(ExternalModule::from_wire)
                .collect(),
            properties: p.properties.into_iter().collect(),
            directives: p.directives.into_iter().map(Directive::from_wire).collect(),
            simulation: p.simulation.map(Simulation::from_wire),
            libraries: p
                .libraries
                .into_iter()
                .map(|(k, v)| (k, ExternalLibrary::from_wire(v)))
                .collect(),
        }
    }

    /// Serialize to the upstream protobuf wire format.
    pub fn to_proto(&self) -> wire::Circuit {
        self.to_wire()
    }

    /// Serialize to upstream protobuf bytes (`ProtoModel.to_proto_bytes`).
    pub fn to_proto_bytes(&self) -> Vec<u8> {
        self.to_wire().encode_to_vec()
    }

    /// Deserialize from upstream protobuf bytes (`ProtoModel.from_proto_bytes`).
    pub fn from_proto_bytes(bytes: &[u8]) -> Result<Self, String> {
        let msg = wire::Circuit::decode(bytes).map_err(|e| e.to_string())?;
        Ok(Circuit::from_wire(msg))
    }
}
