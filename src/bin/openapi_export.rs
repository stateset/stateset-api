use std::{fs, path::PathBuf};

use stateset_api::openapi::ApiDocV1;
use utoipa::OpenApi;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let openapi = ApiDocV1::openapi();
    let json = serde_json::to_string_pretty(&openapi)?;

    let output_dir = PathBuf::from("openapi");
    fs::create_dir_all(&output_dir)?;

    let output_path = output_dir.join("stateset-api.v1.json");
    fs::write(&output_path, json)?;

    println!("OpenAPI spec written to {}", output_path.display());
    Ok(())
}
