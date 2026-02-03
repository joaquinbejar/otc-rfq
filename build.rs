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

    // Generate SBE codecs using IronSBE
    let out_dir = PathBuf::from(std::env::var("OUT_DIR")?);
    let output_path = out_dir.join("sbe_generated.rs");

    match ironsbe_codegen::generate_from_file(&schema_path) {
        Ok(generated_code) => {
            std::fs::write(&output_path, generated_code)?;
            println!("cargo:warning=SBE codecs generated successfully");
        }
        Err(e) => {
            // Write a placeholder module if generation fails
            let placeholder = "// IronSBE codegen failed - using custom implementation\n";
            std::fs::write(&output_path, placeholder)?;
            println!("cargo:warning=SBE codegen failed: {e}, using custom implementation");
        }
    }

    println!("cargo:rerun-if-changed=schemas/sbe/otc-rfq.xml");

    Ok(())
}
