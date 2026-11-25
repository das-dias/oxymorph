//! Shared helpers ported from `value_utils.py`, `net_utils.py` and
//! `simulation_data.py`. Kept behaviourally identical to upstream.

use std::sync::OnceLock;

use indexmap::IndexMap;
use oxymorph_schema::{
    AnalysisKind, Connection, OutputRequestKind, ParameterValue, PortReference, PrefixedValue,
    SignalDomain, Siprefix,
};
use regex::Regex;

// ---------------------------------------------------------------------------
// SI suffix <-> SIPrefix
// ---------------------------------------------------------------------------

pub fn si_suffix_to_prefix(suffix: &str) -> Option<Siprefix> {
    Some(match suffix {
        "a" => Siprefix::Atto,
        "f" => Siprefix::Femto,
        "p" => Siprefix::Pico,
        "n" => Siprefix::Nano,
        "u" => Siprefix::Micro,
        "m" => Siprefix::Milli,
        "k" => Siprefix::Kilo,
        "x" => Siprefix::Mega,
        "meg" => Siprefix::Mega,
        "g" => Siprefix::Giga,
        "t" => Siprefix::Tera,
        _ => return None,
    })
}

pub fn prefix_to_si_suffix(prefix: Siprefix) -> &'static str {
    match prefix {
        Siprefix::Atto => "a",
        Siprefix::Femto => "f",
        Siprefix::Pico => "p",
        Siprefix::Nano => "n",
        Siprefix::Micro => "u",
        Siprefix::Milli => "m",
        Siprefix::Kilo => "k",
        Siprefix::Mega => "meg",
        Siprefix::Giga => "g",
        Siprefix::Tera => "t",
        _ => "",
    }
}

pub const MIL_SCALE: f64 = 25.4e-6;

// ---------------------------------------------------------------------------
// Device port templates
// ---------------------------------------------------------------------------

pub fn device_port_template(prefix: &str) -> Option<&'static [&'static str]> {
    Some(match prefix {
        "R" | "C" | "L" | "D" | "V" | "I" => &["p", "n"],
        "Q" => &["c", "b", "e"],
        "M" => &["d", "g", "s", "b"],
        _ => return None,
    })
}

pub fn device_prefix_to_module(prefix: &str) -> Option<&'static str> {
    Some(match prefix {
        "R" => "resistor",
        "C" => "capacitor",
        "L" => "inductor",
        "D" => "diode",
        "Q" => "bjt",
        "M" => "mosfet",
        "V" => "vsource",
        "I" => "isource",
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// SI number parsing / formatting
// ---------------------------------------------------------------------------

fn si_number_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)^([+-]?(?:\d+\.?\d*|\.\d+)(?:[eE][+-]?\d+)?)(meg|mil|[afpnumkgtx])?$")
            .unwrap()
    })
}

pub fn parse_si_number(token: &str) -> Result<PrefixedValue, String> {
    let caps = si_number_re()
        .captures(token)
        .ok_or_else(|| format!("Not a valid SI number: {token:?}"))?;
    let numeric_str = caps.get(1).unwrap().as_str();
    let suffix = caps.get(2).map(|m| m.as_str());
    let value: f64 = numeric_str
        .parse()
        .map_err(|_| format!("Not a valid SI number: {token:?}"))?;
    match suffix {
        None => Ok(PrefixedValue {
            double_value: value,
            prefix: Siprefix::Unspecified,
        }),
        Some(s) => {
            let lower = s.to_lowercase();
            if lower == "mil" {
                return Ok(PrefixedValue {
                    double_value: value * MIL_SCALE,
                    prefix: Siprefix::Unspecified,
                });
            }
            match si_suffix_to_prefix(&lower) {
                Some(prefix) => Ok(PrefixedValue {
                    double_value: value,
                    prefix,
                }),
                None => Err(format!("Unknown SI suffix: {s:?}")),
            }
        }
    }
}

pub fn parse_parameter_number(token: &str) -> Result<ParameterValue, String> {
    Ok(ParameterValue {
        prefixed_value: Some(parse_si_number(token)?),
        ..Default::default()
    })
}

pub fn is_si_number(token: &str) -> bool {
    si_number_re().is_match(token)
}

/// Emulate Python's `f"{x:g}"` (shortest clean representation, 6 significant
/// digits, scientific notation outside `[-4, 6)`).
pub fn python_g(x: f64) -> String {
    if x == 0.0 {
        return "0".to_string();
    }
    if x.is_nan() {
        return "nan".to_string();
    }
    if x.is_infinite() {
        return if x > 0.0 { "inf" } else { "-inf" }.to_string();
    }
    let exp = x.abs().log10().floor() as i32;
    if !(-4..6).contains(&exp) {
        let s = format!("{:.*e}", 5, x);
        let (mant, ex) = s.split_once('e').unwrap();
        let mant = trim_zeros(mant);
        let ex: i32 = ex.parse().unwrap();
        format!("{}e{}{:02}", mant, if ex < 0 { '-' } else { '+' }, ex.abs())
    } else {
        let decimals = (5 - exp).max(0) as usize;
        let s = format!("{:.*}", decimals, x);
        trim_zeros(&s)
    }
}

fn trim_zeros(s: &str) -> String {
    if s.contains('.') {
        let t = s.trim_end_matches('0');
        let t = t.trim_end_matches('.');
        t.to_string()
    } else {
        s.to_string()
    }
}

pub fn format_si_value(pv: &PrefixedValue) -> String {
    let num = pv.double_value;
    let num_str = if num.fract() == 0.0 && num.is_finite() {
        format!("{}", num as i64)
    } else {
        python_g(num)
    };
    if pv.prefix == Siprefix::Unspecified {
        return num_str;
    }
    format!("{}{}", num_str, prefix_to_si_suffix(pv.prefix))
}

// ---------------------------------------------------------------------------
// Signal domains
// ---------------------------------------------------------------------------

/// Cell name → pin name → signal domain.
pub type PinDomains = IndexMap<String, IndexMap<String, SignalDomain>>;

/// `o1`, `o2`, ... are waveguide terminals; `e1`, `vdd`, ... are electrical.
pub fn domain_of(port_name: &str) -> SignalDomain {
    let mut characters = port_name.chars();
    if characters.next() == Some('o')
        && characters
            .next()
            .is_some_and(|character| character.is_ascii_digit())
    {
        SignalDomain::Waveguide
    } else {
        SignalDomain::Electrical
    }
}

// ---------------------------------------------------------------------------
// Net maps
// ---------------------------------------------------------------------------

/// Insertion-ordered net map (Python dict semantics).
#[derive(Default)]
pub struct NetMap {
    map: IndexMap<String, Vec<PortReference>>,
}

impl NetMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, net_name: &str, instance_name: &str, port_name: &str) {
        self.map
            .entry(net_name.to_string())
            .or_default()
            .push(PortReference {
                instance_name: instance_name.to_string(),
                port_name: port_name.to_string(),
            });
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &Vec<PortReference>)> {
        self.map.iter().map(|(k, v)| (k.as_str(), v))
    }
}

pub fn net_map_to_connections(net_map: &NetMap) -> Vec<Connection> {
    let mut connections = Vec::new();
    for (net_name, refs) in net_map.iter() {
        if refs.len() < 2 {
            continue;
        }
        let source = refs[0].clone();
        let domain = net_domain(refs);
        for target in &refs[1..] {
            connections.push(Connection {
                name: net_name.to_string(),
                source: Some(source.clone()),
                target: Some(target.clone()),
                // parity: kf-signal-domains
                domain,
                weight: 0,
                properties: Default::default(),
            });
        }
    }
    connections
}

/// A net touching any waveguide terminal is a waveguide net.
pub fn net_domain(refs: &[PortReference]) -> SignalDomain {
    for port_reference in refs {
        if domain_of(&port_reference.port_name) == SignalDomain::Waveguide {
            return SignalDomain::Waveguide;
        }
    }
    SignalDomain::Electrical
}

pub fn connections_to_instance_nets(
    connections: &[Connection],
) -> IndexMap<(String, String), String> {
    let mut mapping = IndexMap::new();
    for conn in connections {
        if let Some(s) = &conn.source {
            mapping.insert(
                (s.instance_name.clone(), s.port_name.clone()),
                conn.name.clone(),
            );
        }
        if let Some(t) = &conn.target {
            mapping.insert(
                (t.instance_name.clone(), t.port_name.clone()),
                conn.name.clone(),
            );
        }
    }
    mapping
}

// ---------------------------------------------------------------------------
// Simulation kind maps
// ---------------------------------------------------------------------------

pub fn analysis_kind(kind: &str) -> Option<AnalysisKind> {
    Some(match kind {
        "op" => AnalysisKind::Op,
        "dc" => AnalysisKind::Dc,
        "ac" => AnalysisKind::Ac,
        "tran" => AnalysisKind::Tran,
        "noise" => AnalysisKind::Noise,
        "tf" => AnalysisKind::Tf,
        "sens" => AnalysisKind::Sens,
        "pz" => AnalysisKind::Pz,
        "disto" => AnalysisKind::Disto,
        "four" => AnalysisKind::Four,
        "fft" => AnalysisKind::Fft,
        _ => return None,
    })
}

pub fn output_request_kind(kind: &str) -> Option<OutputRequestKind> {
    Some(match kind {
        "print" => OutputRequestKind::Print,
        "plot" => OutputRequestKind::Plot,
        "probe" => OutputRequestKind::Probe,
        "save" => OutputRequestKind::Save,
        _ => return None,
    })
}
