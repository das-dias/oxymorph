//! SPICE-family parser + writer, ported from `spice/transformer.py` and
//! `spice/writer.py`. `hspice`, `xyce` and `spectre_spice` reuse this base
//! with a [`Mode`] switch (the upstream subclasses are thin overrides).

use indexmap::IndexMap;
use oxymorph_schema::*;

use crate::ast::TreeTransform;
use crate::engine::Engine;
use crate::error::Error;
use crate::helpers::*;
use crate::preprocess::{preprocess_hspice, preprocess_spice, preprocess_xyce};

const GRAMMAR_SPICE: &str = include_str!("../../grammars/spice.lark");
const GRAMMAR_HSPICE: &str = include_str!("../../grammars/hspice.lark");
const GRAMMAR_XYCE: &str = include_str!("../../grammars/xyce.lark");

thread_local! {
    static E_SPICE: Engine = Engine::earley(GRAMMAR_SPICE).expect("spice grammar builds");
    static E_HSPICE: Engine = Engine::earley(GRAMMAR_HSPICE).expect("hspice grammar builds");
    static E_XYCE: Engine = Engine::earley(GRAMMAR_XYCE).expect("xyce grammar builds");
}

fn with_engine<R>(mode: Mode, f: impl FnOnce(&Engine) -> R) -> R {
    match mode {
        Mode::Spice => E_SPICE.with(|e| f(e)),
        Mode::Hspice => E_HSPICE.with(|e| f(e)),
        Mode::Xyce => E_XYCE.with(|e| f(e)),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Spice,
    Hspice,
    Xyce,
}

// ---------------------------------------------------------------------------
// Intermediate values
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct InstanceData {
    pub name: String,
    pub net_names: Vec<String>,
    pub model_or_subckt: String,
    pub value: Option<PrefixedValue>,
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
pub struct LibData {
    pub path: String,
    pub section: String,
}

#[derive(Clone, Debug)]
pub struct GlobalData {
    pub nets: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct OptionData {
    pub options: IndexMap<String, String>,
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
pub struct MeasurementData {
    pub name: String,
    pub analysis_type: String,
    pub body: String,
}

#[derive(Clone, Debug)]
pub struct InitialConditionData {
    pub kind: String,
    pub conditions: IndexMap<String, String>,
}

#[derive(Clone, Debug)]
pub struct TemperatureData {
    pub temperatures: Vec<f64>,
}

#[derive(Clone, Debug)]
#[allow(clippy::large_enum_variant)]
pub enum Item {
    Token(String, String),
    Tree(String, Vec<Item>),
    None,
    Circuit(Circuit),
    Pval(ParameterValue),
    ParamKv(String, ParameterValue),
    Port(String, String),
    Instance(InstanceData),
    Model(ModelData),
    Subckt(SubcktData),
    Include(IncludeData),
    Lib(LibData),
    Global(GlobalData),
    Option(OptionData),
    Params(ParamData),
    Analysis(AnalysisData),
    OutputReq(OutputRequestData),
    Meas(MeasurementData),
    Ic(InitialConditionData),
    IcAssign(String, String),
    Temp(TemperatureData),
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

// ---------------------------------------------------------------------------
// Transformer
// ---------------------------------------------------------------------------

pub struct SpiceTransformer {
    pub mode: Mode,
    all_subckts: IndexMap<String, Module>,
}

impl SpiceTransformer {
    pub fn new(mode: Mode) -> Self {
        Self {
            mode,
            all_subckts: IndexMap::new(),
        }
    }
}

impl TreeTransform for SpiceTransformer {
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
            "token_number" | "token_ident" | "token_output_var" | "token_expr" => {
                items.into_iter().next().unwrap_or(Item::None)
            }
            "pvalue_number" => {
                let v = tok(&items[0]).unwrap_or("");
                Item::Pval(parse_parameter_number(v).unwrap_or_default())
            }
            "pvalue_ident" | "pvalue_string" | "pvalue_qstring" | "pvalue_expr" => {
                Item::Pval(ParameterValue::default())
            }
            "params_kw" => Item::None,
            "param_assign" => {
                let name = tok(&items[0]).unwrap_or("").to_string();
                let value = match items.get(1) {
                    Some(Item::Pval(p)) => p.clone(),
                    _ => ParameterValue::default(),
                };
                Item::ParamKv(name, value)
            }
            "instance_stmt" => self.instance_stmt(items),
            "model_stmt" => self.model_stmt(items),
            "param_stmt" => self.param_stmt(items),
            "include_stmt" => {
                let path = tok(&items[1])
                    .unwrap_or("")
                    .trim_matches(|c| c == '"' || c == '\'');
                Item::Include(IncludeData {
                    path: path.to_string(),
                })
            }
            "lib_stmt" => {
                let path = tok(&items[1])
                    .unwrap_or("")
                    .trim_matches(|c| c == '"' || c == '\'');
                let section = items.get(2).and_then(tok).unwrap_or("").to_string();
                Item::Lib(LibData {
                    path: path.to_string(),
                    section,
                })
            }
            "global_stmt" => {
                let nets = items
                    .iter()
                    .filter_map(|i| match i {
                        Item::Token(_, v) if !v.starts_with('.') => Some(v.clone()),
                        _ => None,
                    })
                    .collect();
                Item::Global(GlobalData { nets })
            }
            "option_stmt" => {
                let options = param_assign_map(&items);
                Item::Option(OptionData { options })
            }
            "end_stmt" => Item::None,
            "op_stmt" => Item::Analysis(AnalysisData {
                kind: "op".to_string(),
                name: String::new(),
                arguments: vec![],
                options: IndexMap::new(),
            }),
            "tran_stmt" | "ac_stmt" | "dc_stmt" | "noise_stmt" | "tf_stmt" | "sens_stmt"
            | "pz_stmt" | "disto_stmt" | "four_stmt" | "fft_stmt" => {
                let kind = name.trim_end_matches("_stmt").to_string();
                let (arguments, options) = extract_sim_args(&items);
                Item::Analysis(AnalysisData {
                    kind,
                    name: String::new(),
                    arguments,
                    options,
                })
            }
            "print_stmt" | "plot_stmt" | "probe_stmt" => {
                let kind = name.trim_end_matches("_stmt").to_string();
                Item::OutputReq(output_request(&items, kind))
            }
            "meas_stmt" => Item::Meas(meas_stmt(&items)),
            "ic_assign" => {
                let name = tok(&items[0]).unwrap_or("").to_string();
                let value = match items.get(1) {
                    Some(Item::Pval(p)) => param_value_str(p),
                    _ => items.get(1).and_then(tok).unwrap_or("").to_string(),
                };
                Item::IcAssign(name, value)
            }
            "ic_stmt" | "nodeset_stmt" => {
                let kind = if name == "ic_stmt" { "ic" } else { "nodeset" };
                let conditions = ic_conditions(&items);
                Item::Ic(InitialConditionData {
                    kind: kind.to_string(),
                    conditions,
                })
            }
            "temp_stmt" => Item::Temp(TemperatureData {
                temperatures: temp_values(&items),
            }),
            "subckt_body" => Item::List(items.into_iter().filter(|i| !is_none(i)).collect()),
            "port_decl" => Item::Port(
                "__port__".to_string(),
                tok(&items[0]).unwrap_or("").to_string(),
            ),
            "subckt_def" | "macro_def" => self.subckt_def(items),
            "start" => self.start(items),
            _ => Item::Tree(name.to_string(), items),
        }
    }
}

fn is_none(i: &Item) -> bool {
    matches!(i, Item::None)
}

fn param_assign_values(items: &[Item]) -> IndexMap<String, ParameterValue> {
    let mut map = IndexMap::new();
    for item in items {
        if let Item::ParamKv(n, v) = item {
            map.insert(n.clone(), v.clone());
        }
    }
    map
}

fn param_assign_map(items: &[Item]) -> IndexMap<String, String> {
    let mut map = IndexMap::new();
    for item in items {
        if let Item::ParamKv(n, v) = item {
            map.insert(n.clone(), param_value_str(v));
        }
    }
    map
}

fn extract_sim_args(items: &[Item]) -> (Vec<String>, IndexMap<String, String>) {
    let mut args = Vec::new();
    let mut opts = IndexMap::new();
    for item in items {
        match item {
            Item::ParamKv(n, v) => {
                opts.insert(n.clone(), param_value_str(v));
            }
            Item::Token(_, v) if !v.starts_with('.') => {
                args.push(v.clone());
            }
            _ => {}
        }
    }
    (args, opts)
}

fn output_request(items: &[Item], kind: String) -> OutputRequestData {
    let tokens: Vec<String> = items
        .iter()
        .filter_map(|i| match i {
            Item::Token(_, v) if !v.starts_with('.') => Some(v.clone()),
            _ => None,
        })
        .collect();
    let analysis_type = tokens.first().cloned().unwrap_or_default();
    let variables = if tokens.is_empty() {
        vec![]
    } else {
        tokens[1..].to_vec()
    };
    OutputRequestData {
        kind,
        analysis_type,
        variables,
    }
}

fn meas_stmt(items: &[Item]) -> MeasurementData {
    let tokens: Vec<String> = items
        .iter()
        .filter_map(|i| match i {
            Item::Token(_, v) if !v.starts_with('.') => Some(v.clone()),
            _ => None,
        })
        .collect();
    let mut opts: Vec<String> = Vec::new();
    for item in items {
        if let Item::ParamKv(n, v) = item {
            opts.push(format!("{n}={}", param_value_str(v)));
        }
    }
    let analysis_type = tokens.first().cloned().unwrap_or_default();
    let name = tokens.get(1).cloned().unwrap_or_default();
    let mut body_parts: Vec<String> = if tokens.len() > 2 {
        tokens[2..].to_vec()
    } else {
        vec![]
    };
    body_parts.extend(opts);
    MeasurementData {
        name,
        analysis_type,
        body: body_parts.join(" "),
    }
}

/// `ic_assign` is encoded as `ParamKv`, but its value is a string, so we store
/// it in the canonical `(name, value)` shape via a `Token`-free encoding.
fn ic_conditions(items: &[Item]) -> IndexMap<String, String> {
    let mut conditions = IndexMap::new();
    for item in items {
        if let Item::IcAssign(n, v) = item {
            conditions.insert(n.clone(), v.clone());
        }
    }
    conditions
}

fn temp_values(items: &[Item]) -> Vec<f64> {
    let mut temps = Vec::new();
    for item in items {
        if let Item::Token(_, v) = item {
            if v.starts_with('.') {
                continue;
            }
            if let Ok(f) = v.parse::<f64>() {
                temps.push(f);
            }
        }
    }
    temps
}

impl SpiceTransformer {
    fn instance_stmt(&mut self, items: Vec<Item>) -> Item {
        let inst_name = tok(&items[0]).unwrap_or("").to_string();
        let device_prefix = inst_name
            .chars()
            .next()
            .map(|c| c.to_ascii_uppercase().to_string())
            .unwrap_or_default();

        let mut tokens: Vec<String> = Vec::new();
        let mut params: IndexMap<String, ParameterValue> = IndexMap::new();
        for item in &items[1..] {
            match item {
                Item::ParamKv(n, v) => {
                    params.insert(n.clone(), v.clone());
                }
                Item::Token(_, v) => tokens.push(v.clone()),
                _ => {}
            }
        }

        let port_template = device_port_template(&device_prefix);

        if device_prefix == "X" {
            let model_or_subckt = tokens.last().cloned().unwrap_or_default();
            let net_names = if tokens.is_empty() {
                vec![]
            } else {
                tokens[..tokens.len() - 1].to_vec()
            };
            return Item::Instance(InstanceData {
                name: inst_name,
                net_names,
                model_or_subckt,
                value: None,
                params,
                device_prefix,
            });
        }

        if let Some(template) = port_template {
            let n_ports = template.len();
            let net_names = tokens[..n_ports.min(tokens.len())].to_vec();
            let remaining = if tokens.len() > n_ports {
                &tokens[n_ports..]
            } else {
                &[]
            };
            let mut value = None;
            let mut model_name = String::new();
            for t in remaining {
                if is_si_number(t) {
                    value = parse_si_number(t).ok();
                } else {
                    model_name = t.clone();
                }
            }
            return Item::Instance(InstanceData {
                name: inst_name,
                net_names,
                model_or_subckt: model_name,
                value,
                params,
                device_prefix,
            });
        }

        let model_or_subckt = tokens.last().cloned().unwrap_or_default();
        let net_names = if tokens.is_empty() {
            vec![]
        } else {
            tokens[..tokens.len() - 1].to_vec()
        };
        Item::Instance(InstanceData {
            name: inst_name,
            net_names,
            model_or_subckt,
            value: None,
            params,
            device_prefix,
        })
    }

    fn model_stmt(&mut self, items: Vec<Item>) -> Item {
        let name = tok(&items[1]).unwrap_or("").to_string();
        let type_name = tok(&items[2]).unwrap_or("").to_string();
        let params = param_assign_values(&items[3..]);
        Item::Model(ModelData {
            name,
            type_name,
            params,
        })
    }

    fn param_stmt(&mut self, items: Vec<Item>) -> Item {
        Item::Params(ParamData {
            params: param_assign_values(&items),
        })
    }

    fn subckt_def(&mut self, items: Vec<Item>) -> Item {
        let subckt_name = tok(&items[1]).unwrap_or("").to_string();
        let mut port_names: Vec<String> = Vec::new();
        let mut subckt_params: IndexMap<String, ParameterValue> = IndexMap::new();
        let mut body_items: Vec<Item> = Vec::new();

        for item in &items[2..] {
            match item {
                Item::Port(_, n) => port_names.push(n.clone()),
                Item::ParamKv(n, v) => {
                    subckt_params.insert(n.clone(), v.clone());
                }
                Item::List(list) => body_items = list.clone(),
                _ => {}
            }
        }

        let result = self.build_subckt(subckt_name.clone(), port_names, subckt_params, body_items);
        self.all_subckts.insert(subckt_name, result.module.clone());
        Item::Subckt(result)
    }

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
                    for (n, v) in &p.params {
                        circuit_params.push(Parameter {
                            name: n.clone(),
                            default_value: Some(v.clone()),
                            ..Default::default()
                        });
                    }
                }
                Item::Include(inc) => directives.push(Directive {
                    kind: DirectiveKind::Include,
                    value: inc.path.clone(),
                    ..Default::default()
                }),
                Item::Global(g) => directives.push(Directive {
                    kind: DirectiveKind::Global,
                    value: g.nets.join(" "),
                    ..Default::default()
                }),
                Item::Option(o) => {
                    for (k, v) in &o.options {
                        directives.push(Directive {
                            kind: DirectiveKind::Option,
                            name: k.clone(),
                            value: v.clone(),
                        });
                    }
                }
                _ => {}
            }
        }

        for (n, v) in &subckt_params {
            circuit_params.push(Parameter {
                name: n.clone(),
                default_value: Some(v.clone()),
                ..Default::default()
            });
        }

        let ports: Vec<Port> = port_names
            .iter()
            .map(|pn| Port {
                name: pn.clone(),
                direction: PortDirection::Inout,
                domain: domain_of(pn),
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

            if device_prefix == "X" {
                let subckt_mod = nested_subckts
                    .get(&inst.model_or_subckt)
                    .or_else(|| self.all_subckts.get(&inst.model_or_subckt));
                let inst_port_names: Vec<String> = match subckt_mod {
                    Some(m) => m.ports.iter().map(|p| p.name.clone()).collect(),
                    None => (0..inst.net_names.len()).map(|i| format!("p{i}")).collect(),
                };
                for (net_name, port_name) in inst.net_names.iter().zip(inst_port_names.iter()) {
                    net_map.add(net_name, &inst.name, port_name);
                }
                module_references.push(ModuleReference {
                    name: inst.name.clone(),
                    module_name: inst.model_or_subckt.clone(),
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
            } else if let Some(template) = port_template {
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
                            domain: domain_of(pn),
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

                let mut params: Vec<Parameter> = Vec::new();
                if let Some(value) = &inst.value {
                    params.push(Parameter {
                        name: "value".to_string(),
                        default_value: Some(ParameterValue {
                            prefixed_value: Some(value.clone()),
                            ..Default::default()
                        }),
                        ..Default::default()
                    });
                }
                for (n, v) in &inst.params {
                    params.push(Parameter {
                        name: n.clone(),
                        default_value: Some(v.clone()),
                        ..Default::default()
                    });
                }

                let mut props = Properties::new();
                if !inst.model_or_subckt.is_empty() {
                    props.insert("model_name".to_string(), inst.model_or_subckt.clone());
                }

                module_references.push(ModuleReference {
                    name: inst.name.clone(),
                    module_name,
                    parameters: params,
                    properties: props,
                    ..Default::default()
                });
            } else {
                for (i, net_name) in inst.net_names.iter().enumerate() {
                    net_map.add(net_name, &inst.name, &format!("p{i}"));
                }
                module_references.push(ModuleReference {
                    name: inst.name.clone(),
                    module_name: inst.model_or_subckt.clone(),
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
        let mut circuit_params: Vec<Parameter> = Vec::new();
        let mut directives: Vec<Directive> = Vec::new();
        let mut analyses: Vec<AnalysisData> = Vec::new();
        let mut output_requests: Vec<OutputRequestData> = Vec::new();
        let mut measurements: Vec<MeasurementData> = Vec::new();
        let mut ic_data: Option<InitialConditionData> = None;
        let mut nodeset_data: Option<InitialConditionData> = None;
        let mut temperatures: Vec<f64> = Vec::new();

        for item in &items {
            match item {
                Item::None => {}
                Item::Subckt(s) => subckt_datas.push(s.clone()),
                Item::Model(m) => {
                    model_defs.insert(m.name.clone(), m.clone());
                }
                Item::Instance(i) => top_instances.push(i.clone()),
                Item::Params(p) => {
                    for (n, v) in &p.params {
                        circuit_params.push(Parameter {
                            name: n.clone(),
                            default_value: Some(v.clone()),
                            ..Default::default()
                        });
                    }
                }
                Item::Include(inc) => directives.push(Directive {
                    kind: DirectiveKind::Include,
                    value: inc.path.clone(),
                    ..Default::default()
                }),
                Item::Lib(l) => {
                    let mut val = l.path.clone();
                    if !l.section.is_empty() {
                        val.push(' ');
                        val.push_str(&l.section);
                    }
                    directives.push(Directive {
                        kind: DirectiveKind::Lib,
                        value: val,
                        ..Default::default()
                    });
                }
                Item::Global(g) => directives.push(Directive {
                    kind: DirectiveKind::Global,
                    value: g.nets.join(" "),
                    ..Default::default()
                }),
                Item::Option(o) => {
                    for (k, v) in &o.options {
                        directives.push(Directive {
                            kind: DirectiveKind::Option,
                            name: k.clone(),
                            value: v.clone(),
                        });
                    }
                }
                Item::Analysis(a) => analyses.push(a.clone()),
                Item::OutputReq(o) => output_requests.push(o.clone()),
                Item::Meas(m) => measurements.push(m.clone()),
                Item::Ic(ic) => {
                    if ic.kind == "ic" {
                        match &mut ic_data {
                            None => ic_data = Some(ic.clone()),
                            Some(existing) => existing.conditions.extend(ic.conditions.clone()),
                        }
                    } else {
                        match &mut nodeset_data {
                            None => nodeset_data = Some(ic.clone()),
                            Some(existing) => existing.conditions.extend(ic.conditions.clone()),
                        }
                    }
                }
                Item::Temp(t) => temperatures.extend(t.temperatures.clone()),
                Item::List(list) => {
                    for sub in list {
                        if let Item::Subckt(s) = sub {
                            subckt_datas.push(s.clone());
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

        for p in &circuit_params {
            let val = match &p.default_value {
                Some(v) => param_value_str(v),
                None => String::new(),
            };
            directives.push(Directive {
                kind: DirectiveKind::Param,
                name: p.name.clone(),
                value: val,
            });
        }

        let has_sim = !analyses.is_empty()
            || !output_requests.is_empty()
            || !measurements.is_empty()
            || ic_data.is_some()
            || nodeset_data.is_some()
            || !temperatures.is_empty();

        let simulation = if has_sim {
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
                measurements: measurements
                    .iter()
                    .map(|m| Measurement {
                        name: m.name.clone(),
                        analysis_type: m.analysis_type.clone(),
                        body: m.body.clone(),
                    })
                    .collect(),
                initial_conditions: ic_data.map(|ic| InitialCondition {
                    conditions: ic.conditions.into_iter().collect(),
                }),
                node_sets: nodeset_data.map(|ns| InitialCondition {
                    conditions: ns.conditions.into_iter().collect(),
                }),
                temperatures,
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

// ---------------------------------------------------------------------------
// Entry points
// ---------------------------------------------------------------------------

fn parse_with(text: &str, mode: Mode) -> Result<Circuit, Error> {
    let cleaned = match mode {
        Mode::Spice => preprocess_spice(text),
        Mode::Hspice => preprocess_hspice(text),
        Mode::Xyce => preprocess_xyce(text),
    };
    let tree = with_engine(mode, |e| e.parse(&cleaned))?;
    let mut t = SpiceTransformer::new(mode);
    match t.transform(&tree) {
        Item::Circuit(c) => Ok(c),
        _ => Err(Error::Validation(
            "spice transform did not produce a circuit".into(),
        )),
    }
}

pub fn parse_spice(text: &str) -> Result<Circuit, Error> {
    parse_with(text, Mode::Spice)
}

pub fn parse_hspice(text: &str) -> Result<Circuit, Error> {
    parse_with(text, Mode::Hspice)
}

pub fn parse_xyce(text: &str) -> Result<Circuit, Error> {
    parse_with(text, Mode::Xyce)
}

// ---------------------------------------------------------------------------
// Writer (shared by spice / hspice / xyce / spectre-spice)
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

fn analysis_kind_to_kw(kind: AnalysisKind) -> &'static str {
    match kind {
        AnalysisKind::Op => ".op",
        AnalysisKind::Dc => ".dc",
        AnalysisKind::Ac => ".ac",
        AnalysisKind::Tran => ".tran",
        AnalysisKind::Noise => ".noise",
        AnalysisKind::Tf => ".tf",
        AnalysisKind::Sens => ".sens",
        AnalysisKind::Pz => ".pz",
        AnalysisKind::Disto => ".disto",
        AnalysisKind::Four => ".four",
        AnalysisKind::Fft => ".fft",
        _ => ".op",
    }
}

fn output_request_kind_to_kw(kind: OutputRequestKind) -> &'static str {
    match kind {
        OutputRequestKind::Print => ".print",
        OutputRequestKind::Plot => ".plot",
        OutputRequestKind::Probe => ".probe",
        OutputRequestKind::Save => "save",
        _ => ".print",
    }
}

pub fn write_spice(circuit: &Circuit) -> String {
    let mut lines: Vec<String> = Vec::new();

    let title = if circuit.name.is_empty() {
        "untitled"
    } else {
        circuit.name.as_str()
    };
    lines.push(format!("* {title}"));

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

    for d in &circuit.directives {
        match d.kind {
            DirectiveKind::Include => lines.push(format!(".include \"{}\"", d.value)),
            DirectiveKind::Lib => lines.push(format!(".lib {}", d.value)),
            DirectiveKind::Global => lines.push(format!(".global {}", d.value)),
            DirectiveKind::Option => lines.push(format!(".option {}={}", d.name, d.value)),
            DirectiveKind::Param => lines.push(format!(".param {}={}", d.name, d.value)),
            _ => {}
        }
    }

    for ext in &circuit.ext_modules {
        let model_type = ext
            .properties
            .get("model_type")
            .cloned()
            .unwrap_or_default();
        if !model_type.is_empty() {
            let param_strs: Vec<String> = ext
                .parameters
                .iter()
                .filter_map(|p| {
                    p.default_value
                        .as_ref()
                        .map(|v| format!("{}={}", p.name, format_param_value(v)))
                })
                .collect();
            let params_text = param_strs.join(" ");
            if params_text.is_empty() {
                lines.push(format!(".model {} {}", ext.name, model_type));
            } else {
                lines.push(format!(
                    ".model {} {} {}",
                    ext.name, model_type, params_text
                ));
            }
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
            lines.push(format_output_request(req));
        }
        for meas in &sim.measurements {
            lines.push(format_measurement(meas));
        }
        if let Some(ic) = &sim.initial_conditions {
            let assigns = ic
                .conditions
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect::<Vec<_>>()
                .join(" ");
            lines.push(format!(".ic {assigns}"));
        }
        if let Some(ns) = &sim.node_sets {
            let assigns = ns
                .conditions
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect::<Vec<_>>()
                .join(" ");
            lines.push(format!(".nodeset {assigns}"));
        }
        if !sim.temperatures.is_empty() {
            let temps = sim
                .temperatures
                .iter()
                .map(|t| format!("{t:?}"))
                .collect::<Vec<_>>()
                .join(" ");
            lines.push(format!(".temp {temps}"));
        }
    }

    lines.push(".end".to_string());
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
    lines.push(format!(".subckt {} {port_names}", module.name));

    for param in &module.parameters {
        if let Some(v) = &param.default_value {
            lines.push(format!(".param {}={}", param.name, format_param_value(v)));
        }
    }

    let inst_nets = connections_to_instance_nets(&module.connections);
    for r in &module.module_references {
        lines.push(write_instance(r, &inst_nets, ext_by_name, mod_by_name));
    }
    lines.push(format!(".ends {}", module.name));
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

fn write_instance(
    r: &ModuleReference,
    inst_nets: &IndexMap<(String, String), String>,
    ext_by_name: &IndexMap<&str, &ExternalModule>,
    mod_by_name: &IndexMap<&str, &Module>,
) -> String {
    let prefix = module_to_prefix(&r.module_name);
    let ext_mod = ext_by_name.get(r.module_name.as_str());

    if let Some(prefix) = prefix {
        let inst_name = r.name.clone();
        let port_order: Vec<String> = match ext_mod {
            Some(em) => em.ports.iter().map(|p| p.name.clone()).collect(),
            // parity: kf-device-pin-template
            None => device_port_template(prefix)
                .unwrap_or_default()
                .iter()
                .map(|pin| pin.to_string())
                .collect(),
        };
        let nets = port_order
            .iter()
            .map(|pn| {
                inst_nets
                    .get(&(inst_name.clone(), pn.clone()))
                    .cloned()
                    .unwrap_or_else(|| "?".to_string())
            })
            .collect::<Vec<_>>()
            .join(" ");

        let model_name = r.properties.get("model_name").cloned().unwrap_or_default();
        let mut value_str = String::new();
        let mut other_params: Vec<String> = Vec::new();
        for param in &r.parameters {
            if param.name == "value" {
                if let Some(v) = &param.default_value {
                    value_str = format_param_value(v);
                }
            } else if let Some(v) = &param.default_value {
                other_params.push(format!("{}={}", param.name, format_param_value(v)));
            }
        }

        let mut parts = vec![inst_name, nets];
        if !model_name.is_empty() {
            parts.push(model_name);
        }
        if !value_str.is_empty() {
            parts.push(value_str);
        }
        parts.extend(other_params);
        return parts.join(" ");
    }

    let inst_name = r.name.clone();
    let subckt_name = r.module_name.clone();

    let port_order: Vec<String> = if let Some(rm) = mod_by_name.get(subckt_name.as_str()) {
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
                .get(&(inst_name.clone(), pn.clone()))
                .cloned()
                .unwrap_or_else(|| "?".to_string())
        })
        .collect::<Vec<_>>()
        .join(" ");

    let mut parts = vec![inst_name, nets, subckt_name];
    for p in &r.parameters {
        if let Some(v) = &p.default_value {
            parts.push(format!("{}={}", p.name, format_param_value(v)));
        }
    }
    parts.join(" ")
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
    let keyword = analysis_kind_to_kw(analysis.kind);
    let mut parts = vec![keyword.to_string()];
    parts.extend(analysis.arguments.clone());
    for (k, v) in &analysis.options {
        parts.push(format!("{k}={v}"));
    }
    parts.join(" ")
}

fn format_output_request(req: &OutputRequest) -> String {
    let keyword = output_request_kind_to_kw(req.kind);
    let mut parts = vec![keyword.to_string()];
    if !req.analysis_type.is_empty() {
        parts.push(req.analysis_type.clone());
    }
    parts.extend(req.variables.clone());
    parts.join(" ")
}

fn format_measurement(meas: &Measurement) -> String {
    let mut parts = vec![".meas".to_string()];
    if !meas.analysis_type.is_empty() {
        parts.push(meas.analysis_type.clone());
    }
    if !meas.name.is_empty() {
        parts.push(meas.name.clone());
    }
    if !meas.body.is_empty() {
        parts.push(meas.body.clone());
    }
    parts.join(" ")
}
