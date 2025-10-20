use crate::{
    auth::{AuthService, LoginCredentials, TokenPair},
    entities::commerce::{customer, customer_address, Customer, CustomerModel},
    errors::ServiceError,
    events::{Event, EventSender},
};
use chrono::Utc;
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

        let customer_id = Uuid::new_v4();

        // Hash password
        let password_hash = self.hash_password(&input.password)?;

        let customer = customer::ActiveModel {
            id: Set(customer_id),
            email: Set(input.email.clone()),
            first_name: Set(input.first_name),
            last_name: Set(input.last_name),
            phone: Set(input.phone),
            accepts_marketing: Set(input.accepts_marketing),
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
        self.store_customer_credentials(customer_id, &input.email, &password_hash)
            .await?;

        self.event_sender
            .send(Event::CustomerCreated(customer_id))
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
        // Find customer
        let customer = Customer::find()
            .filter(customer::Column::Email.eq(&credentials.email))
            .one(&*self.db)
            .await?
            .ok_or_else(|| ServiceError::AuthError("Invalid credentials".to_string()))?;

        if customer.status != customer::CustomerStatus::Active {
            return Err(ServiceError::AuthError("Account is not active".to_string()));
        }

        // Generate auth tokens
        let auth_user = self.create_auth_user(&customer);
        let tokens = self
            .auth_service
            .generate_token(&auth_user)
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
            .send(Event::CustomerUpdated(customer_id))
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
            let mut customer: customer::ActiveModel = Customer::find_by_id(customer_id)
                .one(&txn)
                .await?
                .unwrap()
                .into();

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
        // This would use bcrypt or argon2
        Ok(format!("hashed_{}", password))
    }

    /// Helper: Store customer credentials
    async fn store_customer_credentials(
        &self,
        _customer_id: Uuid,
        _email: &str,
        _password_hash: &str,
    ) -> Result<(), ServiceError> {
        // This would store in auth system
        Ok(())
    }

    /// Helper: Create auth user from customer
    fn create_auth_user(&self, customer: &CustomerModel) -> crate::auth::User {
        crate::auth::User {
            id: customer.id,
            name: format!("{} {}", customer.first_name, customer.last_name),
            email: customer.email.clone(),
            password_hash: String::new(), // Would be fetched from auth store
            tenant_id: None,
            active: customer.status == customer::CustomerStatus::Active,
            created_at: customer.created_at,
            updated_at: customer.updated_at,
        }
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
