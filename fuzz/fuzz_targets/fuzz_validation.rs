#![no_main]

use libfuzzer_sys::fuzz_target;
use arbitrary::Arbitrary;

#[derive(Debug, Arbitrary)]
struct ValidationInput {
    order_number: String,
    quantity: i64,
    price: String,
    email: String,
    phone: String,
}

fuzz_target!(|input: ValidationInput| {
    // Test order number validation
    let _ = validate_order_number(&input.order_number);

    // Test quantity validation (must be non-negative)
    let _ = validate_quantity(input.quantity);

    // Test price parsing and validation
    if let Ok(price) = input.price.parse::<rust_decimal::Decimal>() {
        let _ = validate_price(price);
    }

    // Test email format
    let _ = is_valid_email(&input.email);

    // Test phone format
    let _ = is_valid_phone(&input.phone);
});

fn validate_order_number(order_number: &str) -> Result<(), String> {
    if order_number.is_empty() {
        return Err("Order number cannot be empty".to_string());
    }
    if order_number.len() > 50 {
        return Err("Order number too long".to_string());
    }
    if !order_number.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
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

fn validate_price(price: rust_decimal::Decimal) -> Result<(), String> {
    if price.is_sign_negative() {
        return Err("Price cannot be negative".to_string());
    }
    if price > rust_decimal::Decimal::new(999_999_999, 0) {
        return Err("Price exceeds maximum allowed".to_string());
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

fn is_valid_phone(phone: &str) -> bool {
    let digits: String = phone.chars().filter(|c| c.is_ascii_digit()).collect();
    digits.len() >= 10 && digits.len() <= 15
}
