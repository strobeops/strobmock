use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = PathBuf::from(std::env::var("OUT_DIR")?);

    // Vendored protoc from protobuf-src — no system toolchain required.
    // Safe: build scripts run single-threaded before any threads are spawned.
    unsafe { std::env::set_var("PROTOC", protobuf_src::protoc()) };

    tonic_prost_build::configure()
        .file_descriptor_set_path(out_dir.join("mock_descriptor.bin"))
        .compile_protos(&["proto/mock.proto"], &["proto"])?;

    Ok(())
}
