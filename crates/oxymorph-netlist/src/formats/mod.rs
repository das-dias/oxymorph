pub mod hspice;
pub mod spectre;
pub mod spectre_spice;
pub mod spice;
pub mod vams;
pub mod xyce;

pub use hspice::write_hspice;
pub use spectre::{parse_spectre, write_spectre};
pub use spectre_spice::{parse_spectre_spice, write_spectre_spice};
pub use spice::{parse_hspice, parse_spice, parse_xyce, write_spice};
pub use vams::{parse_vams, write_vams};
pub use xyce::write_xyce;

use oxymorph_schema::Circuit;

use crate::error::Error;

/// Dispatch parse by format name (used by bindings).
pub fn parse_dispatch(format: &str, text: &str) -> Result<Circuit, Error> {
    match format {
        "spice" => parse_spice(text),
        "hspice" => parse_hspice(text),
        "xyce" => parse_xyce(text),
        "spectre" => parse_spectre(text),
        "spectre_spice" => parse_spectre_spice(text),
        "vams" => parse_vams(text),
        other => Err(Error::Validation(format!("unknown format: {other}"))),
    }
}

/// Dispatch write by format name (used by bindings).
pub fn write_dispatch(format: &str, circuit: &Circuit) -> String {
    match format {
        "spectre" => write_spectre(circuit),
        "vams" => write_vams(circuit),
        _ => write_spice(circuit),
    }
}
