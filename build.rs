//! Build script for OTC RFQ Engine
//!
//! This script handles SBE message generation using IronSBE codegen.

use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Rerun if SBE schema changes
    println!("cargo:rerun-if-changed=schemas/sbe/");

    // Generate SBE codecs
    generate_sbe_codecs()?;

    Ok(())
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
