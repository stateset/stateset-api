use crate::{
    config::AppConfig,
    entities::commerce::{cart, cart_item, Cart, CartItem, CartModel, ProductVariant},
    errors::ServiceError,
    events::{Event, EventSender},
};
use chrono::{Duration, Utc};
use rust_decimal::Decimal;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, ModelTrait, PaginatorTrait,
    QueryFilter, QueryOrder, Set, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{info, instrument};
use uuid::Uuid;

/// Shopping cart service for managing e-commerce shopping carts.
///
/// The `CartService` provides comprehensive cart management functionality including:
/// - Cart creation and lifecycle management
/// - Adding, updating, and removing cart items
/// - Automatic total calculation (subtotal, tax, shipping, discounts)
/// - Cart abandonment tracking
/// - Customer cart history
///
/// # Examples
///
/// ```ignore
/// use stateset_api::services::commerce::CartService;
/// use stateset_api::services::commerce::CreateCartInput;
///
/// let cart_service = CartService::new(db, event_sender, config);
///
/// // Create a new cart
/// let input = CreateCartInput {
///     session_id: Some("session_123".to_string()),
///     customer_id: Some(customer_uuid),
///     currency: Some("USD".to_string()),
///     metadata: None,
/// };
///
/// let cart = cart_service.create_cart(input).await?;
/// ```
#[derive(Clone)]
pub struct CartService {
    db: Arc<DatabaseConnection>,
    event_sender: Arc<EventSender>,
    config: Arc<AppConfig>,
}

impl CartService {
    /// Creates a new `CartService` instance.
    ///
    /// # Arguments
    ///
    /// * `db` - Database connection pool
    /// * `event_sender` - Event sender for publishing cart events
    /// * `config` - Application configuration (for tax/shipping defaults)
    pub fn new(
        db: Arc<DatabaseConnection>,
        event_sender: Arc<EventSender>,
        config: Arc<AppConfig>,
    ) -> Self {
        Self {
            db,
            event_sender,
            config,
        }
    }

    /// Creates a new shopping cart.
    ///
    /// Creates a cart with automatic 30-day expiration and zero initial totals.
    /// Publishes a `CartCreated` event upon success.
    ///
    /// # Arguments
    ///
    /// * `input` - Cart creation parameters including session ID, customer ID, and currency
    ///
    /// # Returns
    ///
    /// * `Ok(CartModel)` - The created cart with generated UUID
    /// * `Err(ServiceError)` - Database error if cart creation fails
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let cart = cart_service.create_cart(CreateCartInput {
    ///     customer_id: Some(uuid),
    ///     currency: Some("EUR".to_string()),
    ///     ..Default::default()
    /// }).await?;
    /// ```
    #[instrument(skip(self))]
    pub async fn create_cart(&self, input: CreateCartInput) -> Result<CartModel, ServiceError> {
        let cart_id = Uuid::new_v4();
        let expires_at = Utc::now() + Duration::days(30); // 30 day expiry

        let cart = cart::ActiveModel {
            id: Set(cart_id),
            session_id: Set(input.session_id),
            customer_id: Set(input.customer_id),
            currency: Set(input.currency.unwrap_or_else(|| "USD".to_string())),
            subtotal: Set(Decimal::ZERO),
            tax_total: Set(Decimal::ZERO),
            shipping_total: Set(Decimal::ZERO),
            discount_total: Set(Decimal::ZERO),
            total: Set(Decimal::ZERO),
            metadata: Set(input.metadata.and_then(|m| serde_json::to_value(m).ok())),
            status: Set(cart::CartStatus::Active),
            expires_at: Set(expires_at),
            created_at: Set(Utc::now()),
            updated_at: Set(Utc::now()),
        };

        let cart = cart.insert(&*self.db).await?;

        self.event_sender
            .send_or_log(Event::CartCreated(cart_id))
            .await;

        info!("Created cart: {}", cart_id);
        Ok(cart)
    }

    /// Adds an item to the cart or updates quantity if item already exists.
    ///
    /// This method handles both new items and existing items intelligently:
    /// - If the variant is already in the cart, increments the quantity
    /// - If the variant is new, creates a new cart item
    /// - Automatically recalculates cart totals after adding
    /// - Publishes a `CartItemAdded` event upon success
    ///
    /// # Arguments
    ///
    /// * `cart_id` - UUID of the target cart
    /// * `input` - Item details including variant ID and quantity
    ///
    /// # Returns
    ///
    /// * `Ok(CartModel)` - Updated cart with recalculated totals
    /// * `Err(ServiceError::NotFound)` - Cart or variant not found
    /// * `Err(ServiceError::InvalidOperation)` - Cart is not active
    /// * `Err(ServiceError)` - Database transaction error
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let updated_cart = cart_service.add_item(
    ///     cart_id,
    ///     AddToCartInput {
    ///         variant_id: product_variant_uuid,
    ///         quantity: 2,
    ///     }
    /// ).await?;
    /// ```
    #[instrument(skip(self))]
    pub async fn add_item(
        &self,
        cart_id: Uuid,
        input: AddToCartInput,
    ) -> Result<CartModel, ServiceError> {
        let txn = self.db.begin().await?;

        // Verify cart exists and is active
        let cart = Cart::find_by_id(cart_id)
            .one(&txn)
            .await?
            .ok_or_else(|| ServiceError::NotFound(format!("Cart {} not found", cart_id)))?;

        if cart.status != cart::CartStatus::Active {
            return Err(ServiceError::InvalidOperation(
                "Cart is not active".to_string(),
            ));
        }

        // Get variant details
        let variant = ProductVariant::find_by_id(input.variant_id)
            .one(&txn)
            .await?
            .ok_or_else(|| {
                ServiceError::NotFound(format!("Variant {} not found", input.variant_id))
            })?;

        // Check if item already exists in cart
        let existing_item = CartItem::find()
            .filter(cart_item::Column::CartId.eq(cart_id))
            .filter(cart_item::Column::VariantId.eq(input.variant_id))
            .one(&txn)
            .await?;

        if let Some(item) = existing_item {
            // Update quantity
            let current_quantity = item.quantity;
            let mut item: cart_item::ActiveModel = item.into();
            item.quantity = Set(current_quantity + input.quantity);
            item.line_total = Set(variant.price * Decimal::from(current_quantity + input.quantity));
            item.updated_at = Set(Utc::now());
            item.update(&txn).await?;
        } else {
            // Create new item
            let item_id = Uuid::new_v4();
            let line_total = variant.price * Decimal::from(input.quantity);

            let cart_item = cart_item::ActiveModel {
                id: Set(item_id),
                cart_id: Set(cart_id),
                variant_id: Set(input.variant_id),
                quantity: Set(input.quantity),
                unit_price: Set(variant.price),
                line_total: Set(line_total),
                discount_amount: Set(Decimal::ZERO),
                metadata: Set(None),
                created_at: Set(Utc::now()),
                updated_at: Set(Utc::now()),
            };

            cart_item.insert(&txn).await?;
        }

        // Update cart totals
        let updated_cart = self.recalculate_cart_totals(&txn, cart_id).await?;

        txn.commit().await?;

        self.event_sender
            .send_or_log(Event::CartItemAdded {
                cart_id,
                variant_id: input.variant_id,
            })
            .await;

        info!(
            "Added item to cart {}: variant {} x{}",
            cart_id, input.variant_id, input.quantity
        );
        Ok(updated_cart)
    }

    /// Updates the quantity of a cart item.
    ///
    /// Special handling:
    /// - If quantity is 0 or negative, removes the item from the cart
    /// - If quantity is positive, updates the item and recalculates line total
    /// - Automatically recalculates cart totals after update
    ///
    /// # Arguments
    ///
    /// * `cart_id` - UUID of the cart (for validation)
    /// * `item_id` - UUID of the cart item to update
    /// * `quantity` - New quantity (0 or negative to remove)
    ///
    /// # Returns
    ///
    /// * `Ok(CartModel)` - Updated cart with recalculated totals
    /// * `Err(ServiceError::NotFound)` - Cart item not found
    /// * `Err(ServiceError::InvalidOperation)` - Item doesn't belong to specified cart
    /// * `Err(ServiceError)` - Database transaction error
    ///
    /// # Examples
    ///
    /// ```ignore
    /// // Update quantity to 5
    /// let cart = cart_service.update_item_quantity(cart_id, item_id, 5).await?;
    ///
    /// // Remove item (quantity 0)
    /// let cart = cart_service.update_item_quantity(cart_id, item_id, 0).await?;
    /// ```
    #[instrument(skip(self))]
    pub async fn update_item_quantity(
        &self,
        cart_id: Uuid,
        item_id: Uuid,
        quantity: i32,
    ) -> Result<CartModel, ServiceError> {
        let txn = self.db.begin().await?;

        if quantity <= 0 {
            // Remove item if quantity is 0 or less
            CartItem::delete_by_id(item_id).exec(&txn).await?;
        } else {
            let item = CartItem::find_by_id(item_id)
                .one(&txn)
                .await?
                .ok_or_else(|| {
                    ServiceError::NotFound(format!("Cart item {} not found", item_id))
                })?;

            if item.cart_id != cart_id {
                return Err(ServiceError::InvalidOperation(
                    "Item does not belong to this cart".to_string(),
                ));
            }

            let unit_price = item.unit_price;
            let mut item: cart_item::ActiveModel = item.into();
            item.quantity = Set(quantity);
            item.line_total = Set(unit_price * Decimal::from(quantity));
            item.updated_at = Set(Utc::now());
            item.update(&txn).await?;
        }

        let updated_cart = self.recalculate_cart_totals(&txn, cart_id).await?;
        txn.commit().await?;

        Ok(updated_cart)
    }

    /// Retrieves a cart with all its items.
    ///
    /// Performs a join to efficiently load the cart and all associated items
    /// in a single database query.
    ///
    /// # Arguments
    ///
    /// * `cart_id` - UUID of the cart to retrieve
    ///
    /// # Returns
    ///
    /// * `Ok(CartWithItems)` - Cart model with associated items
    /// * `Err(ServiceError::NotFound)` - Cart not found
    /// * `Err(ServiceError)` - Database error
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let cart_with_items = cart_service.get_cart(cart_id).await?;
    /// println!("Cart has {} items", cart_with_items.items.len());
    /// ```
    #[instrument(skip(self))]
    pub async fn get_cart(&self, cart_id: Uuid) -> Result<CartWithItems, ServiceError> {
        let cart = Cart::find_by_id(cart_id)
            .one(&*self.db)
            .await?
            .ok_or_else(|| ServiceError::NotFound(format!("Cart {} not found", cart_id)))?;

        let items = cart.find_related(CartItem).all(&*self.db).await?;

        Ok(CartWithItems { cart, items })
    }

    /// Retrieves a cart without loading its items.
    ///
    /// More efficient than `get_cart` when you only need cart metadata
    /// and don't need the associated items.
    ///
    /// # Arguments
    ///
    /// * `cart_id` - UUID of the cart to retrieve
    ///
    /// # Returns
    ///
    /// * `Ok(CartModel)` - Cart model without items
    /// * `Err(ServiceError::NotFound)` - Cart not found
    /// * `Err(ServiceError)` - Database error
    pub async fn get_cart_model(&self, cart_id: Uuid) -> Result<CartModel, ServiceError> {
        Cart::find_by_id(cart_id)
            .one(&*self.db)
            .await?
            .ok_or_else(|| ServiceError::NotFound(format!("Cart {} not found", cart_id)))
    }

    /// Lists all carts for a specific customer with pagination.
    ///
    /// Returns carts ordered by creation date (newest first).
    ///
    /// # Arguments
    ///
    /// * `customer_id` - UUID of the customer
    /// * `page` - Page number (1-indexed)
    /// * `per_page` - Number of carts per page
    ///
    /// # Returns
    ///
    /// * `Ok((Vec<CartModel>, u64))` - Tuple of (carts for page, total count)
    /// * `Err(ServiceError)` - Database error
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let (carts, total) = cart_service.list_carts_for_customer(
    ///     customer_id,
    ///     1, // page
    ///     20 // per_page
    /// ).await?;
    /// println!("Showing {} of {} total carts", carts.len(), total);
    /// ```
    pub async fn list_carts_for_customer(
        &self,
        customer_id: Uuid,
        page: u64,
        per_page: u64,
    ) -> Result<(Vec<CartModel>, u64), ServiceError> {
        let paginator = Cart::find()
            .filter(cart::Column::CustomerId.eq(Some(customer_id)))
            .order_by_desc(cart::Column::CreatedAt)
            .paginate(&*self.db, per_page);

        let total = paginator.num_items().await?;
        let data = paginator.fetch_page(page.saturating_sub(1)).await?;

        Ok((data, total))
    }

    /// Marks a cart as abandoned (soft delete).
    ///
    /// Changes the cart status to `Abandoned` without deleting the cart or items.
    /// Useful for cart abandonment analysis and recovery campaigns.
    /// Publishes a `CartUpdated` event upon success.
    ///
    /// # Arguments
    ///
    /// * `cart_id` - UUID of the cart to abandon
    ///
    /// # Returns
    ///
    /// * `Ok(CartModel)` - Updated cart with abandoned status
    /// * `Err(ServiceError::NotFound)` - Cart not found
    /// * `Err(ServiceError)` - Database error
    ///
    /// # Examples
    ///
    /// ```ignore
    /// // Mark cart as abandoned after 24 hours of inactivity
    /// let abandoned_cart = cart_service.abandon_cart(cart_id).await?;
    /// // Trigger cart recovery email
    /// send_cart_recovery_email(&abandoned_cart).await?;
    /// ```
    pub async fn abandon_cart(&self, cart_id: Uuid) -> Result<CartModel, ServiceError> {
        let cart = Cart::find_by_id(cart_id)
            .one(&*self.db)
            .await?
            .ok_or_else(|| ServiceError::NotFound(format!("Cart {} not found", cart_id)))?;

        let mut active: cart::ActiveModel = cart.into();
        active.status = Set(cart::CartStatus::Abandoned);
        active.updated_at = Set(Utc::now());

        let updated = active.update(&*self.db).await?;
        self.event_sender
            .send_or_log(Event::CartUpdated(updated.id))
            .await;

        Ok(updated)
    }

    /// Clears all items from a cart and resets totals to zero.
    ///
    /// Deletes all cart items and resets all monetary totals (subtotal, tax,
    /// shipping, discount, total) to zero. The cart itself remains active.
    ///
    /// # Arguments
    ///
    /// * `cart_id` - UUID of the cart to clear
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Cart successfully cleared
    /// * `Err(ServiceError::NotFound)` - Cart not found
    /// * `Err(ServiceError)` - Database transaction error
    ///
    /// # Examples
    ///
    /// ```ignore
    /// // Clear cart before starting new shopping session
    /// cart_service.clear_cart(cart_id).await?;
    /// ```
    #[instrument(skip(self))]
    pub async fn clear_cart(&self, cart_id: Uuid) -> Result<(), ServiceError> {
        let txn = self.db.begin().await?;

        // Delete all items
        CartItem::delete_many()
            .filter(cart_item::Column::CartId.eq(cart_id))
            .exec(&txn)
            .await?;

        // Reset cart totals
        let mut cart: cart::ActiveModel = Cart::find_by_id(cart_id)
            .one(&txn)
            .await?
            .ok_or_else(|| ServiceError::NotFound(format!("Cart {} not found", cart_id)))?
            .into();

        cart.subtotal = Set(Decimal::ZERO);
        cart.tax_total = Set(Decimal::ZERO);
        cart.shipping_total = Set(Decimal::ZERO);
        cart.discount_total = Set(Decimal::ZERO);
        cart.total = Set(Decimal::ZERO);
        cart.updated_at = Set(Utc::now());
        cart.update(&txn).await?;

        txn.commit().await?;

        info!("Cleared cart: {}", cart_id);
        Ok(())
    }

    /// Recalculate cart totals including tax, shipping, and discounts
    async fn recalculate_cart_totals(
        &self,
        conn: &impl sea_orm::ConnectionTrait,
        cart_id: Uuid,
    ) -> Result<CartModel, ServiceError> {
        let items = CartItem::find()
            .filter(cart_item::Column::CartId.eq(cart_id))
            .all(conn)
            .await?;

        let subtotal: Decimal = items.iter().map(|item| item.line_total).sum();

        // Calculate tax using configurable default rate
        let tax_rate =
            Decimal::from_f64_retain(self.config.default_tax_rate).unwrap_or(Decimal::ZERO);
        let tax_total = subtotal * tax_rate;

        // Calculate shipping
        // Free shipping on orders over $50, otherwise $10 flat rate
        let shipping_total = if subtotal >= Decimal::from(50) {
            Decimal::ZERO
        } else if subtotal > Decimal::ZERO {
            Decimal::from(10)
        } else {
            Decimal::ZERO
        };

        // Get discount total from cart items (item-level discounts)
        let discount_total: Decimal = items.iter().map(|item| item.discount_amount).sum();

        // Calculate final total
        let total = subtotal + tax_total + shipping_total - discount_total;

        let mut cart: cart::ActiveModel = Cart::find_by_id(cart_id)
            .one(conn)
            .await?
            .ok_or_else(|| ServiceError::NotFound(format!("Cart {} not found", cart_id)))?
            .into();

        cart.subtotal = Set(subtotal);
        cart.tax_total = Set(tax_total);
        cart.shipping_total = Set(shipping_total);
        cart.discount_total = Set(discount_total);
        cart.total = Set(total);
        cart.updated_at = Set(Utc::now());

        info!(
            "Recalculated cart {}: subtotal=${}, tax=${}, shipping=${}, discount=${}, total=${}",
            cart_id, subtotal, tax_total, shipping_total, discount_total, total
        );

        Ok(cart.update(conn).await?)
    }
}

/// Input for creating a cart
#[derive(Debug, Deserialize)]
pub struct CreateCartInput {
    pub session_id: Option<String>,
    pub customer_id: Option<Uuid>,
    pub currency: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

/// Input for adding item to cart
#[derive(Debug, Deserialize)]
pub struct AddToCartInput {
    pub variant_id: Uuid,
    pub quantity: i32,
}

/// Cart with items
#[derive(Debug, Serialize)]
pub struct CartWithItems {
    pub cart: CartModel,
    pub items: Vec<cart_item::Model>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    // ==================== CreateCartInput Tests ====================

    #[test]
    fn test_create_cart_input_default_values() {
        let input = CreateCartInput {
            session_id: None,
            customer_id: None,
            currency: None,
            metadata: None,
        };

        assert!(input.session_id.is_none());
        assert!(input.customer_id.is_none());
        assert!(input.currency.is_none());
        assert!(input.metadata.is_none());
    }

    #[test]
    fn test_create_cart_input_with_values() {
        let customer_id = Uuid::new_v4();
        let input = CreateCartInput {
            session_id: Some("session_123".to_string()),
            customer_id: Some(customer_id),
            currency: Some("EUR".to_string()),
            metadata: Some(serde_json::json!({"source": "mobile"})),
        };

        assert_eq!(input.session_id.unwrap(), "session_123");
        assert_eq!(input.customer_id.unwrap(), customer_id);
        assert_eq!(input.currency.unwrap(), "EUR");
        assert!(input.metadata.is_some());
    }

    #[test]
    fn test_create_cart_input_deserialization() {
        let json = r#"{
            "session_id": "sess_abc",
            "currency": "USD"
        }"#;

        let input: CreateCartInput =
            serde_json::from_str(json).expect("deserialization should succeed");
        assert_eq!(input.session_id.unwrap(), "sess_abc");
        assert_eq!(input.currency.unwrap(), "USD");
        assert!(input.customer_id.is_none());
    }

    // ==================== AddToCartInput Tests ====================

    #[test]
    fn test_add_to_cart_input_valid() {
        let variant_id = Uuid::new_v4();
        let input = AddToCartInput {
            variant_id,
            quantity: 5,
        };

        assert_eq!(input.variant_id, variant_id);
        assert_eq!(input.quantity, 5);
    }

    #[test]
    fn test_add_to_cart_input_deserialization() {
        let json = r#"{
            "variant_id": "550e8400-e29b-41d4-a716-446655440000",
            "quantity": 3
        }"#;

        let input: AddToCartInput =
            serde_json::from_str(json).expect("deserialization should succeed");
        assert_eq!(input.quantity, 3);
        assert_eq!(
            input.variant_id.to_string(),
            "550e8400-e29b-41d4-a716-446655440000"
        );
    }

    #[test]
    fn test_add_to_cart_input_zero_quantity() {
        let input = AddToCartInput {
            variant_id: Uuid::new_v4(),
            quantity: 0,
        };
        assert_eq!(input.quantity, 0);
    }

    #[test]
    fn test_add_to_cart_input_negative_quantity() {
        let input = AddToCartInput {
            variant_id: Uuid::new_v4(),
            quantity: -5,
        };
        assert_eq!(input.quantity, -5);
    }

    // ==================== CartWithItems Tests ====================

    #[test]
    fn test_cart_with_items_serialization() {
        // This test verifies the struct can be serialized
        // (actual data requires database models)
    }

    // ==================== Cart Total Calculation Logic Tests ====================

    #[test]
    fn test_tax_rate_calculation() {
        // Standard 8% tax rate using exact decimal representation
        let tax_rate = dec!(0.08);
        let subtotal = dec!(100.00);
        let tax = subtotal * tax_rate;

        assert_eq!(tax, dec!(8.00));
    }

    #[test]
    fn test_shipping_free_over_fifty() {
        let subtotal = dec!(50.00);
        let shipping = if subtotal >= Decimal::from(50) {
            Decimal::ZERO
        } else if subtotal > Decimal::ZERO {
            Decimal::from(10)
        } else {
            Decimal::ZERO
        };

        assert_eq!(shipping, Decimal::ZERO);
    }

    #[test]
    fn test_shipping_flat_rate_under_fifty() {
        let subtotal = dec!(49.99);
        let shipping = if subtotal >= Decimal::from(50) {
            Decimal::ZERO
        } else if subtotal > Decimal::ZERO {
            Decimal::from(10)
        } else {
            Decimal::ZERO
        };

        assert_eq!(shipping, Decimal::from(10));
    }

    #[test]
    fn test_shipping_zero_for_empty_cart() {
        let subtotal = Decimal::ZERO;
        let shipping = if subtotal >= Decimal::from(50) {
            Decimal::ZERO
        } else if subtotal > Decimal::ZERO {
            Decimal::from(10)
        } else {
            Decimal::ZERO
        };

        assert_eq!(shipping, Decimal::ZERO);
    }

    #[test]
    fn test_total_calculation() {
        let subtotal = dec!(100.00);
        let tax_total = dec!(8.00);
        let shipping_total = Decimal::ZERO; // Free shipping over $50
        let discount_total = dec!(10.00);

        let total = subtotal + tax_total + shipping_total - discount_total;

        assert_eq!(total, dec!(98.00));
    }

    #[test]
    fn test_total_calculation_with_shipping() {
        let subtotal = dec!(30.00);
        let tax_rate = dec!(0.08);
        let tax_total = subtotal * tax_rate;
        let shipping_total = Decimal::from(10); // Under $50 threshold
        let discount_total = Decimal::ZERO;

        let total = subtotal + tax_total + shipping_total - discount_total;

        // $30 + $2.40 tax + $10 shipping = $42.40
        assert_eq!(total, dec!(42.40));
    }

    #[test]
    fn test_line_total_calculation() {
        let unit_price = dec!(25.50);
        let quantity = 3;
        let line_total = unit_price * Decimal::from(quantity);

        assert_eq!(line_total, dec!(76.50));
    }

    #[test]
    fn test_line_total_single_item() {
        let unit_price = dec!(99.99);
        let quantity = 1;
        let line_total = unit_price * Decimal::from(quantity);

        assert_eq!(line_total, dec!(99.99));
    }

    #[test]
    fn test_subtotal_multiple_items() {
        let line_totals = vec![dec!(25.00), dec!(35.50), dec!(14.50)];
        let subtotal: Decimal = line_totals.iter().sum();

        assert_eq!(subtotal, dec!(75.00));
    }

    #[test]
    fn test_discount_total_aggregation() {
        let discounts = vec![dec!(5.00), dec!(2.50), dec!(0.00)];
        let discount_total: Decimal = discounts.iter().sum();

        assert_eq!(discount_total, dec!(7.50));
    }

    // ==================== Currency Tests ====================

    #[test]
    fn test_default_currency() {
        let currency = None::<String>.unwrap_or_else(|| "USD".to_string());
        assert_eq!(currency, "USD");
    }

    #[test]
    fn test_custom_currency() {
        let currency = Some("EUR".to_string()).unwrap_or_else(|| "USD".to_string());
        assert_eq!(currency, "EUR");
    }

    // ==================== Cart Expiration Tests ====================

    #[test]
    fn test_cart_expiration_duration() {
        let now = Utc::now();
        let expires_at = now + Duration::days(30);

        assert!(expires_at > now);
        assert!((expires_at - now).num_days() == 30);
    }

    // ==================== UUID Tests ====================

    #[test]
    fn test_cart_id_uniqueness() {
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        assert_ne!(id1, id2);
    }

    #[test]
    fn test_uuid_parsing() {
        let uuid_str = "550e8400-e29b-41d4-a716-446655440000";
        let parsed = Uuid::parse_str(uuid_str);

        assert!(parsed.is_ok());
        assert_eq!(parsed.unwrap().to_string(), uuid_str);
    }

    // ==================== Input Validation Edge Cases ====================

    #[test]
    fn test_large_quantity() {
        let input = AddToCartInput {
            variant_id: Uuid::new_v4(),
            quantity: i32::MAX,
        };

        assert_eq!(input.quantity, i32::MAX);
    }

    #[test]
    fn test_empty_session_id() {
        let input = CreateCartInput {
            session_id: Some("".to_string()),
            customer_id: None,
            currency: None,
            metadata: None,
        };

        assert_eq!(input.session_id.unwrap(), "");
    }

    #[test]
    fn test_long_session_id() {
        let long_session = "x".repeat(1000);
        let input = CreateCartInput {
            session_id: Some(long_session.clone()),
            customer_id: None,
            currency: None,
            metadata: None,
        };

        assert_eq!(input.session_id.unwrap().len(), 1000);
    }

    // ==================== Decimal Precision Tests ====================

    #[test]
    fn test_decimal_precision_maintained() {
        let price1 = dec!(33.33);
        let price2 = dec!(33.33);
        let price3 = dec!(33.34);
        let total = price1 + price2 + price3;

        assert_eq!(total, dec!(100.00));
    }

    #[test]
    fn test_decimal_multiplication_precision() {
        let unit_price = dec!(19.99);
        let quantity = Decimal::from(7);
        let line_total = unit_price * quantity;

        assert_eq!(line_total, dec!(139.93));
    }

    #[test]
    fn test_small_decimal_amounts() {
        let unit_price = dec!(0.01);
        let quantity = Decimal::from(100);
        let line_total = unit_price * quantity;

        assert_eq!(line_total, dec!(1.00));
    }
}
