//! Every embedded grammar is byte-identical to kfnetlist's at f020af6, so a re-vendor or an edit cannot drift unnoticed.

use sha2::{Digest, Sha256};

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hex = String::new();
    for byte in Sha256::digest(bytes) {
        hex.push_str(&format!("{byte:02x}"));
    }
    hex
}

#[test]
fn grammars_match_kfnetlist() {
    let grammars: [(&str, &[u8], &str); 8] = [
        // kfnetlist f020af6: src/kfnetlist/morphnet/netlist/grammars/common.lark
        (
            "common",
            include_bytes!("../grammars/common.lark"),
            "b53486a4931ca72e8be998409eb17a4434200b77ac2bc5f0833ecc37e0857f3e",
        ),
        // kfnetlist f020af6: src/kfnetlist/morphnet/netlist/grammars/expression.lark
        (
            "expression",
            include_bytes!("../grammars/expression.lark"),
            "6ef37b7b1c6785c31d2b8da3ee1d467fd4d91f6ce01d46d2cccb82717535cb6a",
        ),
        // kfnetlist f020af6: src/kfnetlist/morphnet/netlist/grammars/hspice.lark
        (
            "hspice",
            include_bytes!("../grammars/hspice.lark"),
            "06a3a9fc34626d18ea4f4498f35b1cab9d4dd9180120e7e23401980197499fa1",
        ),
        // kfnetlist f020af6: src/kfnetlist/morphnet/netlist/grammars/spectre.lark
        (
            "spectre",
            include_bytes!("../grammars/spectre.lark"),
            "6a6d3e9abe27c9bcd8ff7a842e4801900160f074a182ceed57aa36d3e2edf4c1",
        ),
        // kfnetlist f020af6: src/kfnetlist/morphnet/netlist/grammars/spice.lark
        (
            "spice",
            include_bytes!("../grammars/spice.lark"),
            "44ee30a69d720a3fce53bd8c4b0cbe61be93fa8c1112d3ee5d38993bc08e4b3e",
        ),
        // kfnetlist f020af6: src/kfnetlist/morphnet/netlist/grammars/systemverilog.lark
        (
            "systemverilog",
            include_bytes!("../grammars/systemverilog.lark"),
            "1367fd1b67a81cfc7ca5d8aebf16f40637fdbec470cb94264c322aed675006b3",
        ),
        // kfnetlist f020af6: src/kfnetlist/morphnet/netlist/grammars/vams.lark
        (
            "vams",
            include_bytes!("../grammars/vams.lark"),
            "828530f8692b9ff65a957eb007fe35bbb9679d54d95b9acfd582c70b491fa61d",
        ),
        // kfnetlist f020af6: src/kfnetlist/morphnet/netlist/grammars/xyce.lark
        (
            "xyce",
            include_bytes!("../grammars/xyce.lark"),
            "bce1551884e181f28b8d360e5d8f87452f691bbfac1f2d60882b825a868cc94d",
        ),
    ];
    for (name, contents, expected_hash) in grammars {
        assert_eq!(
            sha256_hex(contents),
            expected_hash,
            "{name}.lark differs from kfnetlist f020af6"
        );
    }
}
