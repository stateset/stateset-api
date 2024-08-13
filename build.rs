use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .file_descriptor_set_path(out_dir.join("descriptor.bin"))
        .out_dir("./src/proto")
        .compile(
            &[
                "proto/common.proto",
                "proto/order.proto",
                "proto/inventory.proto",
                "proto/return.proto",
                "proto/warranty.proto",
                "proto/shipment.proto",
                "proto/customer.proto",
                "proto/product.proto",
                "proto/work_order.proto",
            ],
            &["proto"],
        )?;
    Ok(())
}