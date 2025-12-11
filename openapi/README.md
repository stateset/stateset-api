# OpenAPI Artifacts

This directory holds generated OpenAPI specifications for the HTTP API.

To regenerate the v1 spec:

```sh
cargo run --bin openapi-export
```

Outputs:
- `openapi/openapi.json` – canonical v1 spec (latest)
- `openapi/stateset-api.v1.json` – versioned alias for releases

These files are generated from `src/openapi/mod.rs` (`ApiDocV1`).
