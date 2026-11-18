fn main() {
    let protos = "../../vendor/morphnet/protos";
    println!("cargo:rerun-if-changed={protos}/circuit.proto");
    println!("cargo:rerun-if-changed={protos}/simulation.proto");
    let inputs = [
        format!("{protos}/circuit.proto"),
        format!("{protos}/simulation.proto"),
    ];
    prost_build::Config::new()
        .compile_protos(&inputs, &[protos.to_string()])
        .expect("protobuf codegen");
}
