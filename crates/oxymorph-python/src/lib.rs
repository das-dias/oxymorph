//! Native half of the `oxymorph` Python package (`oxymorph._oxymorph`).
//!
//! A JSON bridge to `oxymorph-schema` / `oxymorph-netlist` (`decisions/0012`): every
//! function takes and returns the IR's JSON (`Circuit::to_json`), and the Python
//! package turns it into its dataclass object model. No IR logic lives here.

use indexmap::IndexMap;
use oxymorph_netlist::expression::Dialect;
use oxymorph_netlist::hdl::HdlOptions;
use oxymorph_netlist::helpers::PinDomains;
use oxymorph_netlist::GschOptions;
use oxymorph_schema::{paths, Circuit, Expression, SignalDomain};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyBytes;

fn value_error(error: impl ToString) -> PyErr {
    PyValueError::new_err(error.to_string())
}

fn circuit_of(circuit_json: &str) -> PyResult<Circuit> {
    Circuit::from_json(circuit_json).map_err(value_error)
}

fn json_of(circuit: &Circuit) -> PyResult<String> {
    circuit.to_json().map_err(value_error)
}

/// `format` (`spice`, `hspice`, `xyce`, `spectre`, `spectre_spice`, `vams`,
/// `systemverilog`, `gsch`) parsed with default options, as circuit JSON.
#[pyfunction]
fn parse(format: &str, text: &str) -> PyResult<String> {
    let circuit = oxymorph_netlist::parse_dispatch(format, text).map_err(value_error)?;
    json_of(&circuit)
}

/// `systemverilog` or `vams`; the options are JSON objects (`{"IS_SIM": "1"}`,
/// `{"connection": 2}`, `{"CELL": {"pin": 2}}`), `pin_domains` may be `null`.
#[pyfunction]
fn parse_hdl(
    format: &str,
    text: &str,
    defines_json: &str,
    disciplines_json: &str,
    pin_domains_json: &str,
) -> PyResult<String> {
    let options = HdlOptions {
        defines: serde_json::from_str::<IndexMap<String, String>>(defines_json)
            .map_err(value_error)?,
        disciplines: serde_json::from_str::<IndexMap<String, SignalDomain>>(disciplines_json)
            .map_err(value_error)?,
        pin_domains: serde_json::from_str::<Option<PinDomains>>(pin_domains_json)
            .map_err(value_error)?,
    };
    let circuit = match format {
        "systemverilog" => oxymorph_netlist::parse_systemverilog(text, &options),
        "vams" => oxymorph_netlist::parse_vams_with(text, &options),
        other => return Err(value_error(format!("not an HDL format: {other}"))),
    }
    .map_err(value_error)?;
    json_of(&circuit)
}

fn gsch_options(
    models_json: Option<&str>,
    sim: String,
    corners: Vec<String>,
    name: Option<String>,
) -> PyResult<GschOptions> {
    let models = match models_json {
        Some(text) => Some(serde_json::from_str(text).map_err(value_error)?),
        None => None,
    };
    Ok(GschOptions {
        models,
        sim,
        corners,
        name,
    })
}

#[pyfunction]
#[pyo3(signature = (text, models_json, sim, corners, name))]
fn parse_gsch(
    text: &str,
    models_json: Option<&str>,
    sim: String,
    corners: Vec<String>,
    name: Option<String>,
) -> PyResult<String> {
    let options = gsch_options(models_json, sim, corners, name)?;
    let circuit = oxymorph_netlist::parse_gsch(text, &options).map_err(value_error)?;
    json_of(&circuit)
}

#[pyfunction]
#[pyo3(signature = (path, project_root, models_json, sim, corners))]
fn read_gsch(
    path: &str,
    project_root: Option<&str>,
    models_json: Option<&str>,
    sim: String,
    corners: Vec<String>,
) -> PyResult<String> {
    let options = gsch_options(models_json, sim, corners, None)?;
    let circuit = oxymorph_netlist::read_gsch(path, project_root, &options).map_err(value_error)?;
    json_of(&circuit)
}

/// Circuit JSON written as `format`.
#[pyfunction]
fn write(format: &str, circuit_json: &str) -> PyResult<String> {
    oxymorph_netlist::write_dispatch(format, &circuit_of(circuit_json)?).map_err(value_error)
}

/// Circuit JSON in its canonical form: the IR's serde output.
#[pyfunction]
fn circuit_to_json(circuit_json: &str, pretty: bool) -> PyResult<String> {
    let circuit = circuit_of(circuit_json)?;
    if pretty {
        circuit.to_json_pretty().map_err(value_error)
    } else {
        json_of(&circuit)
    }
}

#[pyfunction]
fn circuit_to_yaml(circuit_json: &str) -> PyResult<String> {
    circuit_of(circuit_json)?.to_yaml().map_err(value_error)
}

#[pyfunction]
fn circuit_from_yaml(yaml: &str) -> PyResult<String> {
    json_of(&Circuit::from_yaml(yaml).map_err(value_error)?)
}

#[pyfunction]
fn circuit_to_proto<'py>(py: Python<'py>, circuit_json: &str) -> PyResult<Bound<'py, PyBytes>> {
    Ok(PyBytes::new(
        py,
        &circuit_of(circuit_json)?.to_proto_bytes(),
    ))
}

#[pyfunction]
fn circuit_from_proto(data: &[u8]) -> PyResult<String> {
    json_of(&Circuit::from_proto_bytes(data).map_err(value_error)?)
}

/// The expression tree of `text` as JSON; `{"raw": text}` when it does not parse.
#[pyfunction]
fn parse_expression(text: &str) -> PyResult<String> {
    let expression = oxymorph_netlist::expression::parse_expression(text).map_err(value_error)?;
    serde_json::to_string(&expression).map_err(value_error)
}

#[pyfunction]
fn format_expression(expression_json: &str, dialect: &str, named: bool) -> PyResult<String> {
    let expression: Expression = serde_json::from_str(expression_json).map_err(value_error)?;
    expression.validate().map_err(value_error)?;
    let dialect: Dialect = dialect.parse().map_err(value_error)?;
    Ok(oxymorph_netlist::expression::format_expression(
        &expression,
        dialect,
        None,
        named,
    ))
}

#[pyfunction]
fn posix_file(path: &str) -> String {
    paths::posix_file(path)
}

#[pyfunction]
fn is_absolute(path: &str) -> bool {
    paths::is_absolute(path)
}

#[pyfunction]
#[pyo3(signature = (path, base = None))]
fn resolve_file(path: &str, base: Option<&str>) -> String {
    paths::resolve_file(path, base)
}

#[pymodule]
fn _oxymorph(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add("__version__", env!("CARGO_PKG_VERSION"))?;
    module.add_function(wrap_pyfunction!(parse, module)?)?;
    module.add_function(wrap_pyfunction!(parse_hdl, module)?)?;
    module.add_function(wrap_pyfunction!(parse_gsch, module)?)?;
    module.add_function(wrap_pyfunction!(read_gsch, module)?)?;
    module.add_function(wrap_pyfunction!(write, module)?)?;
    module.add_function(wrap_pyfunction!(circuit_to_json, module)?)?;
    module.add_function(wrap_pyfunction!(circuit_to_yaml, module)?)?;
    module.add_function(wrap_pyfunction!(circuit_from_yaml, module)?)?;
    module.add_function(wrap_pyfunction!(circuit_to_proto, module)?)?;
    module.add_function(wrap_pyfunction!(circuit_from_proto, module)?)?;
    module.add_function(wrap_pyfunction!(parse_expression, module)?)?;
    module.add_function(wrap_pyfunction!(format_expression, module)?)?;
    module.add_function(wrap_pyfunction!(posix_file, module)?)?;
    module.add_function(wrap_pyfunction!(is_absolute, module)?)?;
    module.add_function(wrap_pyfunction!(resolve_file, module)?)?;
    Ok(())
}
