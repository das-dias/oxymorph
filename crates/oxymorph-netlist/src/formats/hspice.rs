//! HSPICE: the SPICE-family base transformer/writer with HSPICE handling.
//! Parsing entry point is `crate::formats::spice::parse_hspice`.

use oxymorph_schema::Circuit;

use crate::formats::spice::write_spice;

pub fn write_hspice(circuit: &Circuit) -> String {
    write_spice(circuit)
}
