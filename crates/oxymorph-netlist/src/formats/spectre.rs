//! Spectre parser + writer, ported from `spectre/transformer.py` and
//! `spectre/writer.py`.

use indexmap::IndexMap;
use oxymorph_schema::*;

use crate::ast::TreeTransform;
use crate::engine::Engine;
use crate::error::Error;
use crate::helpers::*;
use crate::preprocess::preprocess_spectre;

const GRAMMAR: &str = include_str!("../../grammars/spectre.lark");

thread_local! {
    static ENGINE: Engine = Engine::lalr(GRAMMAR).expect("spectre grammar builds");
}

fn with_engine<R>(f: impl FnOnce(&Engine) -> R) -> R {
    ENGINE.with(|e| f(e))
}

// ---------------------------------------------------------------------------
// Intermediate values (mirrors the `*Data` classes in the transformer)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct InstanceData {
    pub name: String,
    pub net_names: Vec<String>,
    pub cell_name: String,
    pub params: IndexMap<String, ParameterValue>,
    pub device_prefix: String,
}

#[derive(Clone, Debug)]
pub struct ModelData {
    pub name: String,
    pub type_name: String,
    pub params: IndexMap<String, ParameterValue>,
}

#[derive(Clone, Debug)]
pub struct SubcktData {
    pub module: Module,
    pub ext_modules: IndexMap<String, ExternalModule>,
}

#[derive(Clone, Debug)]
pub struct IncludeData {
    pub path: String,
}

#[derive(Clone, Debug)]
pub struct ParamData {
    pub params: IndexMap<String, ParameterValue>,
}

#[derive(Clone, Debug)]
pub struct AnalysisData {
    pub kind: String,
    pub name: String,
    pub arguments: Vec<String>,
    pub options: IndexMap<String, String>,
}

#[derive(Clone, Debug)]
pub struct OutputRequestData {
    pub kind: String,
    pub analysis_type: String,
    pub variables: Vec<String>,
}

#[derive(Clone, Debug)]
#[allow(clippy::large_enum_variant)]
pub enum Item {
    Token(String, String),
    Tree(String, Vec<Item>),
    None,
    Circuit(Circuit),
    Str(String),
    Port(String, String),
    ParamKv(String, ParameterValue),
    Pval(ParameterValue),
    Instance(InstanceData),
    Model(ModelData),
    Subckt(SubcktData),
    Include(IncludeData),
    Params(ParamData),
    Analysis(AnalysisData),
    OutputReq(OutputRequestData),
    List(Vec<Item>),
}

fn tok(it: &Item) -> Option<&str> {
    match it {
        Item::Token(_, v) => Some(v),
        _ => None,
    }
}

fn param_value_str(pval: &ParameterValue) -> String {
    match &pval.prefixed_value {
        Some(pv) => format_si_value(pv),
        None => String::new(),
    }
}

const SPECTRE_ANALYSIS_TYPES: &[&str] = &[
    "tran", "dc", "ac", "noise", "sp", "stb", "pss", "pnoise", "xf", "psp", "op",
];

// ---------------------------------------------------------------------------
// Transformer
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct SpectreTransformer {
    all_subckts: IndexMap<String, Module>,
}

impl TreeTransform for SpectreTransformer {
    type Item = Item;

    fn token(&mut self, t: &lark_rs::tree::Token) -> Item {
        Item::Token(t.type_.clone(), t.value.clone())
    }

    fn none(&mut self) -> Item {
        Item::None
    }

    fn default_rule(&mut self, name: &str, items: Vec<Item>) -> Item {
        Item::Tree(name.to_string(), items)
    }

    fn rule(&mut self, name: &str, items: Vec<Item>) -> Item {
        match name {
            "net_ref" => Item::Str(tok(&items[0]).unwrap_or("").to_string()),
            "pvalue_number" => {
                let v = tok(&items[0]).unwrap_or("");
                Item::Pval(parse_parameter_number(v).unwrap_or_default())
            }
            "pvalue_ident" | "pvalue_string" => Item::Pval(ParameterValue::default()),
            "param_assign" => {
                let name = tok(&items[0]).unwrap_or("").to_string();
                let value = match items.get(1) {
                    Some(Item::Pval(p)) => p.clone(),
                    _ => ParameterValue::default(),
                };
                Item::ParamKv(name, value)
            }
            "port_decl" => Item::Port(
                "__port__".to_string(),
                tok(&items[0]).unwrap_or("").to_string(),
            ),
            "instance_stmt" => self.instance_stmt(items),
            "model_stmt" => self.model_stmt(items),
            "parameters_stmt" => self.parameters_stmt(items),
            "include_stmt" => {
                let path = tok(&items[0])
                    .unwrap_or("")
                    .trim_matches(|c| c == '"' || c == '\'');
                Item::Include(IncludeData {
                    path: path.to_string(),
                })
            }
            "save_stmt" => {
                let variables = items
                    .iter()
                    .filter_map(|i| match i {
                        Item::Str(s) => Some(s.clone()),
                        _ => None,
                    })
                    .collect();
                Item::OutputReq(OutputRequestData {
                    kind: "save".to_string(),
                    analysis_type: String::new(),
                    variables,
                })
            }
            "section_body" => Item::List(items.into_iter().filter(|i| !is_none(i)).collect()),
            "section_def" => items
                .into_iter()
                .find(|i| matches!(i, Item::List(_)))
                .unwrap_or(Item::List(vec![])),
            "subckt_body" => Item::List(items.into_iter().filter(|i| !is_none(i)).collect()),
            "subckt_def" => self.subckt_def(items),
            "start" => self.start(items),
            _ => Item::Tree(name.to_string(), items),
        }
    }
}

fn is_none(i: &Item) -> bool {
    matches!(i, Item::None)
}

impl SpectreTransformer {
    fn instance_stmt(&mut self, items: Vec<Item>) -> Item {
        let inst_name = tok(&items[0]).unwrap_or("").to_string();
        let device_prefix = inst_name
            .chars()
            .next()
            .map(|c| c.to_ascii_uppercase().to_string())
            .unwrap_or_default();

        let mut net_names = Vec::new();
        let mut cell_name = String::new();
        let mut params = IndexMap::new();

        for item in &items[1..] {
            match item {
                Item::Token(_, v) => cell_name = v.clone(),
                Item::ParamKv(pname, pval) => {
                    params.insert(pname.clone(), pval.clone());
                }
                Item::Str(s) => net_names.push(s.clone()),
                _ => {}
            }
        }

        if SPECTRE_ANALYSIS_TYPES.contains(&cell_name.as_str()) {
            let mut options = IndexMap::new();
            for (k, v) in &params {
                options.insert(k.clone(), param_value_str(v));
            }
            return Item::Analysis(AnalysisData {
                kind: cell_name,
                name: inst_name,
                arguments: vec![],
                options,
            });
        }

        Item::Instance(InstanceData {
            name: inst_name,
            net_names,
            cell_name,
            params,
            device_prefix,
        })
    }

    fn model_stmt(&mut self, items: Vec<Item>) -> Item {
        let name = tok(&items[0]).unwrap_or("").to_string();
        let type_name = tok(&items[1]).unwrap_or("").to_string();
        let mut params = IndexMap::new();
        for item in &items[2..] {
            if let Item::ParamKv(pname, pval) = item {
                params.insert(pname.clone(), pval.clone());
            }
        }
        Item::Model(ModelData {
            name,
            type_name,
            params,
        })
    }

    fn parameters_stmt(&mut self, items: Vec<Item>) -> Item {
        let mut params = IndexMap::new();
        for item in &items {
            if let Item::ParamKv(pname, pval) = item {
                params.insert(pname.clone(), pval.clone());
            }
        }
        Item::Params(ParamData { params })
    }

    fn subckt_def(&mut self, items: Vec<Item>) -> Item {
        let subckt_name = tok(&items[0]).unwrap_or("").to_string();
        let mut port_names = Vec::new();
        let mut subckt_params: IndexMap<String, ParameterValue> = IndexMap::new();
        let mut body_items: Vec<Item> = Vec::new();

        for item in &items[1..] {
            match item {
                Item::Port(_, name) => port_names.push(name.clone()),
                Item::ParamKv(pname, pval) => {
                    subckt_params.insert(pname.clone(), pval.clone());
                }
                Item::List(list) => body_items = list.clone(),
                _ => {}
            }
        }

        let result = self.build_subckt(subckt_name.clone(), port_names, subckt_params, body_items);
        self.all_subckts.insert(subckt_name, result.module.clone());
        Item::Subckt(result)
    }

    #[allow(clippy::unnecessary_unwrap)]
    fn build_subckt(
        &self,
        name: String,
        port_names: Vec<String>,
        subckt_params: IndexMap<String, ParameterValue>,
        body_items: Vec<Item>,
    ) -> SubcktData {
        let mut instances: Vec<InstanceData> = Vec::new();
        let mut nested_subckts: IndexMap<String, Module> = IndexMap::new();
        let mut ext_modules: IndexMap<String, ExternalModule> = IndexMap::new();
        let mut model_defs: IndexMap<String, ModelData> = IndexMap::new();
        let mut circuit_params: Vec<Parameter> = Vec::new();
        let mut directives: Vec<Directive> = Vec::new();

        for item in &body_items {
            match item {
                Item::Instance(i) => instances.push(i.clone()),
                Item::Subckt(s) => {
                    nested_subckts.insert(s.module.name.clone(), s.module.clone());
                    ext_modules.extend(s.ext_modules.clone());
                }
                Item::Model(m) => {
                    model_defs.insert(m.name.clone(), m.clone());
                }
                Item::Params(p) => {
                    for (pname, pval) in &p.params {
                        circuit_params.push(Parameter {
                            name: pname.clone(),
                            default_value: Some(pval.clone()),
                            ..Default::default()
                        });
                    }
                }
                Item::Include(inc) => directives.push(Directive {
                    kind: DirectiveKind::Include,
                    value: inc.path.clone(),
                    ..Default::default()
                }),
                Item::List(list) => {
                    for sub in list {
                        if let Item::Instance(i) = sub {
                            instances.push(i.clone());
                        }
                    }
                }
                _ => {}
            }
        }

        for (pname, pval) in &subckt_params {
            circuit_params.push(Parameter {
                name: pname.clone(),
                default_value: Some(pval.clone()),
                ..Default::default()
            });
        }

        let ports: Vec<Port> = port_names
            .iter()
            .map(|pn| Port {
                name: pn.clone(),
                direction: PortDirection::Inout,
                domain: SignalDomain::Electrical,
                ..Default::default()
            })
            .collect();

        let mut net_map = NetMap::new();
        let mut module_references: Vec<ModuleReference> = Vec::new();

        for port in &ports {
            net_map.add(&port.name, "", &port.name);
        }

        for inst in &instances {
            let device_prefix = inst.device_prefix.as_str();
            let port_template = device_port_template(device_prefix);

            if device_prefix == "X" || port_template.is_none() {
                let subckt_mod = nested_subckts
                    .get(&inst.cell_name)
                    .or_else(|| self.all_subckts.get(&inst.cell_name));
                let inst_port_names: Vec<String> = match subckt_mod {
                    Some(m) => m.ports.iter().map(|p| p.name.clone()).collect(),
                    None => (0..inst.net_names.len()).map(|i| format!("p{i}")).collect(),
                };
                for (net_name, port_name) in inst.net_names.iter().zip(inst_port_names.iter()) {
                    net_map.add(net_name, &inst.name, port_name);
                }
                module_references.push(ModuleReference {
                    name: inst.name.clone(),
                    module_name: inst.cell_name.clone(),
                    parameters: inst
                        .params
                        .iter()
                        .map(|(n, v)| Parameter {
                            name: n.clone(),
                            default_value: Some(v.clone()),
                            ..Default::default()
                        })
                        .collect(),
                    ..Default::default()
                });
            } else {
                let template = port_template.unwrap();
                let module_name = device_prefix_to_module(device_prefix)
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| device_prefix.to_lowercase());

                for (net_name, port_name) in inst.net_names.iter().zip(template.iter()) {
                    net_map.add(net_name, &inst.name, port_name);
                }

                if !ext_modules.contains_key(&module_name) {
                    let ext_ports = template
                        .iter()
                        .map(|pn| Port {
                            name: pn.to_string(),
                            direction: PortDirection::Inout,
                            domain: SignalDomain::Electrical,
                            ..Default::default()
                        })
                        .collect();
                    let mut props = Properties::new();
                    props.insert("spice_prefix".to_string(), device_prefix.to_string());
                    ext_modules.insert(
                        module_name.clone(),
                        ExternalModule {
                            name: module_name.clone(),
                            ports: ext_ports,
                            properties: props,
                            ..Default::default()
                        },
                    );
                }

                let mut props = Properties::new();
                if !inst.cell_name.is_empty() && inst.cell_name != module_name {
                    props.insert("model_name".to_string(), inst.cell_name.clone());
                }
                module_references.push(ModuleReference {
                    name: inst.name.clone(),
                    module_name,
                    parameters: inst
                        .params
                        .iter()
                        .map(|(n, v)| Parameter {
                            name: n.clone(),
                            default_value: Some(v.clone()),
                            ..Default::default()
                        })
                        .collect(),
                    properties: props,
                    ..Default::default()
                });
            }
        }

        for mdata in model_defs.values() {
            if !ext_modules.contains_key(&mdata.name) {
                let mut props = Properties::new();
                props.insert("model_type".to_string(), mdata.type_name.clone());
                ext_modules.insert(
                    mdata.name.clone(),
                    ExternalModule {
                        name: mdata.name.clone(),
                        parameters: mdata
                            .params
                            .iter()
                            .map(|(n, v)| Parameter {
                                name: n.clone(),
                                default_value: Some(v.clone()),
                                ..Default::default()
                            })
                            .collect(),
                        properties: props,
                        ..Default::default()
                    },
                );
            }
        }

        let connections = net_map_to_connections(&net_map);

        let module = Module {
            name,
            ports,
            parameters: circuit_params,
            module_references,
            connections,
            directives,
            ..Default::default()
        };
        SubcktData {
            module,
            ext_modules,
        }
    }

    fn start(&mut self, items: Vec<Item>) -> Item {
        let mut subckt_datas: Vec<SubcktData> = Vec::new();
        let mut model_defs: IndexMap<String, ModelData> = IndexMap::new();
        let mut top_instances: Vec<InstanceData> = Vec::new();
        let mut directives: Vec<Directive> = Vec::new();
        let mut analyses: Vec<AnalysisData> = Vec::new();
        let mut output_requests: Vec<OutputRequestData> = Vec::new();

        for item in &items {
            match item {
                Item::None => {}
                Item::Subckt(s) => subckt_datas.push(s.clone()),
                Item::Model(m) => {
                    model_defs.insert(m.name.clone(), m.clone());
                }
                Item::Instance(i) => top_instances.push(i.clone()),
                Item::Analysis(a) => analyses.push(a.clone()),
                Item::OutputReq(o) => output_requests.push(o.clone()),
                Item::Include(inc) => directives.push(Directive {
                    kind: DirectiveKind::Include,
                    value: inc.path.clone(),
                    ..Default::default()
                }),
                Item::Params(p) => {
                    for (pname, pval) in &p.params {
                        directives.push(Directive {
                            kind: DirectiveKind::Param,
                            name: pname.clone(),
                            value: param_value_str(pval),
                        });
                    }
                }
                Item::List(list) => {
                    for sub in list {
                        match sub {
                            Item::Subckt(s) => subckt_datas.push(s.clone()),
                            Item::Analysis(a) => analyses.push(a.clone()),
                            Item::OutputReq(o) => output_requests.push(o.clone()),
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }

        let mut modules: Vec<Module> = Vec::new();
        let mut all_ext_modules: IndexMap<String, ExternalModule> = IndexMap::new();

        for sd in &subckt_datas {
            modules.push(sd.module.clone());
            all_ext_modules.extend(sd.ext_modules.clone());
        }

        if !top_instances.is_empty() {
            let top_data = self.build_subckt(
                "__top__".to_string(),
                vec![],
                IndexMap::new(),
                top_instances
                    .iter()
                    .map(|i| Item::Instance(i.clone()))
                    .collect(),
            );
            modules.insert(0, top_data.module.clone());
            all_ext_modules.extend(top_data.ext_modules.clone());
        }

        for mdata in model_defs.values() {
            if !all_ext_modules.contains_key(&mdata.name) {
                let mut props = Properties::new();
                props.insert("model_type".to_string(), mdata.type_name.clone());
                all_ext_modules.insert(
                    mdata.name.clone(),
                    ExternalModule {
                        name: mdata.name.clone(),
                        parameters: mdata
                            .params
                            .iter()
                            .map(|(n, v)| Parameter {
                                name: n.clone(),
                                default_value: Some(v.clone()),
                                ..Default::default()
                            })
                            .collect(),
                        properties: props,
                        ..Default::default()
                    },
                );
            }
        }

        let top_module = modules.last().map(|m| m.name.clone()).unwrap_or_default();

        let simulation = if !analyses.is_empty() || !output_requests.is_empty() {
            Some(Simulation {
                analyses: analyses
                    .iter()
                    .map(|a| Analysis {
                        kind: analysis_kind(&a.kind).unwrap_or_default(),
                        name: a.name.clone(),
                        arguments: a.arguments.clone(),
                        options: a
                            .options
                            .iter()
                            .map(|(k, v)| (k.clone(), v.clone()))
                            .collect(),
                    })
                    .collect(),
                output_requests: output_requests
                    .iter()
                    .map(|o| OutputRequest {
                        kind: output_request_kind(&o.kind).unwrap_or_default(),
                        analysis_type: o.analysis_type.clone(),
                        variables: o.variables.clone(),
                    })
                    .collect(),
                ..Default::default()
            })
        } else {
            None
        };

        Item::Circuit(Circuit {
            name: String::new(),
            top_module,
            modules,
            ext_modules: all_ext_modules.into_values().collect(),
            directives,
            simulation,
            ..Default::default()
        })
    }
}

pub fn parse_spectre(text: &str) -> Result<Circuit, Error> {
    let cleaned = preprocess_spectre(text);
    let tree = with_engine(|e| e.parse(&cleaned))?;
    let mut t = SpectreTransformer::default();
    match t.transform(&tree) {
        Item::Circuit(c) => Ok(c),
        _ => Err(Error::Validation(
            "spectre transform did not produce a circuit".into(),
        )),
    }
}

// ---------------------------------------------------------------------------
// Writer
// ---------------------------------------------------------------------------

fn module_to_prefix(module: &str) -> Option<&'static str> {
    Some(match module {
        "resistor" => "R",
        "capacitor" => "C",
        "inductor" => "L",
        "diode" => "D",
        "bjt" => "Q",
        "mosfet" => "M",
        "vsource" => "V",
        "isource" => "I",
        _ => return None,
    })
}

fn format_param_value(pval: &ParameterValue) -> String {
    match &pval.prefixed_value {
        Some(pv) => format_si_value(pv),
        None => "0".to_string(),
    }
}

fn analysis_kind_to_spectre(kind: AnalysisKind) -> &'static str {
    match kind {
        AnalysisKind::Op => "op",
        AnalysisKind::Dc => "dc",
        AnalysisKind::Ac => "ac",
        AnalysisKind::Tran => "tran",
        AnalysisKind::Noise => "noise",
        _ => "dc",
    }
}

pub fn write_spectre(circuit: &Circuit) -> String {
    let mut lines: Vec<String> = Vec::new();

    let ext_by_name: IndexMap<&str, &ExternalModule> = circuit
        .ext_modules
        .iter()
        .map(|e| (e.name.as_str(), e))
        .collect();
    let mod_by_name: IndexMap<&str, &Module> = circuit
        .modules
        .iter()
        .map(|m| (m.name.as_str(), m))
        .collect();

    // directives
    let mut params: IndexMap<String, String> = IndexMap::new();
    for d in &circuit.directives {
        if d.kind == DirectiveKind::Include {
            lines.push(format!("include \"{}\"", d.value));
        } else if d.kind == DirectiveKind::Param {
            params.insert(d.name.clone(), d.value.clone());
        }
    }
    if !params.is_empty() {
        let assigns = params
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join(" ");
        lines.push(format!("parameters {assigns}"));
    }

    for ext in &circuit.ext_modules {
        if let Some(model_type) = ext.properties.get("model_type") {
            let param_strs: Vec<String> = ext
                .parameters
                .iter()
                .filter_map(|p| {
                    p.default_value
                        .as_ref()
                        .map(|v| format!("{}={}", p.name, format_param_value(v)))
                })
                .collect();
            lines.push(format!(
                "model {} {} ({})",
                ext.name,
                model_type,
                param_strs.join(" ")
            ));
        }
    }

    for module in &circuit.modules {
        if module.name == "__top__" {
            write_top_instances(module, &ext_by_name, &mod_by_name, &mut lines);
        } else {
            write_subckt(module, &ext_by_name, &mod_by_name, &mut lines);
        }
    }

    if let Some(sim) = &circuit.simulation {
        for analysis in &sim.analyses {
            lines.push(format_analysis(analysis));
        }
        for req in &sim.output_requests {
            lines.push(format!("save {}", req.variables.join(" ")));
        }
    }

    lines.push(String::new());
    lines.join("\n")
}

fn write_subckt(
    module: &Module,
    ext_by_name: &IndexMap<&str, &ExternalModule>,
    mod_by_name: &IndexMap<&str, &Module>,
    lines: &mut Vec<String>,
) {
    let port_names = module
        .ports
        .iter()
        .map(|p| p.name.clone())
        .collect::<Vec<_>>()
        .join(" ");

    let param_strs: Vec<String> = module
        .parameters
        .iter()
        .filter_map(|p| {
            p.default_value
                .as_ref()
                .map(|v| format!("{}={}", p.name, format_param_value(v)))
        })
        .collect();
    let params_suffix = if param_strs.is_empty() {
        String::new()
    } else {
        format!(" {}", param_strs.join(" "))
    };

    lines.push(format!(
        "subckt {module_name} ({port_names}){params_suffix}",
        module_name = module.name
    ));

    let inst_nets = connections_to_instance_nets(&module.connections);
    for r in &module.module_references {
        lines.push(write_instance(r, &inst_nets, ext_by_name, mod_by_name));
    }
    lines.push(format!("ends {}", module.name));
}

fn write_top_instances(
    module: &Module,
    ext_by_name: &IndexMap<&str, &ExternalModule>,
    mod_by_name: &IndexMap<&str, &Module>,
    lines: &mut Vec<String>,
) {
    let inst_nets = connections_to_instance_nets(&module.connections);
    for r in &module.module_references {
        lines.push(write_instance(r, &inst_nets, ext_by_name, mod_by_name));
    }
}

#[allow(clippy::unnecessary_unwrap)]
fn write_instance(
    r: &ModuleReference,
    inst_nets: &IndexMap<(String, String), String>,
    ext_by_name: &IndexMap<&str, &ExternalModule>,
    mod_by_name: &IndexMap<&str, &Module>,
) -> String {
    let ext_mod = ext_by_name.get(r.module_name.as_str());
    let prefix = module_to_prefix(&r.module_name);

    let port_order: Vec<String> = if prefix.is_some() && ext_mod.is_some() {
        ext_mod
            .unwrap()
            .ports
            .iter()
            .map(|p| p.name.clone())
            .collect()
    } else if let Some(rm) = mod_by_name.get(r.module_name.as_str()) {
        rm.ports.iter().map(|p| p.name.clone()).collect()
    } else if let Some(em) = ext_mod {
        em.ports.iter().map(|p| p.name.clone()).collect()
    } else {
        infer_port_order(r, inst_nets)
    };

    let nets = port_order
        .iter()
        .map(|pn| {
            inst_nets
                .get(&(r.name.clone(), pn.clone()))
                .cloned()
                .unwrap_or_else(|| "?".to_string())
        })
        .collect::<Vec<_>>()
        .join(" ");

    let cell_name = r
        .properties
        .get("model_name")
        .cloned()
        .unwrap_or_else(|| r.module_name.clone());

    let param_strs: Vec<String> = r
        .parameters
        .iter()
        .filter_map(|p| {
            p.default_value
                .as_ref()
                .map(|v| format!("{}={}", p.name, format_param_value(v)))
        })
        .collect();
    let params_text = if param_strs.is_empty() {
        String::new()
    } else {
        format!(" {}", param_strs.join(" "))
    };

    format!("{} ({nets}) {cell_name}{params_text}", r.name)
}

fn infer_port_order(
    r: &ModuleReference,
    inst_nets: &IndexMap<(String, String), String>,
) -> Vec<String> {
    let mut port_names: Vec<String> = Vec::new();
    for (iname, pname) in inst_nets.keys() {
        if iname == &r.name && !port_names.contains(pname) {
            port_names.push(pname.clone());
        }
    }
    port_names
}

fn format_analysis(analysis: &Analysis) -> String {
    let keyword = analysis_kind_to_spectre(analysis.kind);
    let mut parts = vec![
        if analysis.name.is_empty() {
            keyword.to_string()
        } else {
            analysis.name.clone()
        },
        keyword.to_string(),
    ];
    for (k, v) in &analysis.options {
        parts.push(format!("{k}={v}"));
    }
    parts.join(" ")
}
