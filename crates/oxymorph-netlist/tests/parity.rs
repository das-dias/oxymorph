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

fn overrides_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/overrides.json")
}

fn parity_md_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../specs/parity.md")
}

fn load_cases() -> Vec<serde_json::Value> {
    let data = std::fs::read_to_string(corpus_path()).expect("corpus is committed");
    serde_json::from_str(&data).expect("valid corpus")
}

fn load_overrides() -> serde_json::Value {
    let data = std::fs::read_to_string(overrides_path()).expect("overrides are committed");
    serde_json::from_str(&data).expect("valid overrides")
}

fn case_key(case: &serde_json::Value) -> String {
    format!(
        "{}/{}",
        case["format"].as_str().unwrap(),
        case["name"].as_str().unwrap()
    )
}

/// Problems with the overrides: an `r4` id missing from the R4 table of `parity_md`,
/// or a key that is not a corpus case.
fn check_overrides(
    overrides: &serde_json::Value,
    parity_md: &str,
    case_keys: &[String],
) -> Vec<String> {
    let r4_section = parity_md
        .split("## R4.")
        .nth(1)
        .and_then(|rest| rest.split("\n## ").next())
        .unwrap_or("");
    let r4_ids: Vec<&str> = r4_section
        .lines()
        .filter(|line| line.starts_with('|'))
        .filter_map(|line| line.split('|').nth(1))
        .map(str::trim)
        .collect();

    let mut problems = Vec::new();
    for (key, entry) in overrides.as_object().expect("overrides are an object") {
        if !case_keys.contains(key) {
            problems.push(format!("{key}: not a corpus case"));
        }
        match entry["r4"].as_str() {
            Some(id) if r4_ids.contains(&id) => {}
            Some(id) => problems.push(format!("{key}: R4 id {id} is not in specs/parity.md")),
            None => problems.push(format!("{key}: no r4 id")),
        }
    }
    problems
}

struct Expected<'a> {
    json: &'a serde_json::Value,
    /// The oracle protobuf encodes the pre-override IR, so it is only compared when `json` is not overridden.
    compare_oracle_proto: bool,
    writes: Vec<(&'a str, &'a str)>,
}

fn expected_for<'a>(case: &'a serde_json::Value, over: &'a serde_json::Value) -> Expected<'a> {
    let writes = case["writes"].as_object().into_iter().flatten();
    Expected {
        json: over.get("json").unwrap_or(&case["json"]),
        compare_oracle_proto: over.get("json").is_none(),
        writes: writes
            .map(|(format, text)| {
                (
                    format.as_str(),
                    over["writes"].get(format).unwrap_or(text).as_str().unwrap(),
                )
            })
            .collect(),
    }
}

#[test]
fn expected_values_apply_overrides() {
    let case =
        serde_json::json!({"json": {"a": 1}, "writes": {"spice": "old s", "spectre": "old x"}});

    let none = serde_json::json!(null);
    let plain = expected_for(&case, &none);
    assert_eq!(plain.json, &serde_json::json!({"a": 1}));
    assert!(plain.compare_oracle_proto);
    assert_eq!(plain.writes, [("spectre", "old x"), ("spice", "old s")]);

    let write_only = serde_json::json!({"r4": "kf-x", "writes": {"spice": "new s"}});
    let expected = expected_for(&case, &write_only);
    assert_eq!(expected.writes, [("spectre", "old x"), ("spice", "new s")]);
    assert!(expected.compare_oracle_proto);

    let json_override = serde_json::json!({"r4": "kf-x", "json": {"a": 2}});
    let expected = expected_for(&case, &json_override);
    assert_eq!(expected.json, &serde_json::json!({"a": 2}));
    assert!(!expected.compare_oracle_proto);
    assert_eq!(expected.writes, [("spectre", "old x"), ("spice", "old s")]);
}

#[test]
fn override_check_reports_unknown_r4_ids_and_cases() {
    let parity_md = "## R4. Known\n\n| id | x |\n|---|---|\n| kf-known | y |\n\n## R5. Next\n\n| kf-other | z |\n";
    let case_keys = vec!["spice/a".to_string()];

    let valid = serde_json::json!({"spice/a": {"r4": "kf-known"}});
    assert!(check_overrides(&valid, parity_md, &case_keys).is_empty());

    let unknown_r4 = serde_json::json!({"spice/a": {"r4": "kf-other"}});
    assert_eq!(check_overrides(&unknown_r4, parity_md, &case_keys).len(), 1);

    let unknown_case = serde_json::json!({"spice/b": {"r4": "kf-known"}});
    assert_eq!(
        check_overrides(&unknown_case, parity_md, &case_keys).len(),
        1
    );

    let missing_r4 = serde_json::json!({"spice/a": {}});
    assert_eq!(check_overrides(&missing_r4, parity_md, &case_keys).len(), 1);
}

#[test]
fn overrides_reference_r4_rows_and_name_existing_cases() {
    let case_keys: Vec<String> = load_cases().iter().map(case_key).collect();
    let parity_md = std::fs::read_to_string(parity_md_path()).expect("parity spec exists");
    let problems = check_overrides(&load_overrides(), &parity_md, &case_keys);
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn parse_and_write_match_the_oracle() {
    let cases = load_cases();
    let overrides = load_overrides();

    let mut failures: Vec<String> = Vec::new();
    let mut parse_checked = 0usize;
    let mut write_checked = 0usize;
    let mut proto_checked = 0usize;
    let mut proto_overridden = 0usize;

    for case in &cases {
        let format = case["format"].as_str().unwrap();
        let name = case["name"].as_str().unwrap();
        let input = case["input"].as_str().unwrap();
        let over = &overrides[case_key(case)];
        let expected = expected_for(case, over);

        let circuit = match parse(format, input) {
            Ok(c) => c,
            Err(e) => {
                failures.push(format!("{format}/{name}: parse failed: {e}"));
                continue;
            }
        };
        parse_checked += 1;

        let actual: serde_json::Value = serde_json::from_str(&circuit.to_json().unwrap()).unwrap();
        if &actual != expected.json {
            failures.push(format!("{format}/{name}: IR differs from oracle"));
        }

        if let Some(proto_hex) = case["proto"].as_str() {
            if expected.compare_oracle_proto {
                let bytes = hex_decode(proto_hex);
                match Circuit::from_proto_bytes(&bytes) {
                    Ok(c) if c == circuit => proto_checked += 1,
                    Ok(_) => {
                        failures.push(format!("{format}/{name}: protobuf interchange differs"))
                    }
                    Err(e) => {
                        failures.push(format!("{format}/{name}: protobuf decode failed: {e}"))
                    }
                }
            } else {
                proto_overridden += 1;
            }
            match Circuit::from_proto_bytes(&circuit.to_proto_bytes()) {
                Ok(c) if c == circuit => {}
                _ => failures.push(format!("{format}/{name}: protobuf round-trip differs")),
            }
        }

        for (wfmt, expected_text) in &expected.writes {
            let got = write(wfmt, &circuit);
            write_checked += 1;
            if got != *expected_text {
                failures.push(format!("{format}/{name}: write[{wfmt}] differs"));
            }
        }
    }

    assert!(parse_checked > 300, "corpus too small: {parse_checked}");
    assert!(
        proto_checked + proto_overridden > 300,
        "protobuf corpus too small: {proto_checked} checked, {proto_overridden} overridden"
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
