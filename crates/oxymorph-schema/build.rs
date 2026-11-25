fn main() {
    let protos = "proto";
    let names = ["circuit", "simulation", "value"];
    let mut inputs = Vec::new();
    for name in names {
        let input = format!("{protos}/{name}.proto");
        println!("cargo:rerun-if-changed={input}");
        inputs.push(input);
    }
    prost_build::Config::new()
        .compile_protos(&inputs, &[protos])
        .expect("protobuf codegen");
}
