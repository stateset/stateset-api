use std::sync::Arc;
use crate::{
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    entities::product::{self, Entity as Product, Column as ProductColumn},
};
use sea_orm::{EntityTrait, QueryFilter, ColumnTrait, ModelTrait, PaginatorTrait, QueryOrder, Set, ActiveModelTrait};
use tracing::{info, error, instrument};
use uuid::Uuid;
use chrono::Utc;
use rust_decimal::Decimal;

/// Service for managing products
pub struct ProductService {
    db_pool: Arc<DbPool>,
    event_sender: Arc<EventSender>,
}

impl ProductService {
    /// Creates a new product service instance
    pub fn new(
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Self {
        Self {
            db_pool,
            event_sender,
        }
    }

    /// Create a new product
    #[instrument(skip(self))]
    pub async fn create_product(
        &self,
        name: String,
        description: Option<String>,
        sku: String,
        price: Decimal,
        currency: String,
        weight_kg: Option<Decimal>,
        dimensions_cm: Option<String>,
        barcode: Option<String>,
        brand: Option<String>,
        manufacturer: Option<String>,
        is_digital: bool,
        image_url: Option<String>,
        category_id: Option<Uuid>,
        reorder_point: Option<i32>,
        tax_rate: Option<Decimal>,
        cost_price: Option<Decimal>,
        msrp: Option<Decimal>,
        tags: Option<String>,
        meta_title: Option<String>,
        meta_description: Option<String>,
    ) -> Result<Uuid, ServiceError> {
        let db = &*self.db_pool;
        
        // Check if a product with the same SKU already exists
        let existing_product = Product::find()
            .filter(ProductColumn::Sku.eq(&sku))
            .one(db)
            .await
            .map_err(|e| {
                let msg = format!("Failed to check for existing product: {}", e);
                error!(%msg);
                ServiceError::db_error(msg)
            })?;
            
        if existing_product.is_some() {
            let msg = format!("Product with SKU '{}' already exists", sku);
            error!(%msg);
            return Err(ServiceError::ValidationError(msg));
        }
        
        // Create a new product
        let product_id = Uuid::new_v4();
        let product = product::ActiveModel {
            id: Set(product_id),
            name: Set(name.clone()),
            description: Set(description.clone()),
            sku: Set(sku.clone()),
            price: Set(price),
            currency: Set(currency),
            weight_kg: Set(weight_kg),
            dimensions_cm: Set(dimensions_cm),
            barcode: Set(barcode),
            brand: Set(brand),
            manufacturer: Set(manufacturer),
            is_active: Set(true),
            is_digital: Set(is_digital),
            image_url: Set(image_url),
            category_id: Set(category_id),
            reorder_point: Set(reorder_point),
            tax_rate: Set(tax_rate),
            cost_price: Set(cost_price),
            msrp: Set(msrp),
            tags: Set(tags),
            meta_title: Set(meta_title),
            meta_description: Set(meta_description),
            created_at: Set(Utc::now()),
            updated_at: Set(Some(Utc::now())),
        };
        
        // Insert the product
        let result = product.insert(db).await.map_err(|e| {
            let msg = format!("Failed to create product: {}", e);
            error!(%msg);
            ServiceError::db_error(msg)
        })?;
        
        // Publish event
        self.event_sender.send(Event::with_data(format!("ProductCreated:{}", result.id))).await
            .map_err(|e| {
                let msg = format!("Failed to publish product created event: {}", e);
                error!(%msg);
                ServiceError::EventError(msg)
            })?;
            
        info!(product_id = %result.id, name = %name, sku = %sku, "Product created successfully");
        
        Ok(result.id)
    }
    
    /// Get a product by ID
    #[instrument(skip(self))]
    pub async fn get_product(&self, id: &Uuid) -> Result<Option<product::Model>, ServiceError> {
        let db = &*self.db_pool;
        
        let product = Product::find_by_id(*id)
            .one(db)
            .await
            .map_err(|e| {
                let msg = format!("Failed to get product: {}", e);
                error!(product_id = %id, error = %e, "Database error when fetching product");
                ServiceError::db_error(msg)
            })?;
            
        Ok(product)
    }
    
    /// Get a product by SKU
    #[instrument(skip(self))]
    pub async fn get_product_by_sku(&self, sku: &str) -> Result<Option<product::Model>, ServiceError> {
        let db = &*self.db_pool;
        
        let product = Product::find()
            .filter(ProductColumn::Sku.eq(sku))
            .one(db)
            .await
            .map_err(|e| {
                let msg = format!("Failed to get product by SKU: {}", e);
                error!(sku = %sku, error = %e, "Database error when fetching product by SKU");
                ServiceError::db_error(msg)
            })?;
            
        Ok(product)
    }
    
    /// List products with pagination
    #[instrument(skip(self))]
    pub async fn list_products(
        &self,
        page: u64,
        limit: u64,
        category_id: Option<Uuid>,
        is_active: Option<bool>,
        search_term: Option<String>,
    ) -> Result<(Vec<product::Model>, u64), ServiceError> {
        let db = &*self.db_pool;
        
        // Start building the query
        let mut query = Product::find();
        
        // Add filters if provided
        if let Some(category_id) = category_id {
            query = query.filter(ProductColumn::CategoryId.eq(category_id));
        }
        
        if let Some(is_active) = is_active {
            query = query.filter(ProductColumn::IsActive.eq(is_active));
        }
        
        if let Some(search_term) = search_term {
            let search_pattern = format!("%{}%", search_term);
            query = query.filter(
                ProductColumn::Name.contains(&search_pattern)
                    .or(ProductColumn::Sku.contains(&search_pattern))
                    .or(ProductColumn::Description.contains(&search_pattern))
            );
        }
        
        // Order by most recently created
        query = query.order_by_desc(ProductColumn::CreatedAt);
        
        // Create a paginator
        let paginator = query.paginate(db, limit);
        
        // Get the total count
        let total = paginator.num_items().await.map_err(|e| {
            let msg = format!("Failed to count products: {}", e);
            error!(error = %e, "Database error when counting products");
            ServiceError::db_error(msg)
        })?;
        
        // Get the requested page
        let products = paginator.fetch_page(page - 1).await.map_err(|e| {
            let msg = format!("Failed to fetch products: {}", e);
            error!(page = %page, limit = %limit, error = %e, "Database error when fetching products");
            ServiceError::db_error(msg)
        })?;
        
        Ok((products, total))
    }
    
    /// Update a product
    #[instrument(skip(self))]
    pub async fn update_product(
        &self,
        id: Uuid,
        name: Option<String>,
        description: Option<String>,
        price: Option<Decimal>,
        currency: Option<String>,
        weight_kg: Option<Decimal>,
        dimensions_cm: Option<String>,
        barcode: Option<String>,
        brand: Option<String>,
        manufacturer: Option<String>,
        is_active: Option<bool>,
        is_digital: Option<bool>,
        image_url: Option<String>,
        category_id: Option<Uuid>,
        reorder_point: Option<i32>,
        tax_rate: Option<Decimal>,
        cost_price: Option<Decimal>,
        msrp: Option<Decimal>,
        tags: Option<String>,
        meta_title: Option<String>,
        meta_description: Option<String>,
    ) -> Result<product::Model, ServiceError> {
        let db = &*self.db_pool;
        
        // Find the product
        let product = Product::find_by_id(id)
            .one(db)
            .await
            .map_err(|e| {
                let msg = format!("Failed to find product: {}", e);
                error!(product_id = %id, error = %e, "Database error when finding product");
                ServiceError::db_error(msg)
            })?
            .ok_or_else(|| {
                let msg = format!("Product with ID {} not found", id);
                error!(%msg);
                ServiceError::NotFound(msg)
            })?;
            
        // Update the product
        let mut product: product::ActiveModel = product.into();
        
        if let Some(name) = name {
            product.name = Set(name);
        }
        
        if let Some(description) = description {
            product.description = Set(Some(description));
        }
        
        if let Some(price) = price {
            product.price = Set(price);
        }
        
        if let Some(currency) = currency {
            product.currency = Set(currency);
        }
        
        if let Some(weight_kg) = weight_kg {
            product.weight_kg = Set(Some(weight_kg));
        }
        
        if let Some(dimensions_cm) = dimensions_cm {
            product.dimensions_cm = Set(Some(dimensions_cm));
        }
        
        if let Some(barcode) = barcode {
            product.barcode = Set(Some(barcode));
        }
        
        if let Some(brand) = brand {
            product.brand = Set(Some(brand));
        }
        
        if let Some(manufacturer) = manufacturer {
            product.manufacturer = Set(Some(manufacturer));
        }
        
        if let Some(is_active) = is_active {
            product.is_active = Set(is_active);
        }
        
        if let Some(is_digital) = is_digital {
            product.is_digital = Set(is_digital);
        }
        
        if let Some(image_url) = image_url {
            product.image_url = Set(Some(image_url));
        }
        
        if let Some(category_id) = category_id {
            product.category_id = Set(Some(category_id));
        }
        
        if let Some(reorder_point) = reorder_point {
            product.reorder_point = Set(Some(reorder_point));
        }
        
        if let Some(tax_rate) = tax_rate {
            product.tax_rate = Set(Some(tax_rate));
        }
        
        if let Some(cost_price) = cost_price {
            product.cost_price = Set(Some(cost_price));
        }
        
        if let Some(msrp) = msrp {
            product.msrp = Set(Some(msrp));
        }
        
        if let Some(tags) = tags {
            product.tags = Set(Some(tags));
        }
        
        if let Some(meta_title) = meta_title {
            product.meta_title = Set(Some(meta_title));
        }
        
        if let Some(meta_description) = meta_description {
            product.meta_description = Set(Some(meta_description));
        }
        
        product.updated_at = Set(Some(Utc::now()));
        
        // Save the updated product
        let updated_product = product.update(db).await.map_err(|e| {
            let msg = format!("Failed to update product: {}", e);
            error!(product_id = %id, error = %e, "Database error when updating product");
            ServiceError::db_error(msg)
        })?;
        
        // Publish event
        self.event_sender.send(Event::with_data(format!("ProductUpdated:{}", updated_product.id))).await
            .map_err(|e| {
                let msg = format!("Failed to publish product updated event: {}", e);
                error!(%msg);
                ServiceError::EventError(msg)
            })?;
            
        info!(product_id = %updated_product.id, "Product updated successfully");
        
        Ok(updated_product)
    }
    
    /// Delete a product
    #[instrument(skip(self))]
    pub async fn delete_product(&self, id: Uuid) -> Result<(), ServiceError> {
        let db = &*self.db_pool;
        
        // Find the product
        let product = Product::find_by_id(id)
            .one(db)
            .await
            .map_err(|e| {
                let msg = format!("Failed to find product: {}", e);
                error!(product_id = %id, error = %e, "Database error when finding product");
                ServiceError::db_error(msg)
            })?
            .ok_or_else(|| {
                let msg = format!("Product with ID {} not found", id);
                error!(%msg);
                ServiceError::NotFound(msg)
            })?;
            
        // Delete the product
        product.delete(db).await.map_err(|e| {
            let msg = format!("Failed to delete product: {}", e);
            error!(product_id = %id, error = %e, "Database error when deleting product");
            ServiceError::db_error(msg)
        })?;
        
        // Publish event
        self.event_sender.send(Event::with_data(format!("ProductDeleted:{}", id))).await
            .map_err(|e| {
                let msg = format!("Failed to publish product deleted event: {}", e);
                error!(%msg);
                ServiceError::EventError(msg)
            })?;
            
        info!(product_id = %id, "Product deleted successfully");
        
        Ok(())
    }
}

#[cfg(all(test, feature = "mock-tests"))]
mod tests {
    use super::*;
    use tokio::sync::broadcast;
    use rust_decimal_macros::dec;
    
    #[tokio::test]
    async fn test_create_product() {
        // Create a mock event sender
        let (sender, _) = broadcast::channel(10);
        let event_sender = Arc::new(sender);
        
        // TODO: Add test implementation
    }
}
