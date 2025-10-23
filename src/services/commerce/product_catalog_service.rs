use crate::{
    entities::commerce::{product_variant, Product, ProductModel, ProductVariant},
    entities::product,
    errors::ServiceError,
    events::{Event, EventSender},
};
use chrono::Utc;
use rust_decimal::Decimal;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect, Set,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{info, instrument};
use uuid::Uuid;

/// Product catalog service for managing products and variants
#[derive(Clone)]
pub struct ProductCatalogService {
    db: Arc<DatabaseConnection>,
    event_sender: Arc<EventSender>,
}

impl ProductCatalogService {
    pub fn new(db: Arc<DatabaseConnection>, event_sender: Arc<EventSender>) -> Self {
        Self { db, event_sender }
    }

    /// Create a new product
    #[instrument(skip(self))]
    pub async fn create_product(
        &self,
        input: CreateProductInput,
    ) -> Result<ProductModel, ServiceError> {
        let product_id = Uuid::new_v4();

        let product = product::ActiveModel {
            id: Set(product_id),
            name: Set(input.name.clone()),
            // description is Option<String> in entity
            description: Set(Some(input.description)),
            sku: Set(input.slug),
            price: Set(Decimal::ZERO),
            currency: Set("USD".to_string()),
            weight_kg: Set(None),
            dimensions_cm: Set(None),
            barcode: Set(None),
            brand: Set(None),
            manufacturer: Set(None),
            is_active: Set(true),
            is_digital: Set(false),
            image_url: Set(None),
            category_id: Set(None),
            reorder_point: Set(None),
            tax_rate: Set(None),
            cost_price: Set(None),
            msrp: Set(None),
            tags: Set(None),
            meta_title: Set(None),
            meta_description: Set(None),
            created_at: Set(Utc::now()),
            updated_at: Set(Some(Utc::now())),
        };

        let product = product.insert(&*self.db).await?;

        // Publish event
        self.event_sender
            .send_or_log(Event::ProductCreated(product_id))
            .await;

        info!("Created product: {}", product_id);
        Ok(product)
    }

    /// Create a product variant
    #[instrument(skip(self))]
    pub async fn create_variant(
        &self,
        input: CreateVariantInput,
    ) -> Result<product_variant::Model, ServiceError> {
        let variant_id = Uuid::new_v4();

        let variant = product_variant::ActiveModel {
            id: Set(variant_id),
            product_id: Set(input.product_id),
            sku: Set(input.sku),
            name: Set(input.name),
            price: Set(input.price),
            compare_at_price: Set(input.compare_at_price),
            cost: Set(input.cost),
            weight: Set(input.weight),
            dimensions: Set(input.dimensions.map(|d| serde_json::to_value(&d).unwrap())),
            options: Set(serde_json::to_value(&input.options).unwrap()),
            inventory_tracking: Set(input.inventory_tracking),
            position: Set(input.position),
            created_at: Set(Utc::now()),
            updated_at: Set(Utc::now()),
        };

        let variant = variant.insert(&*self.db).await?;

        info!(
            "Created variant {} for product {}",
            variant_id, input.product_id
        );
        Ok(variant)
    }

    /// Get a product by ID
    #[instrument(skip(self))]
    pub async fn get_product(&self, product_id: Uuid) -> Result<ProductModel, ServiceError> {
        Product::find_by_id(product_id)
            .one(&*self.db)
            .await?
            .ok_or_else(|| ServiceError::NotFound(format!("Product {} not found", product_id)))
    }

    /// Get product variants
    #[instrument(skip(self))]
    pub async fn get_product_variants(
        &self,
        product_id: Uuid,
    ) -> Result<Vec<product_variant::Model>, ServiceError> {
        ProductVariant::find()
            .filter(product_variant::Column::ProductId.eq(product_id))
            .order_by_asc(product_variant::Column::Position)
            .all(&*self.db)
            .await
            .map_err(Into::into)
    }

    /// Get a product variant by its identifier
    #[instrument(skip(self))]
    pub async fn get_variant(
        &self,
        variant_id: Uuid,
    ) -> Result<product_variant::Model, ServiceError> {
        ProductVariant::find_by_id(variant_id)
            .one(&*self.db)
            .await?
            .ok_or_else(|| {
                ServiceError::NotFound(format!("Product variant {} not found", variant_id))
            })
    }

    /// Get a product variant by SKU
    #[instrument(skip(self))]
    pub async fn get_variant_by_sku(
        &self,
        sku: &str,
    ) -> Result<product_variant::Model, ServiceError> {
        ProductVariant::find()
            .filter(product_variant::Column::Sku.eq(sku))
            .one(&*self.db)
            .await?
            .ok_or_else(|| {
                ServiceError::NotFound(format!("Product variant with SKU {} not found", sku))
            })
    }

    /// Search products
    #[instrument(skip(self))]
    pub async fn search_products(
        &self,
        query: ProductSearchQuery,
    ) -> Result<ProductSearchResult, ServiceError> {
        let mut db_query = Product::find();

        if let Some(search) = &query.search {
            let pattern = format!("%{}%", search);
            db_query = db_query.filter(
                product::Column::Name
                    .contains(&pattern)
                    .or(product::Column::Sku.contains(&pattern)),
            );
        }

        if let Some(is_active) = query.is_active {
            db_query = db_query.filter(product::Column::IsActive.eq(is_active));
        }

        let total = db_query.clone().count(&*self.db).await?;

        let products = db_query
            .order_by_desc(product::Column::CreatedAt)
            .limit(query.limit.unwrap_or(20))
            .offset(query.offset.unwrap_or(0))
            .all(&*self.db)
            .await?;

        Ok(ProductSearchResult { products, total })
    }

    /// Update product price
    #[instrument(skip(self))]
    pub async fn update_variant_price(
        &self,
        variant_id: Uuid,
        price: Decimal,
    ) -> Result<(), ServiceError> {
        let variant = ProductVariant::find_by_id(variant_id)
            .one(&*self.db)
            .await?
            .ok_or_else(|| ServiceError::NotFound(format!("Variant {} not found", variant_id)))?;

        let mut variant: product_variant::ActiveModel = variant.into();
        variant.price = Set(price);
        variant.updated_at = Set(Utc::now());
        variant.update(&*self.db).await?;

        info!("Updated variant {} price to {}", variant_id, price);
        Ok(())
    }
}

/// Input for creating a product
#[derive(Debug, Deserialize, Serialize)]
pub struct CreateProductInput {
    pub name: String,
    pub slug: String,
    pub description: String,
    // Simplified fields to match entity
    // pub status: product::ProductStatus,
    // pub product_type: product::ProductType,
    pub attributes: Vec<serde_json::Value>,
    pub seo: serde_json::Value,
}

/// Input for creating a variant
#[derive(Debug, Deserialize, Serialize)]
pub struct CreateVariantInput {
    pub product_id: Uuid,
    pub sku: String,
    pub name: String,
    pub price: Decimal,
    pub compare_at_price: Option<Decimal>,
    pub cost: Option<Decimal>,
    pub weight: Option<f64>,
    pub dimensions: Option<product_variant::Dimensions>,
    pub options: std::collections::HashMap<String, String>,
    pub inventory_tracking: bool,
    pub position: i32,
}

/// Product search query
#[derive(Debug, Deserialize)]
pub struct ProductSearchQuery {
    pub search: Option<String>,
    pub is_active: Option<bool>,
    pub limit: Option<u64>,
    pub offset: Option<u64>,
}

/// Product search result
#[derive(Debug, Serialize)]
pub struct ProductSearchResult {
    pub products: Vec<ProductModel>,
    pub total: u64,
}
