#![no_main]

use libfuzzer_sys::fuzz_target;
use arbitrary::Arbitrary;

#[derive(Debug, Arbitrary)]
struct FuzzInput {
    data: String,
}

fuzz_target!(|input: FuzzInput| {
    // Fuzz JSON parsing - ensure no panics on malformed input
    let _ = serde_json::from_str::<serde_json::Value>(&input.data);

    // Fuzz UUID parsing
    let _ = uuid::Uuid::parse_str(&input.data);

    // Fuzz decimal parsing
    let _ = input.data.parse::<rust_decimal::Decimal>();
});
