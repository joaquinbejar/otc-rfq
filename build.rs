//! Build script for OTC RFQ Engine
//!
//! This script handles:
//! - gRPC protobuf compilation using tonic-build
//! - SBE message generation using IronSBE codegen

use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Rerun if proto files change
    println!("cargo:rerun-if-changed=proto/");
    println!("cargo:rerun-if-changed=schemas/sbe/");

    // Compile gRPC protobufs
    compile_protos()?;

    // Generate SBE codecs
    generate_sbe_codecs()?;

    Ok(())
}

/// Compile Protocol Buffer definitions for gRPC services.
fn compile_protos() -> Result<(), Box<dyn std::error::Error>> {
    let proto_dir = PathBuf::from("proto");

    // Check if proto directory exists
    if !proto_dir.exists() {
        println!("cargo:warning=Proto directory not found, skipping gRPC compilation");
        return Ok(());
    }

    // Find all .proto files recursively
    let proto_files = find_proto_files(&proto_dir)?;

    if proto_files.is_empty() {
        println!("cargo:warning=No proto files found, skipping gRPC compilation");
        return Ok(());
    }

    // Compile all proto files using tonic_prost_build with proper include path
    tonic_prost_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(&proto_files, &[proto_dir])?;

    Ok(())
}

/// Recursively find all .proto files in a directory.
fn find_proto_files(dir: &PathBuf) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let mut proto_files = Vec::new();

    if dir.is_dir() {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                proto_files.extend(find_proto_files(&path)?);
            } else if path.extension().map(|ext| ext == "proto").unwrap_or(false) {
                proto_files.push(path);
            }
        }
    }

    Ok(proto_files)
}

/// Generate SBE message codecs from XML schema.
fn generate_sbe_codecs() -> Result<(), Box<dyn std::error::Error>> {
    let schema_path = PathBuf::from("schemas/sbe/otc-rfq.xml");

    // Check if SBE schema exists
    if !schema_path.exists() {
        println!("cargo:warning=SBE schema not found, skipping SBE code generation");
        return Ok(());
    }

    // TODO: Uncomment when IronSBE codegen is available
    // ironsbe_codegen::generate(
    //     &schema_path,
    //     &PathBuf::from(std::env::var("OUT_DIR")?).join("sbe_messages.rs"),
    // )?;

    println!("cargo:warning=SBE codegen placeholder - implement when IronSBE is ready");

    Ok(())
}
