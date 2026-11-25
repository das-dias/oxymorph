//! Python bindings for oxymorph (`oxymorph-python`).
//!
//! Thin PyO3 layer over `oxymorph-schema` and `oxymorph-netlist`: no logic, only
//! conversion and the module surface. The Python module is named `oxymorph` and
//! exposes the same functionality as upstream `morphnet` (not API-compatible).

use std::collections::HashMap;

use indexmap::IndexMap;
use oxymorph_schema as xs;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict};

fn convert_params(objs: Option<Vec<Bound<'_, PyAny>>>) -> PyResult<Vec<xs::Parameter>> {
    let mut out: Vec<xs::Parameter> = Vec::new();
    if let Some(list) = objs {
        for o in list {
            if let Ok(p) = o.extract::<PyRef<'_, Parameter>>() {
                out.push(p.0.clone());
            } else if let Ok((n, v)) = o.extract::<(String, f64)>() {
                out.push(xs::Parameter {
                    name: n,
                    default_value: Some(xs::ParameterValue::prefixed(v, xs::Siprefix::Unspecified)),
                    ..Default::default()
                });
            } else if let Ok(s) = o.extract::<String>() {
                out.push(xs::Parameter {
                    name: s,
                    ..Default::default()
                });
            } else {
                return Err(PyValueError::new_err(
                    "parameter must be a Parameter, a name, or a (name, float) tuple",
                ));
            }
        }
    }
    Ok(out)
}

fn props(m: Option<HashMap<String, String>>) -> xs::Properties {
    m.map(|m| m.into_iter().collect()).unwrap_or_default()
}

fn to_pydict(py: Python<'_>, m: &xs::Properties) -> PyResult<Py<PyDict>> {
    let d = PyDict::new(py);
    for (k, v) in m {
        d.set_item(k, v)?;
    }
    Ok(d.into())
}

fn pdir(v: i32) -> xs::PortDirection {
    match v {
        1 => xs::PortDirection::Input,
        2 => xs::PortDirection::Output,
        _ => xs::PortDirection::Inout,
    }
}
fn pdir_i(d: xs::PortDirection) -> i32 {
    d as i32
}
fn sdom(v: i32) -> xs::SignalDomain {
    match v {
        1 => xs::SignalDomain::Electrical,
        2 => xs::SignalDomain::Waveguide,
        _ => xs::SignalDomain::Unspecified,
    }
}
fn sdom_i(d: xs::SignalDomain) -> i32 {
    d as i32
}

// ==========================================================================
// Enums (integer constants, like upstream IntEnum)
// ==========================================================================

#[pyclass(module = "oxymorph", eq)]
#[derive(Clone, Copy, PartialEq)]
pub struct PortDirection;
#[pymethods]
impl PortDirection {
    #[classattr]
    const INOUT: i32 = 0;
    #[classattr]
    const INPUT: i32 = 1;
    #[classattr]
    const OUTPUT: i32 = 2;
}

#[pyclass(module = "oxymorph", eq)]
#[derive(Clone, Copy, PartialEq)]
pub struct SignalDomain;
#[pymethods]
impl SignalDomain {
    #[classattr]
    const UNSPECIFIED: i32 = 0;
    #[classattr]
    const ELECTRICAL: i32 = 1;
    #[classattr]
    const WAVEGUIDE: i32 = 2;
}

// ==========================================================================
// Parameter models
// ==========================================================================

#[pyclass(module = "oxymorph")]
#[derive(Clone)]
pub struct PrefixedValue(pub xs::PrefixedValue);

#[pymethods]
impl PrefixedValue {
    #[new]
    #[pyo3(signature = (double_value = 0.0, prefix = 0))]
    fn new(double_value: f64, prefix: i32) -> Self {
        PrefixedValue(xs::PrefixedValue {
            double_value,
            prefix: siprefix(prefix),
        })
    }
    #[getter]
    fn double_value(&self) -> f64 {
        self.0.double_value
    }
    #[getter]
    fn prefix(&self) -> i32 {
        self.0.prefix as i32
    }
}

fn siprefix(v: i32) -> xs::Siprefix {
    // Values match the proto enum 1:1.
    match v {
        1 => xs::Siprefix::Quecto,
        2 => xs::Siprefix::Ronto,
        3 => xs::Siprefix::Yocto,
        4 => xs::Siprefix::Zepto,
        5 => xs::Siprefix::Atto,
        6 => xs::Siprefix::Femto,
        7 => xs::Siprefix::Pico,
        8 => xs::Siprefix::Nano,
        9 => xs::Siprefix::Micro,
        10 => xs::Siprefix::Milli,
        11 => xs::Siprefix::Centi,
        12 => xs::Siprefix::Deci,
        13 => xs::Siprefix::Deca,
        14 => xs::Siprefix::Hecto,
        15 => xs::Siprefix::Kilo,
        16 => xs::Siprefix::Mega,
        17 => xs::Siprefix::Giga,
        18 => xs::Siprefix::Tera,
        19 => xs::Siprefix::Peta,
        20 => xs::Siprefix::Exa,
        21 => xs::Siprefix::Zetta,
        22 => xs::Siprefix::Yotta,
        23 => xs::Siprefix::Ronna,
        24 => xs::Siprefix::Quetta,
        _ => xs::Siprefix::Unspecified,
    }
}

#[pyclass(module = "oxymorph")]
#[derive(Clone)]
pub struct ParameterValue(pub xs::ParameterValue);

#[pymethods]
impl ParameterValue {
    #[new]
    #[pyo3(signature = (prefixed_value = None, model_ref = None))]
    fn new(
        prefixed_value: Option<Bound<'_, PrefixedValue>>,
        model_ref: Option<Bound<'_, ModelReference>>,
    ) -> Self {
        ParameterValue(xs::ParameterValue {
            prefixed_value: prefixed_value.map(|p| p.borrow().0.clone()),
            model_ref: model_ref.map(|m| m.borrow().0.clone()),
            ..Default::default()
        })
    }
    #[getter]
    fn prefixed_value(&self) -> Option<PrefixedValue> {
        self.0.prefixed_value.clone().map(PrefixedValue)
    }
    #[getter]
    fn model_ref(&self) -> Option<ModelReference> {
        self.0.model_ref.clone().map(ModelReference)
    }
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.0).map_err(|e| PyValueError::new_err(e.to_string()))
    }
}

#[pyclass(module = "oxymorph")]
#[derive(Clone)]
pub struct Parameter(pub xs::Parameter);

#[pymethods]
impl Parameter {
    #[new]
    #[pyo3(signature = (name = String::new(), default_value = None, description = String::new(), properties = None))]
    fn new(
        name: String,
        default_value: Option<Bound<'_, ParameterValue>>,
        description: String,
        properties: Option<HashMap<String, String>>,
    ) -> Self {
        Parameter(xs::Parameter {
            name,
            default_value: default_value.map(|d| d.borrow().0.clone()),
            description,
            properties: props(properties),
        })
    }
    #[getter]
    fn name(&self) -> &str {
        &self.0.name
    }
    #[getter]
    fn default_value(&self) -> Option<ParameterValue> {
        self.0.default_value.clone().map(ParameterValue)
    }
    #[getter]
    fn description(&self) -> &str {
        &self.0.description
    }
}

#[pyclass(module = "oxymorph")]
#[derive(Clone)]
pub struct ModelReference(pub xs::ModelReference);

#[pymethods]
impl ModelReference {
    #[new]
    #[pyo3(signature = (model_interface_name = String::new(), arguments = None))]
    fn new(model_interface_name: String, arguments: Option<Bound<'_, PyDict>>) -> PyResult<Self> {
        let mut args = IndexMap::new();
        if let Some(a) = arguments {
            for (k, v) in a.iter() {
                let key: String = k.extract()?;
                let val: Bound<'_, ParameterValue> = v.extract()?;
                args.insert(key, val.borrow().0.clone());
            }
        }
        Ok(ModelReference(xs::ModelReference {
            model_interface_name,
            arguments: args,
        }))
    }
    #[getter]
    fn model_interface_name(&self) -> &str {
        &self.0.model_interface_name
    }
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.0).map_err(|e| PyValueError::new_err(e.to_string()))
    }
}

// ==========================================================================
// Directives / interfaces
// ==========================================================================

#[pyclass(module = "oxymorph")]
#[derive(Clone)]
pub struct Directive(pub xs::Directive);

#[pymethods]
impl Directive {
    #[new]
    #[pyo3(signature = (kind = 0, name = String::new(), value = String::new()))]
    fn new(kind: i32, name: String, value: String) -> Self {
        Directive(xs::Directive {
            kind: directive_kind(kind),
            name,
            value,
        })
    }
    #[getter]
    fn kind(&self) -> i32 {
        self.0.kind as i32
    }
    #[getter]
    fn name(&self) -> &str {
        &self.0.name
    }
    #[getter]
    fn value(&self) -> &str {
        &self.0.value
    }
}

fn directive_kind(v: i32) -> xs::DirectiveKind {
    match v {
        1 => xs::DirectiveKind::Include,
        2 => xs::DirectiveKind::Lib,
        3 => xs::DirectiveKind::Global,
        4 => xs::DirectiveKind::Option,
        5 => xs::DirectiveKind::Param,
        6 => xs::DirectiveKind::Define,
        7 => xs::DirectiveKind::Timescale,
        _ => xs::DirectiveKind::Unspecified,
    }
}

#[pyclass(module = "oxymorph")]
#[derive(Clone)]
pub struct ModelInterface(pub xs::ModelInterface);

#[pymethods]
impl ModelInterface {
    #[new]
    #[pyo3(signature = (name = String::new(), function_name = String::new(), parameters = None, properties = None))]
    fn new(
        name: String,
        function_name: String,
        parameters: Option<Vec<Bound<'_, Parameter>>>,
        properties: Option<HashMap<String, String>>,
    ) -> Self {
        ModelInterface(xs::ModelInterface {
            name,
            function_name,
            parameters: parameters
                .map(|p| p.iter().map(|b| b.borrow().0.clone()).collect())
                .unwrap_or_default(),
            properties: props(properties),
        })
    }
    #[getter]
    fn name(&self) -> &str {
        &self.0.name
    }
}

// ==========================================================================
// Ports / connections
// ==========================================================================

#[pyclass(module = "oxymorph")]
#[derive(Clone)]
pub struct PortReference(pub xs::PortReference);

#[pymethods]
impl PortReference {
    #[new]
    #[pyo3(signature = (instance_name = String::new(), port_name = String::new()))]
    fn new(instance_name: String, port_name: String) -> Self {
        PortReference(xs::PortReference {
            instance_name,
            port_name,
        })
    }
    #[getter]
    fn instance_name(&self) -> &str {
        &self.0.instance_name
    }
    #[getter]
    fn port_name(&self) -> &str {
        &self.0.port_name
    }
}

#[pyclass(module = "oxymorph")]
#[derive(Clone)]
pub struct Port(pub xs::Port);

#[pymethods]
impl Port {
    #[new]
    #[pyo3(signature = (name, direction = 0, domain = 0, width = 0, discipline = String::new(), properties = None))]
    fn new(
        name: String,
        direction: i32,
        domain: i32,
        width: u32,
        discipline: String,
        properties: Option<HashMap<String, String>>,
    ) -> Self {
        Port(xs::Port {
            name,
            direction: pdir(direction),
            domain: sdom(domain),
            width,
            discipline,
            properties: props(properties),
        })
    }
    #[getter]
    fn name(&self) -> &str {
        &self.0.name
    }
    #[getter]
    fn direction(&self) -> i32 {
        pdir_i(self.0.direction)
    }
    #[getter]
    fn domain(&self) -> i32 {
        sdom_i(self.0.domain)
    }
    #[getter]
    fn width(&self) -> u32 {
        self.0.width
    }
    #[getter]
    fn discipline(&self) -> &str {
        &self.0.discipline
    }
}

#[pyclass(module = "oxymorph")]
#[derive(Clone)]
pub struct Connection(pub xs::Connection);

#[pymethods]
impl Connection {
    #[new]
    #[pyo3(signature = (name, source = None, target = None, domain = 0, properties = None))]
    fn new(
        name: String,
        source: Option<Bound<'_, PortReference>>,
        target: Option<Bound<'_, PortReference>>,
        domain: i32,
        properties: Option<HashMap<String, String>>,
    ) -> Self {
        Connection(xs::Connection {
            name,
            source: source.map(|s| s.borrow().0.clone()),
            target: target.map(|t| t.borrow().0.clone()),
            domain: sdom(domain),
            weight: 0,
            properties: props(properties),
        })
    }
    #[getter]
    fn name(&self) -> &str {
        &self.0.name
    }
}

// ==========================================================================
// Modules
// ==========================================================================

#[pyclass(module = "oxymorph")]
#[derive(Clone)]
pub struct ModuleReference(pub xs::ModuleReference);

#[pymethods]
impl ModuleReference {
    #[new]
    #[pyo3(signature = (name, module_name, class_name = String::new(), parameters = None, properties = None))]
    fn new(
        name: String,
        module_name: String,
        class_name: String,
        parameters: Option<Vec<Bound<'_, PyAny>>>,
        properties: Option<HashMap<String, String>>,
    ) -> PyResult<Self> {
        Ok(ModuleReference(xs::ModuleReference {
            name,
            module_name,
            class_name,
            parameters: convert_params(parameters)?,
            properties: props(properties),
        }))
    }
    #[getter]
    fn name(&self) -> &str {
        &self.0.name
    }
    #[getter]
    fn module_name(&self) -> &str {
        &self.0.module_name
    }
    #[getter]
    fn parameters(&self) -> Vec<Parameter> {
        self.0.parameters.iter().cloned().map(Parameter).collect()
    }
}

#[pyclass(module = "oxymorph")]
#[derive(Clone)]
pub struct Module(pub xs::Module);

#[pymethods]
impl Module {
    #[new]
    #[pyo3(signature = (name, class_name = String::new(), parameters = None, properties = None))]
    fn new(
        name: String,
        class_name: String,
        parameters: Option<Vec<Bound<'_, PyAny>>>,
        properties: Option<HashMap<String, String>>,
    ) -> PyResult<Self> {
        Ok(Module(xs::Module {
            name,
            class_name,
            parameters: convert_params(parameters)?,
            properties: props(properties),
            ..Default::default()
        }))
    }

    #[pyo3(signature = (name, direction = 0, domain = 0, width = 0, discipline = String::new(), properties = None))]
    fn add_terminal(
        mut slf: PyRefMut<'_, Self>,
        name: String,
        direction: i32,
        domain: i32,
        width: u32,
        discipline: String,
        properties: Option<HashMap<String, String>>,
    ) -> PyRefMut<'_, Self> {
        slf.0.ports.push(xs::Port {
            name,
            direction: pdir(direction),
            domain: sdom(domain),
            width,
            discipline,
            properties: props(properties),
        });
        slf
    }

    #[pyo3(signature = (*refs))]
    fn add<'py>(
        mut slf: PyRefMut<'py, Self>,
        refs: Vec<Bound<'py, ModuleReference>>,
    ) -> PyRefMut<'py, Self> {
        for r in refs {
            slf.0.module_references.push(r.borrow().0.clone());
        }
        slf
    }

    #[pyo3(signature = (net_name, *endpoints, domain = 0, properties = None))]
    fn connect(
        mut slf: PyRefMut<'_, Self>,
        net_name: String,
        endpoints: Vec<(String, String)>,
        domain: i32,
        properties: Option<HashMap<String, String>>,
    ) -> PyResult<PyRefMut<'_, Self>> {
        if endpoints.len() < 2 {
            return Err(PyValueError::new_err(format!(
                "connect({net_name:?}): need >= 2 endpoints, got {}",
                endpoints.len()
            )));
        }
        let p = props(properties);
        for w in endpoints.windows(2) {
            slf.0.connections.push(xs::Connection {
                name: net_name.clone(),
                source: Some(xs::PortReference {
                    instance_name: w[0].0.clone(),
                    port_name: w[0].1.clone(),
                }),
                target: Some(xs::PortReference {
                    instance_name: w[1].0.clone(),
                    port_name: w[1].1.clone(),
                }),
                domain: sdom(domain),
                weight: 0,
                properties: p.clone(),
            });
        }
        Ok(slf)
    }

    #[getter]
    fn name(&self) -> &str {
        &self.0.name
    }
    #[getter]
    fn class_name(&self) -> &str {
        &self.0.class_name
    }
    #[getter]
    fn ports(&self) -> Vec<Port> {
        self.0.ports.iter().cloned().map(Port).collect()
    }
    #[getter]
    fn parameters(&self) -> Vec<Parameter> {
        self.0.parameters.iter().cloned().map(Parameter).collect()
    }
    #[getter]
    fn module_references(&self) -> Vec<ModuleReference> {
        self.0
            .module_references
            .iter()
            .cloned()
            .map(ModuleReference)
            .collect()
    }
    #[getter]
    fn connections(&self) -> Vec<Connection> {
        self.0.connections.iter().cloned().map(Connection).collect()
    }
    #[getter]
    fn buses(&self) -> Vec<Bus> {
        self.0.buses.iter().cloned().map(Bus).collect()
    }
}

#[pyclass(module = "oxymorph")]
#[derive(Clone)]
pub struct ExternalModule(pub xs::ExternalModule);

#[pymethods]
impl ExternalModule {
    #[new]
    #[pyo3(signature = (name, class_name = String::new(), library_name = String::new(), group_name = String::new(), ports = None, parameters = None, properties = None))]
    fn new(
        name: String,
        class_name: String,
        library_name: String,
        group_name: String,
        ports: Option<Vec<Bound<'_, Port>>>,
        parameters: Option<Vec<Bound<'_, PyAny>>>,
        properties: Option<HashMap<String, String>>,
    ) -> PyResult<Self> {
        Ok(ExternalModule(xs::ExternalModule {
            name,
            class_name,
            library_name,
            group_name,
            ports: ports
                .map(|p| p.iter().map(|b| b.borrow().0.clone()).collect())
                .unwrap_or_default(),
            parameters: convert_params(parameters)?,
            properties: props(properties),
            ..Default::default()
        }))
    }
    #[getter]
    fn name(&self) -> &str {
        &self.0.name
    }
}

#[pyclass(module = "oxymorph")]
#[derive(Clone)]
pub struct ExternalLibrary(pub xs::ExternalLibrary);

#[pymethods]
impl ExternalLibrary {
    #[new]
    #[pyo3(signature = (path, model_type = String::new(), external_model_names = None))]
    fn new(path: String, model_type: String, external_model_names: Option<Vec<String>>) -> Self {
        ExternalLibrary(xs::ExternalLibrary {
            path,
            model_type,
            external_model_names: external_model_names.unwrap_or_default(),
        })
    }
}

// ==========================================================================
// Circuit
// ==========================================================================

#[pyclass(module = "oxymorph")]
#[derive(Clone)]
pub struct Circuit(pub xs::Circuit);

#[pymethods]
impl Circuit {
    #[new]
    #[pyo3(signature = (name = String::new(), top_module = String::new(), properties = None))]
    fn new(name: String, top_module: String, properties: Option<HashMap<String, String>>) -> Self {
        Circuit(xs::Circuit {
            name,
            top_module,
            properties: props(properties),
            ..Default::default()
        })
    }

    #[pyo3(signature = (*modules))]
    fn add<'py>(
        mut slf: PyRefMut<'py, Self>,
        modules: Vec<Bound<'py, Module>>,
    ) -> PyRefMut<'py, Self> {
        for m in modules {
            slf.0.add([m.borrow().0.clone()]);
        }
        slf
    }

    #[pyo3(signature = (*ext))]
    fn add_ext<'py>(
        mut slf: PyRefMut<'py, Self>,
        ext: Vec<Bound<'py, ExternalModule>>,
    ) -> PyRefMut<'py, Self> {
        for e in ext {
            slf.0.ext_modules.push(e.borrow().0.clone());
        }
        slf
    }

    fn add_library(mut slf: PyRefMut<'_, Self>, name: String, lib: Bound<'_, ExternalLibrary>) {
        slf.0.libraries.insert(name, lib.borrow().0.clone());
    }

    fn to_json(&self) -> PyResult<String> {
        self.0
            .to_json()
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }
    fn to_json_pretty(&self) -> PyResult<String> {
        self.0
            .to_json_pretty()
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }
    fn to_yaml(&self) -> PyResult<String> {
        self.0
            .to_yaml()
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }
    fn to_proto_bytes<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.0.to_proto_bytes())
    }
    #[staticmethod]
    fn from_proto_bytes(data: &[u8]) -> PyResult<Self> {
        xs::Circuit::from_proto_bytes(data)
            .map(Circuit)
            .map_err(PyValueError::new_err)
    }
    #[staticmethod]
    fn from_json(s: &str) -> PyResult<Self> {
        xs::Circuit::from_json(s)
            .map(Circuit)
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }
    #[staticmethod]
    fn from_yaml(s: &str) -> PyResult<Self> {
        xs::Circuit::from_yaml(s)
            .map(Circuit)
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }

    #[getter]
    fn name(&self) -> &str {
        &self.0.name
    }
    #[getter]
    fn top_module(&self) -> &str {
        &self.0.top_module
    }
    #[getter]
    fn modules(&self) -> Vec<Module> {
        self.0.modules.iter().cloned().map(Module).collect()
    }
    #[getter]
    fn ext_modules(&self) -> Vec<ExternalModule> {
        self.0
            .ext_modules
            .iter()
            .cloned()
            .map(ExternalModule)
            .collect()
    }
    #[getter]
    fn directives(&self) -> Vec<Directive> {
        self.0.directives.iter().cloned().map(Directive).collect()
    }
    #[getter]
    fn simulation(&self) -> Option<Simulation> {
        self.0.simulation.clone().map(Simulation)
    }
    #[getter]
    fn properties(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        to_pydict(py, &self.0.properties)
    }

    fn __eq__(&self, other: &Circuit) -> bool {
        self.0 == other.0
    }
}

// ==========================================================================
// Simulation types
// ==========================================================================

#[pyclass(module = "oxymorph")]
#[derive(Clone)]
pub struct InitialCondition(pub xs::InitialCondition);

#[pymethods]
impl InitialCondition {
    #[new]
    #[pyo3(signature = (conditions = None))]
    fn new(conditions: Option<HashMap<String, String>>) -> Self {
        InitialCondition(xs::InitialCondition {
            conditions: props(conditions),
        })
    }
    #[getter]
    fn conditions(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        to_pydict(py, &self.0.conditions)
    }
}

#[pyclass(module = "oxymorph")]
#[derive(Clone)]
pub struct Measurement(pub xs::Measurement);

#[pymethods]
impl Measurement {
    #[new]
    #[pyo3(signature = (name = String::new(), analysis_type = String::new(), body = String::new()))]
    fn new(name: String, analysis_type: String, body: String) -> Self {
        Measurement(xs::Measurement {
            name,
            analysis_type,
            body,
        })
    }
    #[getter]
    fn name(&self) -> &str {
        &self.0.name
    }
    #[getter]
    fn analysis_type(&self) -> &str {
        &self.0.analysis_type
    }
    #[getter]
    fn body(&self) -> &str {
        &self.0.body
    }
}

#[pyclass(module = "oxymorph")]
#[derive(Clone)]
pub struct OutputRequest(pub xs::OutputRequest);

#[pymethods]
impl OutputRequest {
    #[new]
    #[pyo3(signature = (kind = 0, analysis_type = String::new(), variables = None))]
    fn new(kind: i32, analysis_type: String, variables: Option<Vec<String>>) -> Self {
        OutputRequest(xs::OutputRequest {
            kind: okind(kind),
            analysis_type,
            variables: variables.unwrap_or_default(),
        })
    }
    #[getter]
    fn kind(&self) -> i32 {
        self.0.kind as i32
    }
    #[getter]
    fn analysis_type(&self) -> &str {
        &self.0.analysis_type
    }
    #[getter]
    fn variables(&self) -> Vec<String> {
        self.0.variables.clone()
    }
}

fn akind(v: i32) -> xs::AnalysisKind {
    match v {
        1 => xs::AnalysisKind::Op,
        2 => xs::AnalysisKind::Dc,
        3 => xs::AnalysisKind::Ac,
        4 => xs::AnalysisKind::Tran,
        5 => xs::AnalysisKind::Noise,
        6 => xs::AnalysisKind::Tf,
        7 => xs::AnalysisKind::Sens,
        8 => xs::AnalysisKind::Pz,
        9 => xs::AnalysisKind::Disto,
        10 => xs::AnalysisKind::Four,
        11 => xs::AnalysisKind::Fft,
        _ => xs::AnalysisKind::Unspecified,
    }
}
fn okind(v: i32) -> xs::OutputRequestKind {
    match v {
        1 => xs::OutputRequestKind::Print,
        2 => xs::OutputRequestKind::Plot,
        3 => xs::OutputRequestKind::Probe,
        4 => xs::OutputRequestKind::Save,
        _ => xs::OutputRequestKind::Unspecified,
    }
}

#[pyclass(module = "oxymorph")]
#[derive(Clone)]
pub struct Analysis(pub xs::Analysis);

#[pymethods]
impl Analysis {
    #[new]
    #[pyo3(signature = (kind = 0, name = String::new(), arguments = None, options = None))]
    fn new(
        kind: i32,
        name: String,
        arguments: Option<Vec<String>>,
        options: Option<HashMap<String, String>>,
    ) -> Self {
        Analysis(xs::Analysis {
            kind: akind(kind),
            name,
            arguments: arguments.unwrap_or_default(),
            options: props(options),
        })
    }
    #[getter]
    fn kind(&self) -> i32 {
        self.0.kind as i32
    }
    #[getter]
    fn name(&self) -> &str {
        &self.0.name
    }
    #[getter]
    fn arguments(&self) -> Vec<String> {
        self.0.arguments.clone()
    }
}

#[pyclass(module = "oxymorph")]
#[derive(Clone)]
pub struct Simulation(pub xs::Simulation);

#[pymethods]
impl Simulation {
    #[new]
    #[pyo3(signature = ())]
    fn new() -> Self {
        Simulation(xs::Simulation::default())
    }
    #[getter]
    fn analyses(&self) -> Vec<Analysis> {
        self.0.analyses.iter().cloned().map(Analysis).collect()
    }
    #[getter]
    fn output_requests(&self) -> Vec<OutputRequest> {
        self.0
            .output_requests
            .iter()
            .cloned()
            .map(OutputRequest)
            .collect()
    }
    #[getter]
    fn measurements(&self) -> Vec<Measurement> {
        self.0
            .measurements
            .iter()
            .cloned()
            .map(Measurement)
            .collect()
    }
    #[getter]
    fn initial_conditions(&self) -> Option<InitialCondition> {
        self.0.initial_conditions.clone().map(InitialCondition)
    }
    #[getter]
    fn node_sets(&self) -> Option<InitialCondition> {
        self.0.node_sets.clone().map(InitialCondition)
    }
    #[getter]
    fn temperatures(&self) -> Vec<f64> {
        self.0.temperatures.clone()
    }
}

#[pyclass(module = "oxymorph")]
#[derive(Clone)]
pub struct Bus(pub xs::Bus);

#[pymethods]
impl Bus {
    #[new]
    #[pyo3(signature = (name, width = 0, domain = 0, properties = None))]
    fn new(
        name: String,
        width: u32,
        domain: i32,
        properties: Option<HashMap<String, String>>,
    ) -> Self {
        Bus(xs::Bus {
            name,
            width,
            domain: sdom(domain),
            properties: props(properties),
            ..Default::default()
        })
    }
    #[getter]
    fn name(&self) -> &str {
        &self.0.name
    }
    #[getter]
    fn width(&self) -> u32 {
        self.0.width
    }
    #[getter]
    fn connections(&self) -> Vec<Connection> {
        self.0.connections.iter().cloned().map(Connection).collect()
    }
}

// ==========================================================================
// Builder functions (mirrors morphnet/_api.py)
// ==========================================================================

#[pyfunction]
#[pyo3(signature = (name, *, top_module = String::new(), properties = None))]
fn circuit(
    name: String,
    top_module: String,
    properties: Option<HashMap<String, String>>,
) -> Circuit {
    Circuit(xs::Circuit {
        name,
        top_module,
        properties: props(properties),
        ..Default::default()
    })
}

#[pyfunction]
#[pyo3(signature = (name, *, class_name = String::new(), parameters = None, properties = None))]
fn module(
    name: String,
    class_name: String,
    parameters: Option<Vec<Bound<'_, PyAny>>>,
    properties: Option<HashMap<String, String>>,
) -> PyResult<Module> {
    Ok(Module(xs::Module {
        name,
        class_name,
        parameters: convert_params(parameters)?,
        properties: props(properties),
        ..Default::default()
    }))
}

#[pyfunction]
#[pyo3(signature = (name, module_name, *, class_name = String::new(), parameters = None, properties = None, **kwargs))]
fn module_reference(
    name: String,
    module_name: String,
    class_name: String,
    parameters: Option<Vec<Bound<'_, PyAny>>>,
    properties: Option<HashMap<String, String>>,
    kwargs: Option<Bound<'_, PyDict>>,
) -> PyResult<ModuleReference> {
    let mut params = convert_params(parameters)?;
    if let Some(kw) = kwargs {
        for (k, v) in kw.iter() {
            let key: String = k.extract()?;
            let val: f64 = v.extract()?;
            params.push(xs::Parameter {
                name: key,
                default_value: Some(xs::ParameterValue::prefixed(val, xs::Siprefix::Unspecified)),
                ..Default::default()
            });
        }
    }
    Ok(ModuleReference(xs::ModuleReference {
        name,
        module_name,
        class_name,
        parameters: params,
        properties: props(properties),
    }))
}

#[pyfunction]
#[pyo3(signature = (name, *, class_name = String::new(), library_name = String::new(), group_name = String::new(), ports = None, parameters = None, properties = None))]
fn ext_module(
    name: String,
    class_name: String,
    library_name: String,
    group_name: String,
    ports: Option<Vec<Bound<'_, Port>>>,
    parameters: Option<Vec<Bound<'_, PyAny>>>,
    properties: Option<HashMap<String, String>>,
) -> PyResult<ExternalModule> {
    Ok(ExternalModule(xs::ExternalModule {
        name,
        class_name,
        library_name,
        group_name,
        ports: ports
            .map(|p| p.iter().map(|b| b.borrow().0.clone()).collect())
            .unwrap_or_default(),
        parameters: convert_params(parameters)?,
        properties: props(properties),
        ..Default::default()
    }))
}

#[pyfunction]
#[pyo3(signature = (name, *, direction = 0, domain = 0, width = 0, discipline = String::new(), properties = None))]
fn port(
    name: String,
    direction: i32,
    domain: i32,
    width: u32,
    discipline: String,
    properties: Option<HashMap<String, String>>,
) -> Port {
    Port(xs::Port {
        name,
        direction: pdir(direction),
        domain: sdom(domain),
        width,
        discipline,
        properties: props(properties),
    })
}

#[pyfunction]
#[pyo3(signature = (name, src, tgt, *, domain = 0, properties = None))]
fn connection(
    name: String,
    src: (String, String),
    tgt: (String, String),
    domain: i32,
    properties: Option<HashMap<String, String>>,
) -> Connection {
    Connection(xs::Connection {
        name,
        source: Some(xs::PortReference {
            instance_name: src.0,
            port_name: src.1,
        }),
        target: Some(xs::PortReference {
            instance_name: tgt.0,
            port_name: tgt.1,
        }),
        domain: sdom(domain),
        weight: 0,
        properties: props(properties),
    })
}

#[pyfunction]
#[pyo3(signature = (name, *, default_value = None, description = String::new(), properties = None))]
fn parameter(
    name: String,
    default_value: Option<Bound<'_, ParameterValue>>,
    description: String,
    properties: Option<HashMap<String, String>>,
) -> Parameter {
    Parameter(xs::Parameter {
        name,
        default_value: default_value.map(|d| d.borrow().0.clone()),
        description,
        properties: props(properties),
    })
}

#[pyfunction]
#[pyo3(signature = (path, *, model_type = String::new(), external_model_names = None))]
fn external_library(
    path: String,
    model_type: String,
    external_model_names: Option<Vec<String>>,
) -> ExternalLibrary {
    ExternalLibrary(xs::ExternalLibrary {
        path,
        model_type,
        external_model_names: external_model_names.unwrap_or_default(),
    })
}

#[pyfunction]
#[pyo3(signature = (name, *, function_name = String::new(), parameters = None, properties = None))]
fn model_interface(
    name: String,
    function_name: String,
    parameters: Option<Vec<Bound<'_, PyAny>>>,
    properties: Option<HashMap<String, String>>,
) -> PyResult<ModelInterface> {
    Ok(ModelInterface(xs::ModelInterface {
        name,
        function_name,
        parameters: convert_params(parameters)?,
        properties: props(properties),
    }))
}

// ==========================================================================
// Parse / write
// ==========================================================================

macro_rules! parse_fn {
    ($name:ident, $fmt:literal) => {
        #[pyfunction]
        fn $name(text: &str) -> PyResult<Circuit> {
            oxymorph_netlist::formats::parse_dispatch($fmt, text)
                .map(Circuit)
                .map_err(|e| PyValueError::new_err(e.to_string()))
        }
    };
}

parse_fn!(parse_spice, "spice");
parse_fn!(parse_hspice, "hspice");
parse_fn!(parse_xyce, "xyce");
parse_fn!(parse_spectre, "spectre");
parse_fn!(parse_spectre_spice, "spectre_spice");
parse_fn!(parse_vams, "vams");

macro_rules! write_fn {
    ($name:ident, $fmt:literal) => {
        #[pyfunction]
        fn $name(circuit: &Circuit) -> PyResult<String> {
            oxymorph_netlist::formats::write_dispatch($fmt, &circuit.0)
                .map_err(|e| PyValueError::new_err(e.to_string()))
        }
    };
}

write_fn!(write_spice, "spice");
write_fn!(write_hspice, "hspice");
write_fn!(write_xyce, "xyce");
write_fn!(write_spectre, "spectre");
write_fn!(write_spectre_spice, "spectre_spice");
write_fn!(write_vams, "vams");

#[pyfunction]
fn __version__() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[pymodule]
fn oxymorph(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Circuit>()?;
    m.add_class::<Module>()?;
    m.add_class::<Port>()?;
    m.add_class::<PortReference>()?;
    m.add_class::<Connection>()?;
    m.add_class::<ModuleReference>()?;
    m.add_class::<Parameter>()?;
    m.add_class::<ParameterValue>()?;
    m.add_class::<PrefixedValue>()?;
    m.add_class::<ModelReference>()?;
    m.add_class::<ModelInterface>()?;
    m.add_class::<Directive>()?;
    m.add_class::<ExternalModule>()?;
    m.add_class::<ExternalLibrary>()?;
    m.add_class::<PortDirection>()?;
    m.add_class::<SignalDomain>()?;
    m.add_class::<Bus>()?;
    m.add_class::<Simulation>()?;
    m.add_class::<Analysis>()?;
    m.add_class::<OutputRequest>()?;
    m.add_class::<Measurement>()?;
    m.add_class::<InitialCondition>()?;

    m.add_function(wrap_pyfunction!(circuit, m)?)?;
    m.add_function(wrap_pyfunction!(module, m)?)?;
    m.add_function(wrap_pyfunction!(module_reference, m)?)?;
    m.add_function(wrap_pyfunction!(ext_module, m)?)?;
    m.add_function(wrap_pyfunction!(port, m)?)?;
    m.add_function(wrap_pyfunction!(connection, m)?)?;
    m.add_function(wrap_pyfunction!(parameter, m)?)?;
    m.add_function(wrap_pyfunction!(external_library, m)?)?;
    m.add_function(wrap_pyfunction!(model_interface, m)?)?;

    m.add_function(wrap_pyfunction!(parse_spice, m)?)?;
    m.add_function(wrap_pyfunction!(parse_hspice, m)?)?;
    m.add_function(wrap_pyfunction!(parse_xyce, m)?)?;
    m.add_function(wrap_pyfunction!(parse_spectre, m)?)?;
    m.add_function(wrap_pyfunction!(parse_spectre_spice, m)?)?;
    m.add_function(wrap_pyfunction!(parse_vams, m)?)?;
    m.add_function(wrap_pyfunction!(write_spice, m)?)?;
    m.add_function(wrap_pyfunction!(write_hspice, m)?)?;
    m.add_function(wrap_pyfunction!(write_xyce, m)?)?;
    m.add_function(wrap_pyfunction!(write_spectre, m)?)?;
    m.add_function(wrap_pyfunction!(write_spectre_spice, m)?)?;
    m.add_function(wrap_pyfunction!(write_vams, m)?)?;
    m.add_function(wrap_pyfunction!(__version__, m)?)?;
    Ok(())
}
