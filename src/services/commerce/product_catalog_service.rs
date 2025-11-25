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
use serde_json::json;
use std::sync::Arc;
use tracing::{info, instrument};
use uuid::Uuid;

const DEFAULT_LIMIT: u64 = 20;
const MAX_LIMIT: u64 = 100;

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
        self.ensure_unique_sku(&input.sku, None).await?;

        let product_id = Uuid::new_v4();
        let now = Utc::now();

        let product = product::ActiveModel {
            id: Set(product_id),
            name: Set(input.name.clone()),
            description: Set(input.description.clone()),
            sku: Set(input.sku.clone()),
            price: Set(input.price),
            currency: Set(input.currency.clone()),
            weight_kg: Set(input.weight_kg),
            dimensions_cm: Set(input.dimensions_cm.clone()),
            barcode: Set(None),
            brand: Set(input.brand.clone()),
            manufacturer: Set(input.manufacturer.clone()),
            is_active: Set(input.is_active),
            is_digital: Set(input.is_digital),
            image_url: Set(input.image_url.clone()),
            category_id: Set(None),
            reorder_point: Set(input.reorder_point),
            tax_rate: Set(input.tax_rate),
            cost_price: Set(input.cost_price),
            msrp: Set(input.msrp),
            tags: Set(input.tags.clone()),
            meta_title: Set(input.meta_title.clone()),
            meta_description: Set(input.meta_description.clone()),
            created_at: Set(now),
            updated_at: Set(Some(now)),
        };

        let product = product.insert(&*self.db).await?;

        // Publish event
        self.event_sender
            .send_or_log(Event::ProductCreated(product_id))
            .await;

        info!("Created product: {}", product_id);
        Ok(product)
    }

    /// Update an existing product
    #[instrument(skip(self))]
    pub async fn update_product(
        &self,
        product_id: Uuid,
        input: UpdateProductInput,
    ) -> Result<ProductModel, ServiceError> {
        if let Some(ref sku) = input.sku {
            self.ensure_unique_sku(sku, Some(product_id)).await?;
        }

        let product = self.get_product(product_id).await?;
        let mut active: product::ActiveModel = product.into();

        if let Some(name) = input.name {
            active.name = Set(name);
        }
        if let Some(description) = input.description {
            active.description = Set(Some(description));
        }
        if let Some(sku) = input.sku {
            active.sku = Set(sku);
        }
        if let Some(price) = input.price {
            active.price = Set(price);
        }
        if let Some(currency) = input.currency {
            active.currency = Set(currency);
        }
        if let Some(is_active) = input.is_active {
            active.is_active = Set(is_active);
        }
        if let Some(is_digital) = input.is_digital {
            active.is_digital = Set(is_digital);
        }
        if let Some(image_url) = input.image_url {
            active.image_url = Set(Some(image_url));
        }
        if let Some(brand) = input.brand {
            active.brand = Set(Some(brand));
        }
        if let Some(manufacturer) = input.manufacturer {
            active.manufacturer = Set(Some(manufacturer));
        }
        if let Some(weight) = input.weight_kg {
            active.weight_kg = Set(Some(weight));
        }
        if let Some(dimensions) = input.dimensions_cm {
            active.dimensions_cm = Set(Some(dimensions));
        }
        if let Some(tags) = input.tags {
            active.tags = Set(Some(tags));
        }
        if let Some(cost_price) = input.cost_price {
            active.cost_price = Set(Some(cost_price));
        }
        if let Some(msrp) = input.msrp {
            active.msrp = Set(Some(msrp));
        }
        if let Some(tax_rate) = input.tax_rate {
            active.tax_rate = Set(Some(tax_rate));
        }
        if let Some(meta_title) = input.meta_title {
            active.meta_title = Set(Some(meta_title));
        }
        if let Some(meta_description) = input.meta_description {
            active.meta_description = Set(Some(meta_description));
        }
        if let Some(reorder_point) = input.reorder_point {
            active.reorder_point = Set(Some(reorder_point));
        }

        active.updated_at = Set(Some(Utc::now()));

        let product = active.update(&*self.db).await?;
        info!("Updated product: {}", product_id);
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

    /// Delete a product variant
    pub async fn delete_variant(&self, variant_id: Uuid) -> Result<(), ServiceError> {
        let variant = ProductVariant::find_by_id(variant_id)
            .one(&*self.db)
            .await?
            .ok_or_else(|| {
                ServiceError::NotFound(format!("Product variant {} not found", variant_id))
            })?;

        let active: product_variant::ActiveModel = variant.clone().into();
        active.delete(&*self.db).await?;

        self.event_sender
            .send_or_log(Event::Generic {
                message: format!("product_variant_deleted:{}", variant_id),
                timestamp: Utc::now(),
                metadata: json!({
                    "variant_id": variant_id,
                    "product_id": variant.product_id
                }),
            })
            .await;

        info!("Deleted variant {}", variant_id);
        Ok(())
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

        let limit = query.limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT);
        let offset = query.offset.unwrap_or(0);

        let products = db_query
            .order_by_desc(product::Column::CreatedAt)
            .limit(limit)
            .offset(offset)
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

    async fn ensure_unique_sku(
        &self,
        sku: &str,
        exclude_id: Option<Uuid>,
    ) -> Result<(), ServiceError> {
        let mut query = Product::find().filter(product::Column::Sku.eq(sku));
        if let Some(id) = exclude_id {
            query = query.filter(product::Column::Id.ne(id));
        }

        if query.one(&*self.db).await?.is_some() {
            return Err(ServiceError::ValidationError(format!(
                "SKU {} already exists",
                sku
            )));
        }

        Ok(())
    }
}

/// Input for creating a product
#[derive(Debug, Deserialize, Serialize)]
pub struct CreateProductInput {
    pub name: String,
    pub sku: String,
    pub description: Option<String>,
    pub price: Decimal,
    pub currency: String,
    pub is_active: bool,
    pub is_digital: bool,
    pub image_url: Option<String>,
    pub brand: Option<String>,
    pub manufacturer: Option<String>,
    pub weight_kg: Option<Decimal>,
    pub dimensions_cm: Option<String>,
    pub tags: Option<String>,
    pub cost_price: Option<Decimal>,
    pub msrp: Option<Decimal>,
    pub tax_rate: Option<Decimal>,
    pub meta_title: Option<String>,
    pub meta_description: Option<String>,
    pub reorder_point: Option<i32>,
}

/// Input for updating a product
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct UpdateProductInput {
    pub name: Option<String>,
    pub sku: Option<String>,
    pub description: Option<String>,
    pub price: Option<Decimal>,
    pub currency: Option<String>,
    pub is_active: Option<bool>,
    pub is_digital: Option<bool>,
    pub image_url: Option<String>,
    pub brand: Option<String>,
    pub manufacturer: Option<String>,
    pub weight_kg: Option<Decimal>,
    pub dimensions_cm: Option<String>,
    pub tags: Option<String>,
    pub cost_price: Option<Decimal>,
    pub msrp: Option<Decimal>,
    pub tax_rate: Option<Decimal>,
    pub meta_title: Option<String>,
    pub meta_description: Option<String>,
    pub reorder_point: Option<i32>,
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
#[derive(Debug, Clone, Deserialize)]
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
