use crate::{
    entities::commerce::{
        cart, cart_item, product_variant, Cart, CartItem, CartModel, ProductVariant,
    },
    errors::ServiceError,
    events::{Event, EventSender},
};
use chrono::{Duration, Utc};
use rust_decimal::Decimal;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, ModelTrait, QueryFilter,
    Set, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{info, instrument};
use uuid::Uuid;

/// Shopping cart service
#[derive(Clone)]
pub struct CartService {
    db: Arc<DatabaseConnection>,
    event_sender: Arc<EventSender>,
}

impl CartService {
    pub fn new(db: Arc<DatabaseConnection>, event_sender: Arc<EventSender>) -> Self {
        Self { db, event_sender }
    }

    /// Create a new cart
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
            metadata: Set(input.metadata.map(|m| serde_json::to_value(m).unwrap())),
            status: Set(cart::CartStatus::Active),
            expires_at: Set(expires_at),
            created_at: Set(Utc::now()),
            updated_at: Set(Utc::now()),
        };

        let cart = cart.insert(&*self.db).await?;

        self.event_sender
            .send(Event::CartCreated(cart_id))
            .await;

        info!("Created cart: {}", cart_id);
        Ok(cart)
    }

    /// Add item to cart
    #[instrument(skip(self))]
    pub async fn add_item(&self, cart_id: Uuid, input: AddToCartInput) -> Result<CartModel, ServiceError> {
        let txn = self.db.begin().await?;

        // Verify cart exists and is active
        let cart = Cart::find_by_id(cart_id)
            .one(&txn)
            .await?
            .ok_or_else(|| ServiceError::NotFound(format!("Cart {} not found", cart_id)))?;

        if cart.status != cart::CartStatus::Active {
            return Err(ServiceError::InvalidOperation("Cart is not active".to_string()));
        }

        // Get variant details
        let variant = ProductVariant::find_by_id(input.variant_id)
            .one(&txn)
            .await?
            .ok_or_else(|| ServiceError::NotFound(format!("Variant {} not found", input.variant_id)))?;

        // Check if item already exists in cart
        let existing_item = CartItem::find()
            .filter(cart_item::Column::CartId.eq(cart_id))
            .filter(cart_item::Column::VariantId.eq(input.variant_id))
            .one(&txn)
            .await?;

        if let Some(item) = existing_item {
            // Update quantity
            let mut item: cart_item::ActiveModel = item.into();
            item.quantity = Set(item.quantity.clone().unwrap() + input.quantity);
            item.line_total = Set(variant.price * Decimal::from(item.quantity.clone().unwrap()));
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
            .send(Event::CartItemAdded {
                cart_id,
                variant_id: input.variant_id,
            })
            .await;

        info!("Added item to cart {}: variant {} x{}", cart_id, input.variant_id, input.quantity);
        Ok(updated_cart)
    }

    /// Update cart item quantity
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
                .ok_or_else(|| ServiceError::NotFound(format!("Cart item {} not found", item_id)))?;

            if item.cart_id != cart_id {
                return Err(ServiceError::InvalidOperation("Item does not belong to this cart".to_string()));
            }

            let mut item: cart_item::ActiveModel = item.into();
            item.quantity = Set(quantity);
            item.line_total = Set(item.unit_price.clone().unwrap() * Decimal::from(quantity));
            item.updated_at = Set(Utc::now());
            item.update(&txn).await?;
        }

        let updated_cart = self.recalculate_cart_totals(&txn, cart_id).await?;
        txn.commit().await?;

        Ok(updated_cart)
    }

    /// Get cart with items
    #[instrument(skip(self))]
    pub async fn get_cart(&self, cart_id: Uuid) -> Result<CartWithItems, ServiceError> {
        let cart = Cart::find_by_id(cart_id)
            .one(&*self.db)
            .await?
            .ok_or_else(|| ServiceError::NotFound(format!("Cart {} not found", cart_id)))?;

        let items = cart.find_related(CartItem).all(&*self.db).await?;

        Ok(CartWithItems { cart, items })
    }

    /// Clear cart
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

    /// Recalculate cart totals
    async fn recalculate_cart_totals(
        &self,
        conn: &impl sea_orm::ConnectionTrait,
        cart_id: Uuid,
    ) -> Result<CartModel, ServiceError> {
        let items = CartItem::find()
            .filter(cart_item::Column::CartId.eq(cart_id))
            .all(conn)
            .await?;

        let subtotal = items.iter().map(|item| item.line_total).sum();

        let mut cart: cart::ActiveModel = Cart::find_by_id(cart_id)
            .one(conn)
            .await?
            .ok_or_else(|| ServiceError::NotFound(format!("Cart {} not found", cart_id)))?
            .into();

        cart.subtotal = Set(subtotal);
        cart.total = Set(subtotal); // TODO: Add tax, shipping, discounts
        cart.updated_at = Set(Utc::now());

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