//! VAMS parser + writer, ported from `vams/transformer.py` and `vams/writer.py`.

use indexmap::IndexMap;
use oxymorph_schema::*;

use crate::ast::TreeTransform;
use crate::engine::Engine;
use crate::error::Error;
use crate::helpers::*;
use crate::preprocess::preprocess_vams;

const GRAMMAR: &str = include_str!("../../grammars/vams.lark");

thread_local! {
    static ENGINE: Engine = Engine::lalr(GRAMMAR).expect("vams grammar builds");
}

fn with_engine<R>(f: impl FnOnce(&Engine) -> R) -> R {
    ENGINE.with(|e| f(e))
}

#[derive(Clone, Debug)]
pub struct InstanceData {
    pub module_type: String,
    pub name: String,
    pub named_ports: IndexMap<String, String>,
    pub positional_ports: Vec<String>,
    pub named_params: IndexMap<String, ParameterValue>,
    pub positional_params: Vec<ParameterValue>,
}

#[derive(Clone, Debug)]
pub struct NatureData {
    pub name: String,
    pub attrs: IndexMap<String, ParameterValue>,
}

#[derive(Clone, Debug)]
pub struct DisciplineData {
    pub name: String,
    pub natures: IndexMap<String, String>,
    pub domain: String,
}

#[derive(Clone, Debug)]
pub struct ParamsetData {
    pub name: String,
    pub base_type: String,
    pub params: IndexMap<String, ParameterValue>,
}

#[derive(Clone, Debug)]
pub struct PortDirData {
    pub direction: PortDirection,
    pub names: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct DisciplineDeclData {
    pub discipline: String,
    pub names: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct GroundData {
    pub net: String,
}

#[derive(Clone, Debug)]
pub struct AnalogData {
    pub index: i64,
}

#[derive(Clone, Debug)]
pub enum Override {
    Named(IndexMap<String, ParameterValue>),
    Positional(Vec<ParameterValue>),
}

#[derive(Clone, Debug)]
pub enum Conn {
    Named(IndexMap<String, String>),
    Positional(Vec<String>),
}

#[derive(Clone, Debug)]
pub enum Item {
    Token(String, String),
    Tree(String, Vec<Item>),
    None,
    Circuit(Circuit),
    Pval(ParameterValue),
    ParamKv(String, ParameterValue),
    MapPval(IndexMap<String, ParameterValue>),
    DisciplineBody(IndexMap<String, String>, String),
    StrTuple(String, String),
    Str(String),
    Names(Vec<String>),
    AssignList(Vec<(String, ParameterValue)>),
    TypedAssigns(Vec<(String, ParameterValue, String)>),
    Override(Override),
    Conn(Conn),
    PortDir(PortDirData),
    DisciplineDecl(DisciplineDeclData),
    Ground(GroundData),
    Analog(AnalogData),
    Nature(NatureData),
    Discipline(DisciplineData),
    Paramset(ParamsetData),
    Instance(InstanceData),
    ModuleTuple(String, Module, IndexMap<String, ExternalModule>),
    List(Vec<Item>),
}

fn tok(it: &Item) -> Option<&str> {
    match it {
        Item::Token(_, v) => Some(v),
        _ => None,
    }
}

fn dir_map(s: &str) -> PortDirection {
    match s {
        "input" => PortDirection::Input,
        "output" => PortDirection::Output,
        _ => PortDirection::Inout,
    }
}

pub struct VamsTransformer {
    analog_blocks: Vec<String>,
}

impl VamsTransformer {
    pub fn new(analog_blocks: Vec<String>) -> Self {
        Self { analog_blocks }
    }
}

impl TreeTransform for VamsTransformer {
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
            "pvalue_number" => {
                let v = tok(&items[0]).unwrap_or("");
                Item::Pval(parse_parameter_number(v).unwrap_or_default())
            }
            "pvalue_ident" | "pvalue_string" => Item::Pval(ParameterValue::default()),
            "param_assign" => {
                let n = tok(&items[0]).unwrap_or("").to_string();
                let v = match items.get(1) {
                    Some(Item::Pval(p)) => p.clone(),
                    _ => ParameterValue::default(),
                };
                Item::ParamKv(n, v)
            }
            "param_assign_list" => Item::AssignList(
                items
                    .into_iter()
                    .filter_map(|i| match i {
                        Item::ParamKv(n, v) => Some((n, v)),
                        _ => None,
                    })
                    .collect(),
            ),
            "param_type" => items.into_iter().next().unwrap_or(Item::None),
            "port_list" => Item::Names(
                items
                    .into_iter()
                    .filter_map(|i| match i {
                        Item::Token(_, v) => Some(v),
                        Item::Str(s) => Some(s),
                        _ => None,
                    })
                    .collect(),
            ),
            "port_item" => items.into_iter().next().unwrap_or(Item::None),
            "port_name_list" => Item::Names(
                items
                    .into_iter()
                    .filter_map(|i| match i {
                        Item::Token(_, v) => Some(v),
                        Item::Str(s) => Some(s),
                        _ => None,
                    })
                    .collect(),
            ),
            "port_dir_decl" => {
                let direction = dir_map(tok(&items[0]).unwrap_or(""));
                let names = names_of(&items, 1);
                Item::PortDir(PortDirData { direction, names })
            }
            "discipline_decl" => {
                let discipline = tok(&items[0]).unwrap_or("").to_string();
                let names = names_of(&items, 1);
                Item::DisciplineDecl(DisciplineDeclData { discipline, names })
            }
            "typed_parameter_decl" => {
                let ptype = tok(&items[0]).unwrap_or("").to_string();
                Item::TypedAssigns(typed_assigns(&items, ptype, false))
            }
            "untyped_parameter_decl" => {
                Item::TypedAssigns(typed_assigns(&items, String::new(), false))
            }
            "typed_localparam_decl" => {
                let ptype = tok(&items[0]).unwrap_or("").to_string();
                Item::TypedAssigns(typed_assigns(&items, ptype, true))
            }
            "untyped_localparam_decl" => {
                Item::TypedAssigns(typed_assigns(&items, String::new(), true))
            }
            "ground_stmt" => Item::Ground(GroundData {
                net: tok(&items[0]).unwrap_or("").to_string(),
            }),
            "named_override" => {
                let n = tok(&items[0]).unwrap_or("").to_string();
                let v = match items.get(1) {
                    Some(Item::Pval(p)) => p.clone(),
                    _ => ParameterValue::default(),
                };
                Item::ParamKv(n, v)
            }
            "positional_override" => match items.into_iter().next() {
                Some(Item::Pval(p)) => Item::Pval(p),
                _ => Item::Pval(ParameterValue::default()),
            },
            "param_override" => {
                let mut named = IndexMap::new();
                let mut positional = Vec::new();
                for i in items {
                    match i {
                        Item::ParamKv(n, v) => {
                            named.insert(n, v);
                        }
                        Item::Pval(p) => positional.push(p),
                        _ => {}
                    }
                }
                if named.is_empty() {
                    Item::Override(Override::Positional(positional))
                } else {
                    Item::Override(Override::Named(named))
                }
            }
            "named_conn" => Item::StrTuple(
                tok(&items[0]).unwrap_or("").to_string(),
                tok(&items[1]).unwrap_or("").to_string(),
            ),
            "positional_conn" => Item::Str(tok(&items[0]).unwrap_or("").to_string()),
            "port_conn" => {
                let mut named = IndexMap::new();
                let mut positional = Vec::new();
                for i in items {
                    match i {
                        Item::StrTuple(n, v) => {
                            named.insert(n, v);
                        }
                        Item::Str(s) => positional.push(s),
                        _ => {}
                    }
                }
                if named.is_empty() {
                    Item::Conn(Conn::Positional(positional))
                } else {
                    Item::Conn(Conn::Named(named))
                }
            }
            "instance_stmt" => self.instance_stmt(items),
            "analog_placeholder" => {
                let v = tok(&items[0]).unwrap_or("0");
                let index = v.parse::<f64>().unwrap_or(0.0) as i64;
                Item::Analog(AnalogData { index })
            }
            "nature_body" => {
                let mut attrs = IndexMap::new();
                for i in items {
                    if let Item::ParamKv(n, v) = i {
                        attrs.insert(n, v);
                    }
                }
                Item::MapPval(attrs)
            }
            "nature_attr" => {
                let n = tok(&items[0]).unwrap_or("").to_string();
                let v = match items.get(1) {
                    Some(Item::Pval(p)) => p.clone(),
                    _ => ParameterValue::default(),
                };
                Item::ParamKv(n, v)
            }
            "nature_def" => {
                let name = tok(&items[0]).unwrap_or("").to_string();
                let attrs = match items.get(1) {
                    Some(Item::MapPval(m)) => m.clone(),
                    _ => IndexMap::new(),
                };
                Item::Nature(NatureData { name, attrs })
            }
            "discipline_body" => {
                let mut natures = IndexMap::new();
                let mut domain = String::new();
                for i in items {
                    match i {
                        Item::StrTuple(a, b) => {
                            natures.insert(a, b);
                        }
                        Item::Str(s) => domain = s,
                        _ => {}
                    }
                }
                Item::DisciplineBody(natures, domain)
            }
            "discipline_nature_attr" => Item::StrTuple(
                tok(&items[0]).unwrap_or("").to_string(),
                tok(&items[1]).unwrap_or("").to_string(),
            ),
            "discipline_domain_attr" => Item::Str(tok(&items[0]).unwrap_or("").to_string()),
            "discipline_def" => {
                let name = tok(&items[0]).unwrap_or("").to_string();
                match items.get(1) {
                    Some(Item::DisciplineBody(natures, domain)) => {
                        Item::Discipline(DisciplineData {
                            name,
                            natures: natures.clone(),
                            domain: domain.clone(),
                        })
                    }
                    _ => Item::Discipline(DisciplineData {
                        name,
                        natures: IndexMap::new(),
                        domain: String::new(),
                    }),
                }
            }
            "paramset_body" => {
                let mut params = IndexMap::new();
                for i in items {
                    if let Item::ParamKv(n, v) = i {
                        params.insert(n, v);
                    }
                }
                Item::MapPval(params)
            }
            "paramset_param" => {
                for i in items {
                    if let Item::ParamKv(n, v) = i {
                        return Item::ParamKv(n, v);
                    }
                }
                Item::None
            }
            "paramset_constraint" => Item::None,
            "paramset_def" => {
                let name = tok(&items[0]).unwrap_or("").to_string();
                let base_type = tok(&items[1]).unwrap_or("").to_string();
                let params = match items.get(2) {
                    Some(Item::MapPval(m)) => m.clone(),
                    _ => IndexMap::new(),
                };
                Item::Paramset(ParamsetData {
                    name,
                    base_type,
                    params,
                })
            }
            "module_body" => Item::List(items.into_iter().filter(|i| !is_none(i)).collect()),
            "module_def" => self.module_def(items),
            "start" => self.start(items),
            _ => Item::Tree(name.to_string(), items),
        }
    }
}

fn is_none(i: &Item) -> bool {
    matches!(i, Item::None)
}

fn names_of(items: &[Item], idx: usize) -> Vec<String> {
    match items.get(idx) {
        Some(Item::Names(n)) => n.clone(),
        Some(Item::Str(s)) => vec![s.clone()],
        _ => vec![],
    }
}

fn typed_assigns(
    items: &[Item],
    ptype: String,
    localparam: bool,
) -> Vec<(String, ParameterValue, String)> {
    let mut assigns = Vec::new();
    for item in items {
        if let Item::AssignList(list) = item {
            for (n, v) in list {
                let tag = if localparam {
                    if ptype.is_empty() {
                        "localparam".to_string()
                    } else {
                        format!("localparam:{ptype}")
                    }
                } else {
                    ptype.clone()
                };
                assigns.push((n.clone(), v.clone(), tag));
            }
        }
    }
    assigns
}

impl VamsTransformer {
    fn instance_stmt(&mut self, items: Vec<Item>) -> Item {
        let module_type = tok(&items[0]).unwrap_or("").to_string();
        let mut params: Option<Override> = None;
        let mut inst_name = String::new();
        let mut ports = Conn::Positional(vec![]);

        for item in &items[1..] {
            match item {
                Item::Override(o) if params.is_none() && inst_name.is_empty() => {
                    params = Some(o.clone());
                }
                Item::Token(_, v) if inst_name.is_empty() => inst_name = v.clone(),
                Item::Conn(c) => ports = c.clone(),
                _ => {}
            }
        }

        let (named_params, positional_params) = match params {
            Some(Override::Named(m)) => (m, vec![]),
            Some(Override::Positional(v)) => (IndexMap::new(), v),
            None => (IndexMap::new(), vec![]),
        };
        let (named_ports, positional_ports) = match ports {
            Conn::Named(m) => (m, vec![]),
            Conn::Positional(v) => (IndexMap::new(), v),
        };

        Item::Instance(InstanceData {
            module_type,
            name: inst_name,
            named_ports,
            positional_ports,
            named_params,
            positional_params,
        })
    }

    fn module_def(&mut self, items: Vec<Item>) -> Item {
        let module_name = tok(&items[0]).unwrap_or("").to_string();
        let mut port_names: Vec<String> = Vec::new();
        let mut body_items: Vec<Item> = Vec::new();

        for item in &items[1..] {
            match item {
                Item::Names(n) => port_names = n.clone(),
                Item::List(list) => body_items = list.clone(),
                _ => {}
            }
        }

        let mut port_dirs: IndexMap<String, PortDirection> = IndexMap::new();
        let mut port_disciplines: IndexMap<String, String> = IndexMap::new();
        let mut parameters: Vec<Parameter> = Vec::new();
        let mut instances: Vec<InstanceData> = Vec::new();
        let mut ground_net = String::new();
        let mut analog_block = String::new();
        let mut properties = Properties::new();

        for item in &body_items {
            match item {
                Item::PortDir(pd) => {
                    for n in &pd.names {
                        port_dirs.insert(n.clone(), pd.direction);
                    }
                }
                Item::DisciplineDecl(dd) => {
                    for n in &dd.names {
                        port_disciplines.insert(n.clone(), dd.discipline.clone());
                    }
                }
                Item::TypedAssigns(list) => {
                    for (pname, pval, ptype_tag) in list {
                        let mut props = Properties::new();
                        let ptype;
                        if let Some(rest) = ptype_tag.strip_prefix("localparam") {
                            props.insert("localparam".to_string(), "true".to_string());
                            ptype = rest.strip_prefix(':').unwrap_or("").to_string();
                        } else {
                            ptype = ptype_tag.clone();
                        }
                        if !ptype.is_empty() {
                            props.insert("type".to_string(), ptype);
                        }
                        parameters.push(Parameter {
                            name: pname.clone(),
                            default_value: Some(pval.clone()),
                            properties: props,
                            ..Default::default()
                        });
                    }
                }
                Item::Instance(inst) => instances.push(inst.clone()),
                Item::Ground(g) => ground_net = g.net.clone(),
                Item::Analog(a) => {
                    if a.index >= 0 && (a.index as usize) < self.analog_blocks.len() {
                        analog_block = self.analog_blocks[a.index as usize].clone();
                    }
                }
                _ => {}
            }
        }

        let ports: Vec<Port> = port_names
            .iter()
            .map(|pn| Port {
                name: pn.clone(),
                direction: port_dirs.get(pn).copied().unwrap_or(PortDirection::Inout),
                domain: SignalDomain::Electrical,
                discipline: port_disciplines.get(pn).cloned().unwrap_or_default(),
                ..Default::default()
            })
            .collect();

        if !ground_net.is_empty() {
            properties.insert("ground_net".to_string(), ground_net);
        }
        if !analog_block.is_empty() {
            properties.insert("analog_block".to_string(), analog_block);
        }

        let mut net_map = NetMap::new();
        let mut module_references: Vec<ModuleReference> = Vec::new();
        let ext_modules: IndexMap<String, ExternalModule> = IndexMap::new();

        for port in &ports {
            net_map.add(&port.name, "", &port.name);
        }

        for inst in &instances {
            let params: Vec<Parameter> = if !inst.named_params.is_empty() {
                inst.named_params
                    .iter()
                    .map(|(n, v)| Parameter {
                        name: n.clone(),
                        default_value: Some(v.clone()),
                        ..Default::default()
                    })
                    .collect()
            } else if !inst.positional_params.is_empty() {
                inst.positional_params
                    .iter()
                    .enumerate()
                    .map(|(i, v)| Parameter {
                        name: format!("p{i}"),
                        default_value: Some(v.clone()),
                        ..Default::default()
                    })
                    .collect()
            } else {
                vec![]
            };

            if !inst.named_ports.is_empty() {
                for (port_name, net_name) in &inst.named_ports {
                    net_map.add(net_name, &inst.name, port_name);
                }
            } else if !inst.positional_ports.is_empty() {
                for (i, net_name) in inst.positional_ports.iter().enumerate() {
                    net_map.add(net_name, &inst.name, &format!("p{i}"));
                }
            }

            module_references.push(ModuleReference {
                name: inst.name.clone(),
                module_name: inst.module_type.clone(),
                parameters: params,
                ..Default::default()
            });
        }

        let connections = net_map_to_connections(&net_map);

        let module = Module {
            name: module_name.clone(),
            ports,
            parameters,
            module_references,
            connections,
            properties,
            ..Default::default()
        };

        Item::ModuleTuple(module_name, module, ext_modules)
    }

    fn start(&mut self, items: Vec<Item>) -> Item {
        let mut modules: Vec<Module> = Vec::new();
        let mut ext_modules: IndexMap<String, ExternalModule> = IndexMap::new();

        for item in &items {
            match item {
                Item::ModuleTuple(_, m, ext) => {
                    modules.push(m.clone());
                    ext_modules.extend(ext.clone());
                }
                Item::Nature(n) => {
                    let mut props = Properties::new();
                    props.insert("ext_kind".to_string(), "nature".to_string());
                    for (k, v) in &n.attrs {
                        let val = match &v.prefixed_value {
                            Some(pv) => format_si_value(pv),
                            None => String::new(),
                        };
                        props.insert(format!("nature_{k}"), val);
                    }
                    ext_modules.insert(
                        n.name.clone(),
                        ExternalModule {
                            name: n.name.clone(),
                            kind: ExternalModuleKind::Nature,
                            properties: props,
                            ..Default::default()
                        },
                    );
                }
                Item::Discipline(d) => {
                    let mut props = Properties::new();
                    props.insert("ext_kind".to_string(), "discipline".to_string());
                    for (role, nature_name) in &d.natures {
                        props.insert(format!("discipline_{role}"), nature_name.clone());
                    }
                    if !d.domain.is_empty() {
                        props.insert("discipline_domain".to_string(), d.domain.clone());
                    }
                    ext_modules.insert(
                        d.name.clone(),
                        ExternalModule {
                            name: d.name.clone(),
                            kind: ExternalModuleKind::Discipline,
                            properties: props,
                            ..Default::default()
                        },
                    );
                }
                Item::Paramset(p) => {
                    let params: Vec<Parameter> = p
                        .params
                        .iter()
                        .map(|(n, v)| Parameter {
                            name: n.clone(),
                            default_value: Some(v.clone()),
                            ..Default::default()
                        })
                        .collect();
                    let mut props = Properties::new();
                    props.insert("ext_kind".to_string(), "model".to_string());
                    props.insert("model_type".to_string(), p.base_type.clone());
                    ext_modules.insert(
                        p.name.clone(),
                        ExternalModule {
                            name: p.name.clone(),
                            kind: ExternalModuleKind::Model,
                            parameters: params,
                            properties: props,
                            ..Default::default()
                        },
                    );
                }
                _ => {}
            }
        }

        let top_module = modules.last().map(|m| m.name.clone()).unwrap_or_default();

        Item::Circuit(Circuit {
            name: String::new(),
            top_module,
            modules,
            ext_modules: ext_modules.into_values().collect(),
            ..Default::default()
        })
    }
}

pub fn parse_vams(text: &str) -> Result<Circuit, Error> {
    let pre = preprocess_vams(text);
    let tree = with_engine(|e| e.parse(&pre.text))?;
    let mut t = VamsTransformer::new(pre.analog_blocks);
    match t.transform(&tree) {
        Item::Circuit(c) => Ok(c),
        _ => Err(Error::Validation("vams transform did not produce a circuit".into())),
    }
}

// ---------------------------------------------------------------------------
// Writer
// ---------------------------------------------------------------------------

fn format_param_value(pval: &ParameterValue) -> String {
    match &pval.prefixed_value {
        Some(pv) => format_si_value(pv),
        None => "0".to_string(),
    }
}

pub fn write_vams(circuit: &Circuit) -> String {
    let mut lines: Vec<String> = Vec::new();

    let ext_by_name: IndexMap<&str, &ExternalModule> =
        circuit.ext_modules.iter().map(|e| (e.name.as_str(), e)).collect();
    let mod_by_name: IndexMap<&str, &Module> =
        circuit.modules.iter().map(|m| (m.name.as_str(), m)).collect();

    for d in &circuit.directives {
        match d.kind {
            DirectiveKind::Include => lines.push(format!("`include \"{}\"", d.value)),
            DirectiveKind::Define => lines.push(format!("`define {}", d.value)),
            DirectiveKind::Timescale => lines.push(format!("`timescale {}", d.value)),
            _ => {}
        }
    }

    for ext in &circuit.ext_modules {
        if ext.kind == ExternalModuleKind::Nature {
            lines.push(format!("nature {};", ext.name));
            for (key, val) in &ext.properties {
                if let Some(attr) = key.strip_prefix("nature_") {
                    lines.push(format!("  {attr} = {val};"));
                }
            }
            lines.push("endnature".to_string());
        } else if ext.kind == ExternalModuleKind::Discipline {
            lines.push(format!("discipline {};", ext.name));
            if let Some(domain) = ext.properties.get("discipline_domain") {
                lines.push(format!("  domain {domain};"));
            }
            for (key, val) in &ext.properties {
                if key.starts_with("discipline_") && key != "discipline_domain" {
                    let role = key.strip_prefix("discipline_").unwrap();
                    lines.push(format!("  {role} {val};"));
                }
            }
            lines.push("enddiscipline".to_string());
        }
    }

    for module in &circuit.modules {
        write_module(module, &ext_by_name, &mod_by_name, &mut lines);
    }

    if lines.last().map(|s| !s.is_empty()).unwrap_or(false) {
        lines.push(String::new());
    }
    lines.join("\n")
}

fn write_module(
    module: &Module,
    ext_by_name: &IndexMap<&str, &ExternalModule>,
    mod_by_name: &IndexMap<&str, &Module>,
    lines: &mut Vec<String>,
) {
    let port_names: Vec<String> = module.ports.iter().map(|p| p.name.clone()).collect();
    lines.push(format!("module {} ({});", module.name, port_names.join(", ")));

    let mut dirs_by_dir: IndexMap<String, Vec<String>> = IndexMap::new();
    for port in &module.ports {
        let d = match port.direction {
            PortDirection::Input => "input",
            PortDirection::Output => "output",
            PortDirection::Inout => "inout",
        };
        dirs_by_dir.entry(d.to_string()).or_default().push(port.name.clone());
    }
    for (d, names) in &dirs_by_dir {
        lines.push(format!("  {d} {};", names.join(", ")));
    }

    let mut disc_by_disc: IndexMap<String, Vec<String>> = IndexMap::new();
    for port in &module.ports {
        if !port.discipline.is_empty() {
            disc_by_disc
                .entry(port.discipline.clone())
                .or_default()
                .push(port.name.clone());
        }
    }
    for (disc, names) in &disc_by_disc {
        lines.push(format!("  {disc} {};", names.join(", ")));
    }

    for param in &module.parameters {
        let is_local = param.properties.get("localparam").map(|v| v == "true").unwrap_or(false);
        let kw = if is_local { "localparam" } else { "parameter" };
        let ptype = param.properties.get("type").cloned().unwrap_or_default();
        let val = match &param.default_value {
            Some(v) => format_param_value(v),
            None => "0".to_string(),
        };
        let type_str = if ptype.is_empty() {
            String::new()
        } else {
            format!(" {ptype}")
        };
        lines.push(format!("  {kw}{type_str} {} = {val};", param.name));
    }

    if let Some(ground_net) = module.properties.get("ground_net") {
        lines.push(format!("  ground {ground_net};"));
    }

    let inst_nets = connections_to_instance_nets(&module.connections);
    for r in &module.module_references {
        lines.push(write_instance(r, &inst_nets, ext_by_name, mod_by_name));
    }

    if let Some(analog_block) = module.properties.get("analog_block") {
        lines.push(format!("  {analog_block}"));
    }

    lines.push("endmodule".to_string());
}

fn write_instance(
    r: &ModuleReference,
    inst_nets: &IndexMap<(String, String), String>,
    ext_by_name: &IndexMap<&str, &ExternalModule>,
    mod_by_name: &IndexMap<&str, &Module>,
) -> String {
    let params_str = if r.parameters.is_empty() {
        String::new()
    } else {
        let parts: Vec<String> = r
            .parameters
            .iter()
            .filter_map(|p| {
                p.default_value
                    .as_ref()
                    .map(|v| format!(".{}({})", p.name, format_param_value(v)))
            })
            .collect();
        format!(" #({})", parts.join(", "))
    };

    let port_order: Vec<String> = if let Some(rm) = mod_by_name.get(r.module_name.as_str()) {
        rm.ports.iter().map(|p| p.name.clone()).collect()
    } else if let Some(em) = ext_by_name.get(r.module_name.as_str()) {
        em.ports.iter().map(|p| p.name.clone()).collect()
    } else {
        infer_port_order(r, inst_nets)
    };

    let port_parts: Vec<String> = port_order
        .iter()
        .map(|pn| {
            let net = inst_nets
                .get(&(r.name.clone(), pn.clone()))
                .cloned()
                .unwrap_or_else(|| "?".to_string());
            format!(".{pn}({net})")
        })
        .collect();

    format!(
        "  {}{params_str} {} ({});",
        r.module_name,
        r.name,
        port_parts.join(", ")
    )
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
