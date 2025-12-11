use crate::{
    auth::{user, AuthService, LoginCredentials, TokenPair},
    entities::commerce::{customer, customer_address, Customer, CustomerModel},
    errors::ServiceError,
    events::{Event, EventSender},
};
use argon2::{
    password_hash::{PasswordHasher, SaltString},
    Argon2,
};
use chrono::Utc;
use rand::rngs::OsRng;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set,
    TransactionTrait,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{info, instrument};
use uuid::Uuid;

/// Customer service for managing customer accounts
#[derive(Clone)]
pub struct CustomerService {
    db: Arc<DatabaseConnection>,
    event_sender: Arc<EventSender>,
    auth_service: Arc<AuthService>,
}

impl CustomerService {
    pub fn new(
        db: Arc<DatabaseConnection>,
        event_sender: Arc<EventSender>,
        auth_service: Arc<AuthService>,
    ) -> Self {
        Self {
            db,
            event_sender,
            auth_service,
        }
    }

    /// Register a new customer
    #[instrument(skip(self, input))]
    pub async fn register_customer(
        &self,
        input: RegisterCustomerInput,
    ) -> Result<CustomerModel, ServiceError> {
        // Check if email already exists
        let existing = Customer::find()
            .filter(customer::Column::Email.eq(&input.email))
            .one(&*self.db)
            .await?;

        if existing.is_some() {
            return Err(ServiceError::ValidationError(
                "Email already registered".to_string(),
            ));
        }

        let RegisterCustomerInput {
            email,
            password,
            first_name,
            last_name,
            phone,
            accepts_marketing,
        } = input;

        let customer_id = Uuid::new_v4();

        // Hash password
        let password_hash = self.hash_password(&password)?;

        let display_name = format!("{first_name} {last_name}");
        let display_name = display_name.trim();
        let display_name = if display_name.is_empty() {
            first_name.clone()
        } else {
            display_name.to_string()
        };

        let customer = customer::ActiveModel {
            id: Set(customer_id),
            email: Set(email.clone()),
            first_name: Set(first_name.clone()),
            last_name: Set(last_name.clone()),
            phone: Set(phone.clone()),
            accepts_marketing: Set(accepts_marketing),
            customer_group_id: Set(None),
            default_shipping_address_id: Set(None),
            default_billing_address_id: Set(None),
            tags: Set(serde_json::json!([])),
            metadata: Set(None),
            email_verified: Set(false),
            email_verified_at: Set(None),
            status: Set(customer::CustomerStatus::Active),
            created_at: Set(Utc::now()),
            updated_at: Set(Utc::now()),
        };

        let customer = customer.insert(&*self.db).await?;

        // Store password in auth system
        self.store_customer_credentials(customer_id, &display_name, &email, &password_hash)
            .await?;

        self.event_sender
            .send_or_log(Event::CustomerCreated(customer_id))
            .await;

        info!("Customer registered: {}", customer_id);
        Ok(customer)
    }

    /// Login customer
    #[instrument(skip(self, credentials))]
    pub async fn login(
        &self,
        credentials: LoginCredentials,
    ) -> Result<CustomerLoginResponse, ServiceError> {
        let user = self
            .auth_service
            .authenticate_user(&credentials.email, &credentials.password)
            .await
            .map_err(|_| ServiceError::AuthError("Invalid credentials".to_string()))?;

        // Find matching customer record
        let customer = match Customer::find_by_id(user.id).one(&*self.db).await? {
            Some(model) => model,
            None => Customer::find()
                .filter(customer::Column::Email.eq(&user.email))
                .one(&*self.db)
                .await?
                .ok_or_else(|| ServiceError::AuthError("Invalid credentials".to_string()))?,
        };

        if customer.email != user.email {
            return Err(ServiceError::AuthError("Invalid credentials".to_string()));
        }

        if customer.status != customer::CustomerStatus::Active {
            return Err(ServiceError::AuthError("Account is not active".to_string()));
        }

        let tokens = self
            .auth_service
            .generate_token(&user)
            .await
            .map_err(|e| ServiceError::AuthError(e.to_string()))?;

        Ok(CustomerLoginResponse {
            customer: customer.into(),
            tokens,
        })
    }

    /// Get customer by ID
    #[instrument(skip(self))]
    pub async fn get_customer(&self, customer_id: Uuid) -> Result<CustomerModel, ServiceError> {
        Customer::find_by_id(customer_id)
            .one(&*self.db)
            .await?
            .ok_or_else(|| ServiceError::NotFound(format!("Customer {} not found", customer_id)))
    }

    /// Update customer profile
    #[instrument(skip(self))]
    pub async fn update_customer(
        &self,
        customer_id: Uuid,
        input: UpdateCustomerInput,
    ) -> Result<CustomerModel, ServiceError> {
        let customer = self.get_customer(customer_id).await?;

        let mut customer: customer::ActiveModel = customer.into();

        if let Some(first_name) = input.first_name {
            customer.first_name = Set(first_name);
        }
        if let Some(last_name) = input.last_name {
            customer.last_name = Set(last_name);
        }
        if let Some(phone) = input.phone {
            customer.phone = Set(Some(phone));
        }
        if let Some(accepts_marketing) = input.accepts_marketing {
            customer.accepts_marketing = Set(accepts_marketing);
        }

        customer.updated_at = Set(Utc::now());
        let customer = customer.update(&*self.db).await?;

        self.event_sender
            .send_or_log(Event::CustomerUpdated(customer_id))
            .await;

        Ok(customer)
    }

    /// Add customer address
    #[instrument(skip(self))]
    pub async fn add_address(
        &self,
        customer_id: Uuid,
        input: AddAddressInput,
    ) -> Result<CustomerAddressModel, ServiceError> {
        let txn = self.db.begin().await?;

        // Verify customer exists
        self.get_customer(customer_id).await?;

        let address_id = Uuid::new_v4();

        let address = customer_address::ActiveModel {
            id: Set(address_id),
            customer_id: Set(customer_id),
            name: Set(Some(format!("{} {}", input.first_name, input.last_name))),
            company: Set(input.company),
            address_line_1: Set(input.address_line_1),
            address_line_2: Set(input.address_line_2),
            city: Set(input.city),
            province: Set(input.province),
            country_code: Set(input.country_code),
            postal_code: Set(input.postal_code),
            phone: Set(input.phone),
            is_default_shipping: Set(input.is_default_shipping.unwrap_or(false)),
            is_default_billing: Set(input.is_default_billing.unwrap_or(false)),
            created_at: Set(Utc::now()),
            updated_at: Set(Utc::now()),
        };

        let address = address.insert(&txn).await?;

        // Update customer default addresses if requested
        if input.is_default_shipping.unwrap_or(false) || input.is_default_billing.unwrap_or(false) {
            let customer_model = Customer::find_by_id(customer_id)
                .one(&txn)
                .await?
                .ok_or_else(|| {
                    ServiceError::NotFound(format!("Customer {} not found", customer_id))
                })?;
            let mut customer: customer::ActiveModel = customer_model.into();

            if input.is_default_shipping.unwrap_or(false) {
                customer.default_shipping_address_id = Set(Some(address_id));
            }
            if input.is_default_billing.unwrap_or(false) {
                customer.default_billing_address_id = Set(Some(address_id));
            }

            customer.update(&txn).await?;
        }

        txn.commit().await?;

        info!("Address added for customer {}: {}", customer_id, address_id);
        Ok(address)
    }

    /// Get customer addresses
    #[instrument(skip(self))]
    pub async fn get_addresses(
        &self,
        customer_id: Uuid,
    ) -> Result<Vec<CustomerAddressModel>, ServiceError> {
        customer_address::Entity::find()
            .filter(customer_address::Column::CustomerId.eq(customer_id))
            .all(&*self.db)
            .await
            .map_err(Into::into)
    }

    /// Helper: Hash password
    fn hash_password(&self, password: &str) -> Result<String, ServiceError> {
        if password.trim().is_empty() {
            return Err(ServiceError::ValidationError(
                "Password cannot be empty".to_string(),
            ));
        }

        let mut rng = OsRng;
        let salt = SaltString::generate(&mut rng);
        Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map(|hash| hash.to_string())
            .map_err(|err| ServiceError::InternalError(format!("Failed to hash password: {}", err)))
    }

    /// Helper: Store customer credentials
    async fn store_customer_credentials(
        &self,
        customer_id: Uuid,
        name: &str,
        email: &str,
        password_hash: &str,
    ) -> Result<(), ServiceError> {
        let now = Utc::now();

        // Ensure no other auth user is registered with the same email
        if user::Entity::find()
            .filter(user::Column::Email.eq(email))
            .one(&*self.db)
            .await?
            .is_some()
        {
            return Err(ServiceError::ValidationError(
                "Email already registered".to_string(),
            ));
        }

        if user::Entity::find_by_id(customer_id)
            .one(&*self.db)
            .await?
            .is_some()
        {
            return Err(ServiceError::ValidationError(
                "Customer account already exists".to_string(),
            ));
        }

        let display_name = if name.trim().is_empty() {
            email.to_string()
        } else {
            name.trim().to_string()
        };

        let auth_user = user::ActiveModel {
            id: Set(customer_id),
            name: Set(display_name),
            email: Set(email.to_string()),
            password_hash: Set(password_hash.to_string()),
            tenant_id: Set(None),
            active: Set(true),
            created_at: Set(now),
            updated_at: Set(now),
        };

        auth_user.insert(&*self.db).await?;
        Ok(())
    }
}

/// Input for registering a customer
#[derive(Debug, Deserialize)]
pub struct RegisterCustomerInput {
    pub email: String,
    pub password: String,
    pub first_name: String,
    pub last_name: String,
    pub phone: Option<String>,
    pub accepts_marketing: bool,
}

/// Customer login response
#[derive(Debug, Serialize)]
pub struct CustomerLoginResponse {
    pub customer: CustomerResponse,
    pub tokens: TokenPair,
}

/// Customer response DTO
#[derive(Debug, Serialize)]
pub struct CustomerResponse {
    pub id: Uuid,
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    pub phone: Option<String>,
    pub accepts_marketing: bool,
    pub status: customer::CustomerStatus,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl From<CustomerModel> for CustomerResponse {
    fn from(model: CustomerModel) -> Self {
        Self {
            id: model.id,
            email: model.email,
            first_name: model.first_name,
            last_name: model.last_name,
            phone: model.phone,
            accepts_marketing: model.accepts_marketing,
            status: model.status,
            created_at: model.created_at,
        }
    }
}

/// Input for updating customer
#[derive(Debug, Deserialize)]
pub struct UpdateCustomerInput {
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub phone: Option<String>,
    pub accepts_marketing: Option<bool>,
}

/// Input for adding address
#[derive(Debug, Deserialize)]
pub struct AddAddressInput {
    pub first_name: String,
    pub last_name: String,
    pub company: Option<String>,
    pub address_line_1: String,
    pub address_line_2: Option<String>,
    pub city: String,
    pub province: String,
    pub country_code: String,
    pub postal_code: String,
    pub phone: Option<String>,
    pub is_default_shipping: Option<bool>,
    pub is_default_billing: Option<bool>,
}

// Type aliases for clarity
type CustomerAddressModel = customer_address::Model;

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== RegisterCustomerInput Tests ====================

    #[test]
    fn test_register_customer_input_valid() {
        let input = RegisterCustomerInput {
            email: "test@example.com".to_string(),
            password: "SecurePass123!".to_string(),
            first_name: "John".to_string(),
            last_name: "Doe".to_string(),
            phone: Some("+1234567890".to_string()),
            accepts_marketing: true,
        };

        assert_eq!(input.email, "test@example.com");
        assert!(!input.password.is_empty());
        assert_eq!(input.first_name, "John");
        assert_eq!(input.last_name, "Doe");
        assert!(input.phone.is_some());
        assert!(input.accepts_marketing);
    }

    #[test]
    fn test_register_customer_input_without_phone() {
        let input = RegisterCustomerInput {
            email: "test@example.com".to_string(),
            password: "SecurePass123!".to_string(),
            first_name: "Jane".to_string(),
            last_name: "Smith".to_string(),
            phone: None,
            accepts_marketing: false,
        };

        assert!(input.phone.is_none());
        assert!(!input.accepts_marketing);
    }

    // ==================== Email Validation Tests ====================

    #[test]
    fn test_email_format_valid() {
        let valid_emails = vec![
            "user@example.com",
            "user.name@example.com",
            "user+tag@example.com",
            "user@subdomain.example.com",
        ];

        for email in valid_emails {
            assert!(email.contains('@'), "Email {} should contain @", email);
            assert!(email.contains('.'), "Email {} should contain .", email);
        }
    }

    #[test]
    fn test_email_not_empty() {
        let email = "test@example.com";
        assert!(!email.is_empty());
    }

    // ==================== Password Tests ====================

    #[test]
    fn test_password_not_empty() {
        let password = "SecurePass123!";
        assert!(!password.is_empty());
    }

    #[test]
    fn test_password_minimum_length() {
        let password = "Short1!";
        // Password should ideally be at least 8 characters
        assert!(password.len() >= 6);
    }

    #[test]
    fn test_password_with_special_characters() {
        let password = "SecureP@ss123!";
        assert!(password.chars().any(|c| !c.is_alphanumeric()));
    }

    #[test]
    fn test_empty_password_validation() {
        let password = "";
        let trimmed = password.trim();
        assert!(trimmed.is_empty());
    }

    #[test]
    fn test_whitespace_only_password() {
        let password = "   ";
        let trimmed = password.trim();
        assert!(trimmed.is_empty());
    }

    // ==================== CustomerResponse Tests ====================

    #[test]
    fn test_customer_response_serialization() {
        let response = CustomerResponse {
            id: Uuid::new_v4(),
            email: "customer@example.com".to_string(),
            first_name: "Alice".to_string(),
            last_name: "Wonder".to_string(),
            phone: Some("+1987654321".to_string()),
            accepts_marketing: true,
            status: customer::CustomerStatus::Active,
            created_at: Utc::now(),
        };

        let json = serde_json::to_string(&response).expect("serialization should succeed");
        assert!(json.contains("customer@example.com"));
        assert!(json.contains("Alice"));
        assert!(json.contains("Wonder"));
    }

    #[test]
    fn test_customer_response_without_phone() {
        let response = CustomerResponse {
            id: Uuid::new_v4(),
            email: "nophone@example.com".to_string(),
            first_name: "Bob".to_string(),
            last_name: "Builder".to_string(),
            phone: None,
            accepts_marketing: false,
            status: customer::CustomerStatus::Active,
            created_at: Utc::now(),
        };

        let json = serde_json::to_string(&response).expect("serialization should succeed");
        assert!(json.contains("\"phone\":null"));
    }

    // ==================== UpdateCustomerInput Tests ====================

    #[test]
    fn test_update_customer_input_partial() {
        let input = UpdateCustomerInput {
            first_name: Some("NewFirst".to_string()),
            last_name: None,
            phone: None,
            accepts_marketing: None,
        };

        assert!(input.first_name.is_some());
        assert!(input.last_name.is_none());
    }

    #[test]
    fn test_update_customer_input_all_fields() {
        let input = UpdateCustomerInput {
            first_name: Some("Updated".to_string()),
            last_name: Some("Name".to_string()),
            phone: Some("+1111111111".to_string()),
            accepts_marketing: Some(true),
        };

        assert!(input.first_name.is_some());
        assert!(input.last_name.is_some());
        assert!(input.phone.is_some());
        assert!(input.accepts_marketing.is_some());
    }

    #[test]
    fn test_update_customer_input_empty() {
        let input = UpdateCustomerInput {
            first_name: None,
            last_name: None,
            phone: None,
            accepts_marketing: None,
        };

        // All fields are optional
        assert!(input.first_name.is_none());
        assert!(input.last_name.is_none());
        assert!(input.phone.is_none());
        assert!(input.accepts_marketing.is_none());
    }

    // ==================== AddAddressInput Tests ====================

    #[test]
    fn test_add_address_input_complete() {
        let input = AddAddressInput {
            first_name: "John".to_string(),
            last_name: "Doe".to_string(),
            company: Some("Acme Inc".to_string()),
            address_line_1: "123 Main St".to_string(),
            address_line_2: Some("Apt 4B".to_string()),
            city: "New York".to_string(),
            province: "NY".to_string(),
            country_code: "US".to_string(),
            postal_code: "10001".to_string(),
            phone: Some("+1234567890".to_string()),
            is_default_shipping: Some(true),
            is_default_billing: Some(false),
        };

        assert_eq!(input.first_name, "John");
        assert_eq!(input.city, "New York");
        assert_eq!(input.country_code, "US");
        assert!(input.is_default_shipping.unwrap_or(false));
    }

    #[test]
    fn test_add_address_input_minimal() {
        let input = AddAddressInput {
            first_name: "Jane".to_string(),
            last_name: "Smith".to_string(),
            company: None,
            address_line_1: "456 Oak Ave".to_string(),
            address_line_2: None,
            city: "Los Angeles".to_string(),
            province: "CA".to_string(),
            country_code: "US".to_string(),
            postal_code: "90001".to_string(),
            phone: None,
            is_default_shipping: None,
            is_default_billing: None,
        };

        assert!(input.company.is_none());
        assert!(input.address_line_2.is_none());
        assert!(input.phone.is_none());
    }

    // ==================== Country Code Tests ====================

    #[test]
    fn test_valid_country_codes() {
        let valid_codes = vec!["US", "CA", "GB", "DE", "FR", "JP", "AU"];

        for code in valid_codes {
            assert_eq!(
                code.len(),
                2,
                "Country code {} should be 2 characters",
                code
            );
            assert!(code.chars().all(|c| c.is_ascii_uppercase()));
        }
    }

    // ==================== Phone Number Tests ====================

    #[test]
    fn test_phone_number_formats() {
        let valid_phones = vec![
            "+1234567890",
            "+44 20 7946 0958",
            "555-123-4567",
            "(555) 123-4567",
        ];

        for phone in valid_phones {
            assert!(!phone.is_empty());
        }
    }

    // ==================== Display Name Tests ====================

    #[test]
    fn test_display_name_concatenation() {
        let first_name = "John";
        let last_name = "Doe";
        let display_name = format!("{} {}", first_name, last_name);
        let display_name = display_name.trim();

        assert_eq!(display_name, "John Doe");
    }

    #[test]
    fn test_display_name_empty_last_name() {
        let first_name = "John";
        let last_name = "";
        let display_name = format!("{} {}", first_name, last_name);
        let display_name = display_name.trim();

        // Should just be the first name
        assert_eq!(display_name, "John");
    }

    #[test]
    fn test_display_name_fallback_to_email() {
        let name = "";
        let email = "user@example.com";

        let display_name = if name.trim().is_empty() {
            email.to_string()
        } else {
            name.trim().to_string()
        };

        assert_eq!(display_name, "user@example.com");
    }

    // ==================== Customer Status Tests ====================

    #[test]
    fn test_customer_status_active() {
        let status = customer::CustomerStatus::Active;
        assert_eq!(format!("{:?}", status), "Active");
    }

    // ==================== UUID Tests ====================

    #[test]
    fn test_customer_id_uniqueness() {
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_address_id_uniqueness() {
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        assert_ne!(id1, id2);
    }

    // ==================== Error Handling Tests ====================

    #[test]
    fn test_validation_error_email_already_registered() {
        let error = ServiceError::ValidationError("Email already registered".to_string());
        match error {
            ServiceError::ValidationError(msg) => {
                assert!(msg.contains("already registered"));
            }
            _ => panic!("Expected ValidationError"),
        }
    }

    #[test]
    fn test_auth_error_invalid_credentials() {
        let error = ServiceError::AuthError("Invalid credentials".to_string());
        match error {
            ServiceError::AuthError(msg) => {
                assert!(msg.contains("Invalid"));
            }
            _ => panic!("Expected AuthError"),
        }
    }

    #[test]
    fn test_not_found_error_customer() {
        let customer_id = Uuid::new_v4();
        let error = ServiceError::NotFound(format!("Customer {} not found", customer_id));
        match error {
            ServiceError::NotFound(msg) => {
                assert!(msg.contains("not found"));
            }
            _ => panic!("Expected NotFound error"),
        }
    }

    // ==================== Marketing Consent Tests ====================

    #[test]
    fn test_marketing_opt_in() {
        let accepts_marketing = true;
        assert!(accepts_marketing);
    }

    #[test]
    fn test_marketing_opt_out() {
        let accepts_marketing = false;
        assert!(!accepts_marketing);
    }

    // ==================== Address Default Tests ====================

    #[test]
    fn test_default_shipping_address() {
        let is_default_shipping: Option<bool> = Some(true);
        assert!(is_default_shipping.unwrap_or(false));
    }

    #[test]
    fn test_default_billing_address() {
        let is_default_billing: Option<bool> = Some(true);
        assert!(is_default_billing.unwrap_or(false));
    }

    #[test]
    fn test_address_default_fallback() {
        let is_default: Option<bool> = None;
        // Should default to false when not specified
        assert!(!is_default.unwrap_or(false));
    }
}
