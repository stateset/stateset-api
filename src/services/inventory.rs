use crate::models::InventoryItem;
use crate::db::DbPool;
use crate::cache::Cache;
use crate::errors::ApiError;
use crate::events::{EventSender, Event};

pub struct InventoryService {
    db_pool: Arc<DbPool>,
    cache: Arc<dyn Cache>,
    event_sender: EventSender,
}

impl InventoryService {
    pub fn new(db_pool: Arc<DbPool>, cache: Arc<dyn Cache>, event_sender: EventSender) -> Self {
        Self { db_pool, cache, event_sender }
    }

    pub async fn get_inventory(&self, product_id: Uuid) -> Result<InventoryItem, ApiError> {
        if let Some(cached_item) = self.cache.get(&format!("inventory:{}", product_id)).await? {
            return Ok(serde_json::from_str(&cached_item)?);
        }

        let conn = self.db_pool.get().map_err(|_| ApiError::DatabaseError)?;
        let item = inventory::table.find(product_id).first::<InventoryItem>(&conn)?;

        self.cache.set(&format!("inventory:{}", product_id), &serde_json::to_string(&item)?, None).await?;

        Ok(item)
    }

    pub async fn update_inventory(&self, product_id: Uuid, quantity_change: i32) -> Result<InventoryItem, ApiError> {
        let conn = self.db_pool.get().map_err(|_| ApiError::DatabaseError)?;

        let item = conn.transaction::<_, ApiError, _>(|| {
            let item = diesel::update(inventory::table.find(product_id))
                .set(inventory::quantity.eq(inventory::quantity + quantity_change))
                .get_result::<InventoryItem>(&conn)?;

            if item.quantity < item.reorder_threshold {
                self.event_sender.send(Event::LowInventory(product_id))?;
            }

            Ok(item)
        })?;

        self.cache.set(&format!("inventory:{}", product_id), &serde_json::to_string(&item)?, None).await?;

        self.event_sender.send(Event::InventoryUpdated(product_id))?;

        Ok(item)
    }

    pub async fn reserve_inventory(&self, product_id: Uuid, quantity: i32) -> Result<(), ApiError> {
        self.update_inventory(product_id, -quantity).await?;
        Ok(())
    }

    pub async fn release_inventory(&self, product_id: Uuid, quantity: i32) -> Result<(), ApiError> {
        self.update_inventory(product_id, quantity).await?;
        Ok(())
    }
}

    fn create_reorder_work_order(&self, conn: &PgConnection, inventory_item: &InventoryItem) -> Result<(), ServiceError> {
        diesel::insert_into(work_orders::table)
            .values(&WorkOrder {
                product_id: inventory_item.product_id,
                quantity: inventory_item.reorder_quantity,
                status: WorkOrderStatus::Pending,
                created_at: chrono::Utc::now().naive_utc(),
                updated_at: chrono::Utc::now().naive_utc(),
            })
            .execute(conn)?;

        Ok(())
    }

    pub async fn get_low_stock_products(&self) -> Result<Vec<InventoryItem>, ServiceError> {
        let conn = self.pool.get()?;
        
        let low_stock_products = inventory::table
            .filter(inventory::quantity.le(inventory::reorder_point))
            .load::<InventoryItem>(&conn)?;

        Ok(low_stock_products)
    }

    pub async fn reorder_products(&self) -> Result<Vec<WorkOrder>, ServiceError> {
        let conn = self.pool.get()?;
        
        let low_stock_products = self.get_low_stock_products().await?;
        let mut work_orders = Vec::new();

        for product in low_stock_products {
            let work_order = diesel::insert_into(work_orders::table)
                .values(&WorkOrder {
                    product_id: product.product_id,
                    quantity: product.reorder_quantity,
                    status: WorkOrderStatus::Pending,
                    created_at: chrono::Utc::now().naive_utc(),
                    updated_at: chrono::Utc::now().naive_utc(),
                })
                .get_result::<WorkOrder>(&conn)?;

            work_orders.push(work_order);
        }

        Ok(work_orders)
    }
}

pub async fn create_product(pool: &DbPool, new_product: NewProduct) -> Result<Product, ServiceError> {
    let conn = pool.get()?;
    let product = diesel::insert_into(products::table)
        .values(&new_product)
        .get_result::<Product>(&conn)?;
    Ok(product)
}

pub async fn get_product(pool: &DbPool, id: i32) -> Result<Product, ServiceError> {
    let conn = pool.get()?;
    let product = products::table
        .filter(products::id.eq(id))
        .first::<Product>(&conn)?;
    Ok(product)
}

pub async fn update_product(pool: &DbPool, id: i32, updated_product: Product) -> Result<Product, ServiceError> {
    let conn = pool.get()?;
    let product = diesel::update(products::table)
        .filter(products::id.eq(id))
        .set(&updated_product)
        .get_result::<Product>(&conn)?;
    Ok(product)
}

pub async fn delete_product(pool: &DbPool, id: i32) -> Result<(), ServiceError> {
    let conn = pool.get()?;
    diesel::delete(products::table)
        .filter(products::id.eq(id))
        .execute(&conn)?;
    Ok(())
}

pub async fn list_products(pool: &DbPool, pagination: PaginationParams) -> Result<Vec<Product>, ServiceError> {
    let conn = pool.get()?;
    let products = products::table
        .order(products::id.desc())
        .limit(pagination.limit)
        .offset(pagination.offset)
        .load::<Product>(&conn)?;
    Ok(products)
}

pub async fn search_products(pool: &DbPool, search: ProductSearchParams) -> Result<Vec<Product>, ServiceError> {
    let conn = pool.get()?;
    let mut query = products::table.into_boxed();
    
    if let Some(sku) = search.sku {
        query = query.filter(products::sku.eq(sku));
    }
    
    if let Some(name) = search.name {
        query = query.filter(products::name.ilike(format!("%{}%", name)));
    }
    
    if let Some(min_price) = search.min_price {
        query = query.filter(products::price.ge(min_price));
    }
    
    if let Some(max_price) = search.max_price {
        query = query.filter(products::price.le(max_price));
    }
    
    if let Some(in_stock) = search.in_stock {
        if in_stock {
            query = query.filter(products::stock_quantity.gt(0));
        } else {
            query = query.filter(products::stock_quantity.eq(0));
        }
    }
    
    let products = query
        .order(products::id.desc())
        .limit(search.limit)
        .offset(search.offset)
        .load::<Product>(&conn)?;
    
    Ok(products)
}

pub async fn adjust_stock(pool: &DbPool, adjustment: StockAdjustment) -> Result<Product, ServiceError> {
    let conn = pool.get()?;
    let product = conn.transaction::<_, diesel::result::Error, _>(|| {
        let mut product = products::table
            .filter(products::id.eq(adjustment.product_id))
            .first::<Product>(&conn)?;
        
        product.stock_quantity += adjustment.quantity_change;
        
        if product.stock_quantity < 0 {
            return Err(diesel::result::Error::RollbackTransaction);
        }
        
        diesel::update(products::table)
            .filter(products::id.eq(adjustment.product_id))
            .set(products::stock_quantity.eq(product.stock_quantity))
            .execute(&conn)?;
        
        Ok(product)
    })?;
    
    // Log the stock adjustment
    log_stock_adjustment(&adjustment).await?;
    
    Ok(product)
}

async fn log_stock_adjustment(adjustment: &StockAdjustment) -> Result<(), ServiceError> {
    // Implement logging logic for stock adjustments
    Ok(())
}

pub async fn create_order(pool: &DbPool, new_order: NewOrder, user_id: i32) -> Result<Order, ServiceError> {
    let conn = pool.get()?;
    let order = conn.transaction::<_, diesel::result::Error, _>(|| {
        let order = diesel::insert_into(orders::table)
            .values(&new_order)
            .get_result::<Order>(&conn)?;
        
        // Assuming new_order contains order items
        for item in &new_order.items {
            let adjustment = StockAdjustment {
                product_id: item.product_id,
                quantity_change: -item.quantity, // Decrease stock
                reason: Some(format!("Order {}", order.id)),
            };
            adjust_stock(pool, adjustment).await?;
        }
        
        Ok(order)
    })?;
    
    notify_new_order(&order).await?;
    
    Ok(order)
}

#[async_trait]
impl EventHandler for InventoryService {
    async fn handle_event(&self, event: Event) {
        match event {
            Event::OrderCreated(order_id) => {
                // Implement logic to reserve inventory for the order
            },
            Event::OrderCancelled(order_id) => {
                // Implement logic to release reserved inventory
            },
            Event::ReturnProcessed(return_id) => {
                // Implement logic to adjust inventory for processed returns
            },
            Event::WorkOrderCompleted(work_order_id) => {
                // Implement logic to update inventory after work order completion
            },
            _ => {}
        }
    }
}