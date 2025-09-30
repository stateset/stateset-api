use validator::Validate;
use crate::errors::ServiceError;

/// Validate any input that implements Validate trait
pub fn validate_input<T: Validate>(input: &T) -> Result<(), ServiceError> {
    input.validate().map_err(|e| {
        ServiceError::InvalidInput(format!("Validation failed: {}", e))
    })
}

/// Validate email format
pub fn validate_email(email: &str) -> Result<(), ServiceError> {
    if !email.contains('@') || email.len() < 3 || email.len() > 256 {
        return Err(ServiceError::InvalidInput("Invalid email format".to_string()));
    }
    Ok(())
}

/// Validate phone number (E.164 format)
pub fn validate_phone(phone: &str) -> Result<(), ServiceError> {
    if !phone.starts_with('+') {
        return Err(ServiceError::InvalidInput("Phone must start with +".to_string()));
    }
    
    let digits = phone.chars().filter(|c| c.is_ascii_digit()).count();
    if digits < 10 || digits > 15 {
        return Err(ServiceError::InvalidInput("Invalid phone number length".to_string()));
    }
    
    Ok(())
}

/// Validate ISO 3166-1 alpha-2 country code
pub fn validate_country_code(code: &str) -> Result<(), ServiceError> {
    if code.len() != 2 || !code.chars().all(|c| c.is_ascii_uppercase()) {
        return Err(ServiceError::InvalidInput(
            "Country code must be 2 uppercase letters (ISO 3166-1)".to_string()
        ));
    }
    Ok(())
}

/// Validate currency code (ISO 4217)
pub fn validate_currency(currency: &str) -> Result<(), ServiceError> {
    if currency.len() != 3 || !currency.chars().all(|c| c.is_ascii_lowercase()) {
        return Err(ServiceError::InvalidInput(
            "Currency must be 3 lowercase letters (ISO 4217)".to_string()
        ));
    }
    Ok(())
}

/// Validate quantity is positive
pub fn validate_quantity(quantity: i32) -> Result<(), ServiceError> {
    if quantity <= 0 {
        return Err(ServiceError::InvalidInput("Quantity must be greater than 0".to_string()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_email() {
        assert!(validate_email("test@example.com").is_ok());
        assert!(validate_email("invalid").is_err());
        // Note: "@example.com" passes basic validation (has @ and length ok)
        // For stricter validation, use the validator crate's #[validate(email)]
    }

    #[test]
    fn test_validate_phone() {
        assert!(validate_phone("+14155551234").is_ok());
        assert!(validate_phone("4155551234").is_err());
        assert!(validate_phone("+1").is_err());
    }

    #[test]
    fn test_validate_country_code() {
        assert!(validate_country_code("US").is_ok());
        assert!(validate_country_code("GB").is_ok());
        assert!(validate_country_code("USA").is_err());
        assert!(validate_country_code("us").is_err());
    }

    #[test]
    fn test_validate_currency() {
        assert!(validate_currency("usd").is_ok());
        assert!(validate_currency("eur").is_ok());
        assert!(validate_currency("USD").is_err());
        assert!(validate_currency("dollar").is_err());
    }
} 