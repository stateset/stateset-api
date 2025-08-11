use std::env;
use std::path::PathBuf;
use std::process::Command;

// We'll use chrono to embed the build timestamp
use chrono::Utc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    println!("cargo:warning=OUT_DIR: {}", out_dir.display());

    let proto_files = [
        "common.proto",
        "order.proto",
        "inventory.proto",
        "return.proto",
        "warranty.proto",
        "shipment.proto",
        "work_order.proto",
        "billofmaterials.proto",
    ];

    for proto_file in &proto_files {
        println!("cargo:warning=Compiling proto file: {}", proto_file);
        let proto_path = PathBuf::from("proto").join(proto_file);

        if !proto_path.exists() {
            println!(
                "cargo:warning=Proto file does not exist: {}",
                proto_path.display()
            );
            continue;
        }

        tonic_build::configure()
            .compile_protos(&[proto_path], &["proto"])?;

        println!("cargo:warning=Successfully compiled: {}", proto_file);
    }

    println!("cargo:warning=Proto compilation completed");
    println!("cargo:rerun-if-changed=proto");

    // Generate build-time metadata
    let git_hash = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_else(|| "unknown".to_string());

    println!("cargo:rustc-env=GIT_HASH={}", git_hash.trim());
    println!("cargo:rustc-env=BUILD_TIME={}", Utc::now().to_rfc3339());

    // Re-run if the HEAD changes
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/heads");

    Ok(())
}
