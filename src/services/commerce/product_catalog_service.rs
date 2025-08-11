use crate::{
    entities::commerce::{product, product_variant, Product, ProductModel, ProductVariant},
    errors::ServiceError,
    events::{Event, EventSender},
};
use chrono::Utc;
use rust_decimal::Decimal;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, Set, QuerySelect,
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
    pub async fn create_product(&self, input: CreateProductInput) -> Result<ProductModel, ServiceError> {
        let product_id = Uuid::new_v4();
        
        let product = product::ActiveModel {
            id: Set(product_id),
            name: Set(input.name.clone()),
            slug: Set(input.slug),
            description: Set(input.description),
            status: Set(input.status),
            product_type: Set(input.product_type),
            attributes: Set(serde_json::to_value(&input.attributes).unwrap()),
            seo: Set(serde_json::to_value(&input.seo).unwrap()),
            created_at: Set(Utc::now()),
            updated_at: Set(Utc::now()),
        };

        let product = product.insert(&*self.db).await?;

        // Publish event
        self.event_sender
            .send(Event::ProductCreated(product_id))
            .await;

        info!("Created product: {}", product_id);
        Ok(product)
    }

    /// Create a product variant
    #[instrument(skip(self))]
    pub async fn create_variant(&self, input: CreateVariantInput) -> Result<product_variant::Model, ServiceError> {
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

        info!("Created variant {} for product {}", variant_id, input.product_id);
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
    pub async fn get_product_variants(&self, product_id: Uuid) -> Result<Vec<product_variant::Model>, ServiceError> {
        ProductVariant::find()
            .filter(product_variant::Column::ProductId.eq(product_id))
            .order_by_asc(product_variant::Column::Position)
            .all(&*self.db)
            .await
            .map_err(Into::into)
    }

    /// Search products
    #[instrument(skip(self))]
    pub async fn search_products(&self, query: ProductSearchQuery) -> Result<ProductSearchResult, ServiceError> {
        let mut db_query = Product::find();

        if let Some(search) = &query.search {
            db_query = db_query.filter(
                product::Column::Name.contains(search)
                    .or(product::Column::Description.contains(search))
            );
        }

        if let Some(status) = query.status {
            db_query = db_query.filter(product::Column::Status.eq(status));
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
    pub async fn update_variant_price(&self, variant_id: Uuid, price: Decimal) -> Result<(), ServiceError> {
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
    pub status: product::ProductStatus,
    pub product_type: product::ProductType,
    pub attributes: Vec<product::ProductAttribute>,
    pub seo: product::SeoMetadata,
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
    pub status: Option<product::ProductStatus>,
    pub limit: Option<u64>,
    pub offset: Option<u64>,
}

/// Product search result
#[derive(Debug, Serialize)]
pub struct ProductSearchResult {
    pub products: Vec<ProductModel>,
    pub total: u64,
} 