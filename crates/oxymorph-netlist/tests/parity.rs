//! Parity against the committed oracle corpus (`tests/corpus/cases.json`).

use std::path::PathBuf;

use oxymorph_schema::Circuit;

fn hex_decode(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

fn corpus_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/cases.json")
}

fn parse(format: &str, text: &str) -> Result<Circuit, oxymorph_netlist::Error> {
    match format {
        "spice" => oxymorph_netlist::parse_spice(text),
        "hspice" => oxymorph_netlist::parse_hspice(text),
        "xyce" => oxymorph_netlist::parse_xyce(text),
        "spectre" => oxymorph_netlist::parse_spectre(text),
        "spectre_spice" => oxymorph_netlist::parse_spectre_spice(text),
        "vams" => oxymorph_netlist::parse_vams(text),
        other => panic!("unknown format {other}"),
    }
}

fn write(format: &str, c: &Circuit) -> String {
    match format {
        "spice" | "hspice" | "xyce" | "spectre_spice" => oxymorph_netlist::write_spice(c),
        "spectre" => oxymorph_netlist::write_spectre(c),
        "vams" => oxymorph_netlist::write_vams(c),
        other => panic!("unknown format {other}"),
    }
}

#[test]
fn parse_and_write_match_the_oracle() {
    let data = std::fs::read_to_string(corpus_path()).expect("corpus is committed");
    let cases: Vec<serde_json::Value> = serde_json::from_str(&data).expect("valid corpus");

    let mut failures: Vec<String> = Vec::new();
    let mut parse_checked = 0usize;
    let mut write_checked = 0usize;
    let mut proto_checked = 0usize;

    for case in &cases {
        let format = case["format"].as_str().unwrap();
        let name = case["name"].as_str().unwrap();
        let input = case["input"].as_str().unwrap();

        let circuit = match parse(format, input) {
            Ok(c) => c,
            Err(e) => {
                failures.push(format!("{format}/{name}: parse failed: {e}"));
                continue;
            }
        };
        parse_checked += 1;

        let actual: serde_json::Value = serde_json::from_str(&circuit.to_json().unwrap()).unwrap();
        if actual != case["json"] {
            failures.push(format!("{format}/{name}: IR differs from oracle"));
        }

        if let Some(proto_hex) = case["proto"].as_str() {
            let bytes = hex_decode(proto_hex);
            match Circuit::from_proto_bytes(&bytes) {
                Ok(c) if c == circuit => proto_checked += 1,
                Ok(_) => failures.push(format!("{format}/{name}: protobuf interchange differs")),
                Err(e) => failures.push(format!("{format}/{name}: protobuf decode failed: {e}")),
            }
            match Circuit::from_proto_bytes(&circuit.to_proto_bytes()) {
                Ok(c) if c == circuit => {}
                _ => failures.push(format!("{format}/{name}: protobuf round-trip differs")),
            }
        }

        if let Some(writes) = case["writes"].as_object() {
            for (wfmt, expected) in writes {
                let expected = expected.as_str().unwrap();
                let got = write(wfmt, &circuit);
                write_checked += 1;
                if got != expected {
                    failures.push(format!("{format}/{name}: write[{wfmt}] differs"));
                }
            }
        }
    }

    assert!(parse_checked > 300, "corpus too small: {parse_checked}");
    assert!(
        proto_checked > 300,
        "protobuf corpus too small: {proto_checked}"
    );
    if !failures.is_empty() {
        let shown: Vec<String> = failures.iter().take(40).cloned().collect();
        panic!(
            "{} failures ({} parse, {} write, {} proto checked):\n{}",
            failures.len(),
            parse_checked,
            write_checked,
            proto_checked,
            shown.join("\n")
        );
    }
}
