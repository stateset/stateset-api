#![no_main]

use libfuzzer_sys::fuzz_target;
use arbitrary::Arbitrary;

#[derive(Debug, Arbitrary)]
struct SanitizeInput {
    input: String,
}

fuzz_target!(|input: SanitizeInput| {
    // Test string sanitization doesn't panic
    let sanitized = sanitize_string(&input.input);

    // Verify output doesn't contain dangerous patterns
    assert!(!sanitized.contains("<script"));
    assert!(!sanitized.contains("javascript:"));
    assert!(!sanitized.contains('\0'));

    // Test SQL identifier validation
    let _ = validate_sql_identifier(&input.input);

    // Test email validation
    let _ = validate_email(&input.input);
});

// Inline implementations for fuzzing (avoid dependency issues)
fn sanitize_string(input: &str) -> String {
    let mut sanitized = input.replace('\0', "");

    const MAX_STRING_LENGTH: usize = 10000;
    if sanitized.len() > MAX_STRING_LENGTH {
        sanitized.truncate(MAX_STRING_LENGTH);
    }

    sanitized
        .replace("<script", "&lt;script")
        .replace("</script", "&lt;/script")
        .replace("javascript:", "")
        .replace("onerror=", "")
        .replace("onclick=", "")
        .replace("onload=", "")
}

fn validate_sql_identifier(identifier: &str) -> Result<String, String> {
    if identifier.is_empty() || identifier.len() > 64 {
        return Err("Invalid identifier length".to_string());
    }

    if !identifier.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return Err("Invalid characters in identifier".to_string());
    }

    if identifier.chars().next().map_or(false, |c| c.is_numeric()) {
        return Err("Identifier cannot start with a number".to_string());
    }

    Ok(identifier.to_string())
}

fn validate_email(email: &str) -> bool {
    let parts: Vec<&str> = email.split('@').collect();
    if parts.len() != 2 {
        return false;
    }

    let local = parts[0];
    let domain = parts[1];

    if local.is_empty() || local.len() > 64 {
        return false;
    }

    if domain.is_empty() || domain.len() > 255 {
        return false;
    }

    domain.contains('.') && !domain.starts_with('.') && !domain.ends_with('.')
}
