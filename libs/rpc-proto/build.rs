fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto_root = "../../proto";

    println!("cargo:rerun-if-changed={}", proto_root);

    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(&["../../proto/user/v1/user.proto"], &[proto_root])?;

    Ok(())
}
