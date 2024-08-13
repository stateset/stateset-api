use std::env;
use std::path::PathBuf;

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
            println!("cargo:warning=Proto file does not exist: {}", proto_path.display());
            continue;
        }

        tonic_build::configure()
            .build_server(true)
            .out_dir(&out_dir)
            .compile(&[proto_path], &["proto"])?;
        
        println!("cargo:warning=Successfully compiled: {}", proto_file);
    }

    println!("cargo:warning=Proto compilation completed");
    println!("cargo:rerun-if-changed=proto");

    Ok(())
}