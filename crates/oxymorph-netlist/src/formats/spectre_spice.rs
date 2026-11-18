//! Spectre-SPICE: split on `simulator lang=` and delegate to the spectre or
//! spice parser, then merge (ported from `spectre_spice/parser.py`).

use indexmap::IndexMap;
use oxymorph_schema::*;

use crate::error::Error;
use crate::formats::spice::write_spice;
use crate::formats::{parse_spectre, parse_spice};
use crate::preprocess::preprocess_spectre_spice;

pub fn parse_spectre_spice(text: &str) -> Result<Circuit, Error> {
    let sections = preprocess_spectre_spice(text);

    let mut all_modules: Vec<Module> = Vec::new();
    let mut all_ext_modules: IndexMap<String, ExternalModule> = IndexMap::new();
    let mut all_directives: Vec<Directive> = Vec::new();
    let mut all_analyses: Vec<Analysis> = Vec::new();
    let mut all_output_requests: Vec<OutputRequest> = Vec::new();
    let mut all_measurements: Vec<Measurement> = Vec::new();
    let mut all_temperatures: Vec<f64> = Vec::new();
    let mut ic_conditions: Properties = Properties::new();
    let mut ns_conditions: Properties = Properties::new();

    for section in &sections {
        if section.text.trim().is_empty() {
            continue;
        }
        let circuit = if section.language == "spectre" {
            parse_spectre(&section.text)?
        } else {
            parse_spice(&section.text)?
        };

        all_modules.extend(circuit.modules);
        for ext in circuit.ext_modules {
            all_ext_modules.insert(ext.name.clone(), ext);
        }
        all_directives.extend(circuit.directives);

        if let Some(sim) = circuit.simulation {
            all_analyses.extend(sim.analyses);
            all_output_requests.extend(sim.output_requests);
            all_measurements.extend(sim.measurements);
            all_temperatures.extend(sim.temperatures);
            if let Some(ic) = sim.initial_conditions {
                ic_conditions.extend(ic.conditions);
            }
            if let Some(ns) = sim.node_sets {
                ns_conditions.extend(ns.conditions);
            }
        }
    }

    let top_module = all_modules
        .last()
        .map(|m| m.name.clone())
        .unwrap_or_default();

    let has_sim = !all_analyses.is_empty()
        || !all_output_requests.is_empty()
        || !all_measurements.is_empty()
        || !ic_conditions.is_empty()
        || !ns_conditions.is_empty()
        || !all_temperatures.is_empty();

    let simulation = if has_sim {
        Some(Simulation {
            analyses: all_analyses,
            output_requests: all_output_requests,
            measurements: all_measurements,
            initial_conditions: if ic_conditions.is_empty() {
                None
            } else {
                Some(InitialCondition {
                    conditions: ic_conditions,
                })
            },
            node_sets: if ns_conditions.is_empty() {
                None
            } else {
                Some(InitialCondition {
                    conditions: ns_conditions,
                })
            },
            temperatures: all_temperatures,
            ..Default::default()
        })
    } else {
        None
    };

    Ok(Circuit {
        name: String::new(),
        top_module,
        modules: all_modules,
        ext_modules: all_ext_modules.into_values().collect(),
        directives: all_directives,
        simulation,
        ..Default::default()
    })
}

pub fn write_spectre_spice(circuit: &Circuit) -> String {
    write_spice(circuit)
}
