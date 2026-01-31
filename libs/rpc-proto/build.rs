fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto_root = "../../proto";

    // Tell Cargo to rerun this build script if proto files change
    println!("cargo:rerun-if-changed={}", proto_root);

    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(
            &["../../proto/auth/v1/auth.proto", "../../proto/user/v1/user.proto"],
            &[proto_root],
        )?;

    Ok(())
}
