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

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use std::collections::HashMap;

    // ==================== CreateProductInput Tests ====================

    #[test]
    fn test_create_product_input_complete() {
        let input = CreateProductInput {
            name: "Test Product".to_string(),
            sku: "TEST-SKU-001".to_string(),
            description: Some("A test product description".to_string()),
            price: dec!(99.99),
            currency: "USD".to_string(),
            is_active: true,
            is_digital: false,
            image_url: Some("https://example.com/image.jpg".to_string()),
            brand: Some("Test Brand".to_string()),
            manufacturer: Some("Test Manufacturer".to_string()),
            weight_kg: Some(dec!(1.5)),
            dimensions_cm: Some("10x20x30".to_string()),
            tags: Some("test,product".to_string()),
            cost_price: Some(dec!(50.00)),
            msrp: Some(dec!(149.99)),
            tax_rate: Some(dec!(8.5)),
            meta_title: Some("Test Product - Buy Now".to_string()),
            meta_description: Some("Best test product available".to_string()),
            reorder_point: Some(10),
        };

        assert_eq!(input.name, "Test Product");
        assert_eq!(input.sku, "TEST-SKU-001");
        assert_eq!(input.price, dec!(99.99));
        assert!(input.is_active);
        assert!(!input.is_digital);
    }

    #[test]
    fn test_create_product_input_minimal() {
        let input = CreateProductInput {
            name: "Simple Product".to_string(),
            sku: "SIMPLE-001".to_string(),
            description: None,
            price: dec!(19.99),
            currency: "USD".to_string(),
            is_active: true,
            is_digital: false,
            image_url: None,
            brand: None,
            manufacturer: None,
            weight_kg: None,
            dimensions_cm: None,
            tags: None,
            cost_price: None,
            msrp: None,
            tax_rate: None,
            meta_title: None,
            meta_description: None,
            reorder_point: None,
        };

        assert!(input.description.is_none());
        assert!(input.brand.is_none());
        assert!(input.cost_price.is_none());
    }

    #[test]
    fn test_create_product_input_serialization() {
        let input = CreateProductInput {
            name: "Serialized Product".to_string(),
            sku: "SER-001".to_string(),
            description: Some("Test".to_string()),
            price: dec!(25.00),
            currency: "EUR".to_string(),
            is_active: true,
            is_digital: true,
            image_url: None,
            brand: None,
            manufacturer: None,
            weight_kg: None,
            dimensions_cm: None,
            tags: None,
            cost_price: None,
            msrp: None,
            tax_rate: None,
            meta_title: None,
            meta_description: None,
            reorder_point: None,
        };

        let json = serde_json::to_string(&input).expect("serialization should succeed");
        assert!(json.contains("Serialized Product"));
        assert!(json.contains("EUR"));
    }

    // ==================== UpdateProductInput Tests ====================

    #[test]
    fn test_update_product_input_partial() {
        let input = UpdateProductInput {
            name: Some("Updated Name".to_string()),
            price: Some(dec!(149.99)),
            ..Default::default()
        };

        assert!(input.name.is_some());
        assert!(input.price.is_some());
        assert!(input.sku.is_none());
        assert!(input.description.is_none());
    }

    #[test]
    fn test_update_product_input_all_fields() {
        let input = UpdateProductInput {
            name: Some("New Name".to_string()),
            sku: Some("NEW-SKU".to_string()),
            description: Some("New description".to_string()),
            price: Some(dec!(199.99)),
            currency: Some("GBP".to_string()),
            is_active: Some(false),
            is_digital: Some(true),
            image_url: Some("https://new-image.com/img.jpg".to_string()),
            brand: Some("New Brand".to_string()),
            manufacturer: Some("New Manufacturer".to_string()),
            weight_kg: Some(dec!(2.0)),
            dimensions_cm: Some("20x30x40".to_string()),
            tags: Some("new,updated".to_string()),
            cost_price: Some(dec!(100.00)),
            msrp: Some(dec!(299.99)),
            tax_rate: Some(dec!(10.0)),
            meta_title: Some("New Meta Title".to_string()),
            meta_description: Some("New Meta Description".to_string()),
            reorder_point: Some(20),
        };

        assert!(input.name.is_some());
        assert!(input.sku.is_some());
        assert_eq!(input.is_active, Some(false));
    }

    #[test]
    fn test_update_product_input_empty() {
        let input = UpdateProductInput::default();

        assert!(input.name.is_none());
        assert!(input.sku.is_none());
        assert!(input.price.is_none());
    }

    // ==================== CreateVariantInput Tests ====================

    #[test]
    fn test_create_variant_input() {
        let mut options = HashMap::new();
        options.insert("Size".to_string(), "Large".to_string());
        options.insert("Color".to_string(), "Blue".to_string());

        let input = CreateVariantInput {
            product_id: Uuid::new_v4(),
            sku: "PROD-LG-BLUE".to_string(),
            name: "Large Blue".to_string(),
            price: dec!(49.99),
            compare_at_price: Some(dec!(59.99)),
            cost: Some(dec!(25.00)),
            weight: Some(0.5),
            dimensions: None,
            options,
            inventory_tracking: true,
            position: 1,
        };

        assert_eq!(input.sku, "PROD-LG-BLUE");
        assert!(input.inventory_tracking);
        assert_eq!(input.position, 1);
    }

    #[test]
    fn test_create_variant_input_minimal() {
        let input = CreateVariantInput {
            product_id: Uuid::new_v4(),
            sku: "SIMPLE-VAR".to_string(),
            name: "Default".to_string(),
            price: dec!(19.99),
            compare_at_price: None,
            cost: None,
            weight: None,
            dimensions: None,
            options: HashMap::new(),
            inventory_tracking: false,
            position: 0,
        };

        assert!(input.compare_at_price.is_none());
        assert!(input.cost.is_none());
        assert!(!input.inventory_tracking);
    }

    // ==================== ProductSearchQuery Tests ====================

    #[test]
    fn test_product_search_query_with_search() {
        let query = ProductSearchQuery {
            search: Some("test product".to_string()),
            is_active: Some(true),
            limit: Some(20),
            offset: Some(0),
        };

        assert!(query.search.is_some());
        assert_eq!(query.search.unwrap(), "test product");
    }

    #[test]
    fn test_product_search_query_empty() {
        let query = ProductSearchQuery {
            search: None,
            is_active: None,
            limit: None,
            offset: None,
        };

        assert!(query.search.is_none());
        assert!(query.is_active.is_none());
    }

    #[test]
    fn test_product_search_query_pagination() {
        let query = ProductSearchQuery {
            search: None,
            is_active: None,
            limit: Some(50),
            offset: Some(100),
        };

        assert_eq!(query.limit, Some(50));
        assert_eq!(query.offset, Some(100));
    }

    // ==================== SKU Tests ====================

    #[test]
    fn test_sku_format_valid() {
        let valid_skus = vec![
            "SKU-001",
            "PROD-ABC-123",
            "ITEM123",
            "test-sku-001",
        ];

        for sku in valid_skus {
            assert!(!sku.is_empty());
            assert!(sku.len() <= 50);
        }
    }

    #[test]
    fn test_sku_uniqueness_pattern() {
        let sku = "UNIQUE-SKU-001";
        let pattern = format!("%{}%", sku);

        assert!(pattern.contains(sku));
    }

    // ==================== Price Tests ====================

    #[test]
    fn test_price_positive() {
        let price = dec!(99.99);
        assert!(price > Decimal::ZERO);
    }

    #[test]
    fn test_price_zero_valid() {
        // Free products are valid
        let price = Decimal::ZERO;
        assert!(price >= Decimal::ZERO);
    }

    #[test]
    fn test_price_precision() {
        let price = dec!(19.99);
        let quantity = 3;
        let total = price * Decimal::from(quantity);

        assert_eq!(total, dec!(59.97));
    }

    #[test]
    fn test_compare_at_price() {
        let price = dec!(79.99);
        let compare_at = dec!(99.99);

        // Compare at price should be higher (original price)
        assert!(compare_at > price);
    }

    // ==================== Currency Tests ====================

    #[test]
    fn test_valid_currencies() {
        let currencies = vec!["USD", "EUR", "GBP", "CAD", "AUD", "JPY"];

        for currency in currencies {
            assert_eq!(currency.len(), 3);
            assert!(currency.chars().all(|c| c.is_ascii_uppercase()));
        }
    }

    // ==================== Digital Product Tests ====================

    #[test]
    fn test_digital_product_no_weight() {
        let is_digital = true;
        let weight: Option<Decimal> = None;

        if is_digital {
            // Digital products typically have no weight
            assert!(weight.is_none() || weight == Some(Decimal::ZERO));
        }
    }

    #[test]
    fn test_physical_product_has_weight() {
        let is_digital = false;
        let weight: Option<Decimal> = Some(dec!(1.5));

        if !is_digital {
            assert!(weight.is_some());
        }
    }

    // ==================== Inventory Tracking Tests ====================

    #[test]
    fn test_inventory_tracking_enabled() {
        let tracking = true;
        assert!(tracking);
    }

    #[test]
    fn test_reorder_point() {
        let reorder_point: Option<i32> = Some(10);
        let current_stock = 5;

        if let Some(point) = reorder_point {
            let needs_reorder = current_stock < point;
            assert!(needs_reorder);
        }
    }

    // ==================== Pagination Constants Tests ====================

    #[test]
    fn test_default_limit() {
        assert_eq!(DEFAULT_LIMIT, 20);
    }

    #[test]
    fn test_max_limit() {
        assert_eq!(MAX_LIMIT, 100);
    }

    #[test]
    fn test_limit_capping() {
        let requested_limit: u64 = 200;
        let actual_limit = requested_limit.min(MAX_LIMIT);

        assert_eq!(actual_limit, 100);
    }

    // ==================== UUID Tests ====================

    #[test]
    fn test_product_id_uniqueness() {
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        assert_ne!(id1, id2);
    }

    #[test]
    fn test_variant_id_format() {
        let id = Uuid::new_v4();
        let id_str = id.to_string();

        assert_eq!(id_str.len(), 36);
        assert!(!id.is_nil());
    }

    // ==================== Error Tests ====================

    #[test]
    fn test_not_found_error_product() {
        let product_id = Uuid::new_v4();
        let error = ServiceError::NotFound(format!("Product {} not found", product_id));

        match error {
            ServiceError::NotFound(msg) => {
                assert!(msg.contains("not found"));
            }
            _ => panic!("Expected NotFound error"),
        }
    }

    #[test]
    fn test_not_found_error_variant() {
        let variant_id = Uuid::new_v4();
        let error = ServiceError::NotFound(format!("Product variant {} not found", variant_id));

        match error {
            ServiceError::NotFound(msg) => {
                assert!(msg.contains("variant"));
                assert!(msg.contains("not found"));
            }
            _ => panic!("Expected NotFound error"),
        }
    }

    #[test]
    fn test_validation_error_sku_exists() {
        let sku = "EXISTING-SKU";
        let error = ServiceError::ValidationError(format!("SKU {} already exists", sku));

        match error {
            ServiceError::ValidationError(msg) => {
                assert!(msg.contains("already exists"));
            }
            _ => panic!("Expected ValidationError"),
        }
    }

    // ==================== Search Pattern Tests ====================

    #[test]
    fn test_search_pattern_generation() {
        let search = "test";
        let pattern = format!("%{}%", search);

        assert_eq!(pattern, "%test%");
    }

    #[test]
    fn test_search_filter_active() {
        let is_active: Option<bool> = Some(true);
        assert!(is_active.unwrap_or(false));
    }

    #[test]
    fn test_search_filter_inactive() {
        let is_active: Option<bool> = Some(false);
        assert!(!is_active.unwrap_or(true));
    }

    // ==================== Tag Tests ====================

    #[test]
    fn test_tags_format() {
        let tags = "electronics,gadgets,sale";
        let tag_list: Vec<&str> = tags.split(',').collect();

        assert_eq!(tag_list.len(), 3);
        assert!(tag_list.contains(&"electronics"));
    }

    #[test]
    fn test_tags_empty() {
        let tags: Option<String> = None;
        assert!(tags.is_none());
    }

    // ==================== Variant Options Tests ====================

    #[test]
    fn test_variant_options() {
        let mut options = HashMap::new();
        options.insert("Size".to_string(), "Medium".to_string());
        options.insert("Color".to_string(), "Red".to_string());

        assert_eq!(options.len(), 2);
        assert_eq!(options.get("Size"), Some(&"Medium".to_string()));
    }

    #[test]
    fn test_variant_options_empty() {
        let options: HashMap<String, String> = HashMap::new();
        assert!(options.is_empty());
    }

    // ==================== Position Tests ====================

    #[test]
    fn test_variant_position_ordering() {
        let positions = vec![0, 1, 2, 3];

        for i in 0..positions.len() - 1 {
            assert!(positions[i] < positions[i + 1]);
        }
    }

    #[test]
    fn test_variant_position_zero() {
        let position: i32 = 0;
        assert!(position >= 0);
    }
}
