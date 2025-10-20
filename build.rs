use std::env;
use std::path::PathBuf;
use std::process::Command;

// We'll use chrono to embed the build timestamp
use chrono::Utc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    println!("cargo:warning=OUT_DIR: {}", out_dir.display());

    // Discover and compile all .proto files in the proto directory
    let proto_dir = PathBuf::from("proto");
    let mut proto_files: Vec<PathBuf> = Vec::new();

    for entry in std::fs::read_dir(&proto_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().map(|ext| ext == "proto").unwrap_or(false) {
            println!("cargo:warning=Found proto: {}", path.display());
            proto_files.push(path);
        }
    }

    if proto_files.is_empty() {
        println!(
            "cargo:warning=No .proto files found in {}",
            proto_dir.display()
        );
    } else {
        tonic_build::configure().compile_protos(&proto_files, &["proto"])?;
        println!(
            "cargo:warning=Successfully compiled {} proto files",
            proto_files.len()
        );
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
