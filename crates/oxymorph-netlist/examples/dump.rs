fn parse(fmt: &str, text: &str) -> Result<oxymorph_schema::Circuit, oxymorph_netlist::Error> {
    match fmt {
        "spectre" => oxymorph_netlist::parse_spectre(text),
        "spice" => oxymorph_netlist::parse_spice(text),
        "hspice" => oxymorph_netlist::parse_hspice(text),
        "xyce" => oxymorph_netlist::parse_xyce(text),
        "spectre_spice" => oxymorph_netlist::parse_spectre_spice(text),
        "vams" => oxymorph_netlist::parse_vams(text),
        other => panic!("unknown format {other}"),
    }
}
fn write(fmt: &str, c: &oxymorph_schema::Circuit) -> String {
    match fmt {
        "spectre" => oxymorph_netlist::write_spectre(c),
        "spice" | "hspice" | "xyce" | "spectre_spice" => oxymorph_netlist::write_spice(c),
        "vams" => oxymorph_netlist::write_vams(c),
        other => panic!("unknown format {other}"),
    }
}
fn main() {
    let fmt = std::env::args().nth(1).unwrap();
    let path = std::env::args().nth(2).unwrap();
    let out_fmt = std::env::args().nth(3);
    let text = std::fs::read_to_string(&path).unwrap();
    match parse(&fmt, &text) {
        Ok(c) => match out_fmt {
            Some(of) => print!("{}", write(&of, &c)),
            None => println!("{}", c.to_json().unwrap()),
        },
        Err(e) => { eprintln!("ERROR: {e}"); std::process::exit(1); }
    }
}
