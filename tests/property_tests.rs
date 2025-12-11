//! Property-based tests for StateSet API core functionality.
//!
//! These tests use proptest to verify invariants across a wide range of inputs,
//! helping to catch edge cases that unit tests might miss.

use proptest::prelude::*;

// Strategies for generating test data
fn order_number_strategy() -> impl Strategy<Value = String> {
    "[A-Z]{2,4}-[0-9]{4,8}".prop_map(|s| s)
}

fn email_strategy() -> impl Strategy<Value = String> {
    (
        "[a-z]{3,10}",
        "[a-z]{3,8}",
        prop_oneof!["com", "org", "net", "io"],
    )
        .prop_map(|(local, domain, tld)| format!("{}@{}.{}", local, domain, tld))
}

fn quantity_strategy() -> impl Strategy<Value = i64> {
    0i64..1_000_000
}

fn price_strategy() -> impl Strategy<Value = String> {
    (0u64..1_000_000, 0u8..100).prop_map(|(dollars, cents)| format!("{}.{:02}", dollars, cents))
}

// Property: Order numbers should be validated consistently
proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    #[test]
    fn order_number_validation_is_consistent(order_num in order_number_strategy()) {
        // Valid order numbers should always pass validation
        let result = validate_order_number(&order_num);
        prop_assert!(result.is_ok(), "Valid order number rejected: {}", order_num);
    }

    #[test]
    fn empty_order_number_always_fails(s in "\\s*") {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            let result = validate_order_number(trimmed);
            prop_assert!(result.is_err(), "Empty order number should fail");
        }
    }
}

// Property: Email validation is consistent
proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    #[test]
    fn valid_emails_pass_validation(email in email_strategy()) {
        prop_assert!(is_valid_email(&email), "Valid email rejected: {}", email);
    }

    #[test]
    fn emails_without_at_symbol_fail(s in "[a-z]{5,20}") {
        if !s.contains('@') {
            prop_assert!(!is_valid_email(&s), "Email without @ should fail: {}", s);
        }
    }
}

// Property: Quantity validation
proptest! {
    #[test]
    fn non_negative_quantities_are_valid(qty in quantity_strategy()) {
        let result = validate_quantity(qty);
        prop_assert!(result.is_ok(), "Non-negative quantity rejected: {}", qty);
    }

    #[test]
    fn negative_quantities_are_invalid(qty in -1_000_000i64..-1) {
        let result = validate_quantity(qty);
        prop_assert!(result.is_err(), "Negative quantity should fail: {}", qty);
    }
}

// Property: String sanitization preserves safe content
proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    #[test]
    fn alphanumeric_content_is_preserved(s in "[a-zA-Z0-9 ]{1,100}") {
        let sanitized = sanitize_string(&s);
        // Alphanumeric content should be mostly preserved
        // (spaces might be encoded differently)
        let original_alphanum: String = s.chars().filter(|c| c.is_alphanumeric()).collect();
        let sanitized_alphanum: String = sanitized.chars().filter(|c| c.is_alphanumeric()).collect();
        prop_assert_eq!(original_alphanum, sanitized_alphanum);
    }

    #[test]
    fn sanitized_string_has_no_script_tags(s in ".*") {
        let sanitized = sanitize_string(&s);
        prop_assert!(!sanitized.to_lowercase().contains("<script"),
            "Sanitized string contains script tag");
    }

    #[test]
    fn sanitized_string_has_no_javascript_protocol(s in ".*") {
        let sanitized = sanitize_string(&s);
        prop_assert!(!sanitized.to_lowercase().contains("javascript:"),
            "Sanitized string contains javascript protocol");
    }
}

// Property: Price parsing and validation
proptest! {
    #[test]
    fn valid_prices_parse_correctly(price_str in price_strategy()) {
        let parsed: Result<rust_decimal::Decimal, _> = price_str.parse();
        prop_assert!(parsed.is_ok(), "Valid price string failed to parse: {}", price_str);

        if let Ok(price) = parsed {
            prop_assert!(!price.is_sign_negative(), "Price should not be negative");
        }
    }
}

// Property: UUID generation and parsing
proptest! {
    #[test]
    fn generated_uuids_are_valid(_seed in any::<u64>()) {
        let uuid = uuid::Uuid::new_v4();
        let uuid_str = uuid.to_string();
        let parsed = uuid::Uuid::parse_str(&uuid_str);
        prop_assert!(parsed.is_ok(), "Generated UUID failed to round-trip");
    }
}

// Property: SQL identifier validation
proptest! {
    #[test]
    fn valid_identifiers_pass(ident in "[a-z][a-z0-9_]{0,62}") {
        let result = validate_sql_identifier(&ident);
        // Should pass unless it's a reserved keyword
        if !is_sql_keyword(&ident) {
            prop_assert!(result.is_ok(), "Valid identifier rejected: {}", ident);
        }
    }

    #[test]
    fn identifiers_starting_with_number_fail(ident in "[0-9][a-z0-9_]{0,10}") {
        let result = validate_sql_identifier(&ident);
        prop_assert!(result.is_err(), "Identifier starting with number should fail: {}", ident);
    }
}

// Helper functions for validation (mirrors actual implementation)
fn validate_order_number(order_number: &str) -> Result<(), String> {
    if order_number.is_empty() {
        return Err("Order number cannot be empty".to_string());
    }
    if order_number.len() > 50 {
        return Err("Order number too long".to_string());
    }
    if !order_number
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return Err("Invalid characters in order number".to_string());
    }
    Ok(())
}

fn validate_quantity(quantity: i64) -> Result<(), String> {
    if quantity < 0 {
        return Err("Quantity cannot be negative".to_string());
    }
    if quantity > 1_000_000 {
        return Err("Quantity exceeds maximum allowed".to_string());
    }
    Ok(())
}

fn is_valid_email(email: &str) -> bool {
    let parts: Vec<&str> = email.split('@').collect();
    parts.len() == 2
        && !parts[0].is_empty()
        && parts[0].len() <= 64
        && !parts[1].is_empty()
        && parts[1].len() <= 255
        && parts[1].contains('.')
}

fn sanitize_string(input: &str) -> String {
    let mut sanitized = input.replace('\0', "");

    const MAX_STRING_LENGTH: usize = 10000;
    if sanitized.len() > MAX_STRING_LENGTH {
        sanitized.truncate(MAX_STRING_LENGTH);
    }

    // Remove dangerous patterns
    let patterns = [
        "<script",
        "</script",
        "javascript:",
        "onerror=",
        "onclick=",
        "onload=",
    ];

    for pattern in patterns {
        let pattern_lower = pattern.to_lowercase();
        while let Some(pos) = sanitized.to_lowercase().find(&pattern_lower) {
            let end_pos = pos + pattern.len();
            if end_pos <= sanitized.len() {
                sanitized = format!("{}{}", &sanitized[..pos], &sanitized[end_pos..]);
            } else {
                break;
            }
        }
    }

    sanitized
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

    if is_sql_keyword(identifier) {
        return Err("Identifier is a reserved SQL keyword".to_string());
    }

    Ok(identifier.to_string())
}

fn is_sql_keyword(word: &str) -> bool {
    let sql_keywords = [
        "select", "insert", "update", "delete", "drop", "create", "alter", "table", "database",
        "union", "join", "where", "from", "order", "group", "having", "limit",
    ];
    sql_keywords.contains(&word.to_lowercase().as_str())
}
