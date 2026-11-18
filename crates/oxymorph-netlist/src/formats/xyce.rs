//! Xyce: the SPICE-family base transformer/writer with Xyce handling.

use oxymorph_schema::Circuit;

use crate::formats::spice::write_spice;

pub fn write_xyce(circuit: &Circuit) -> String {
    write_spice(circuit)
}
