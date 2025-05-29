fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR")?);

    // Compile internal proto files with file descriptor set (includes source comments).
    // The descriptor set is embedded at runtime to support the PluginInfo service.
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .file_descriptor_set_path(out_dir.join("plugin_descriptor.bin"))
        .compile_protos(
            &[
                "proto/grpc_broker.proto",
                "proto/grpc_controller.proto",
                "proto/grpc_stdio.proto",
                "proto/grpc_plugin_info.proto",
            ],
            &["proto/"],
        )?;

    // Compile example greeter proto (used by E2E tests).
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .file_descriptor_set_path(out_dir.join("example_descriptor.bin"))
        .compile_protos(
            &["proto/example_greeter.proto"],
            &["proto/"],
        )?;

    Ok(())
}
