#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct AddSupplierProductCommand {
    pub supplier_id: i32,
    pub product_id: i32,
    #[validate(range(min = 0.0))]
    pub price: f64,
    #[validate(range(min = 1))]
    pub lead_time_days: i32,
}

#[async_trait]
impl Command for AddSupplierProductCommand {
    type Result = SupplierProduct;

    #[instrument(skip(db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        self.validate().map_err(|e| {
            error!("Validation error: {}", e);
            ServiceError::ValidationError(e.to_string())
        })?;

        let conn = db_pool.get().map_err(|e| {
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError
        })?;

        let supplier_product = SupplierProduct {
            supplier_id: self.supplier_id,
            product_id: self.product_id,
            price: self.price,
            lead_time_days: self.lead_time_days,
        };

        let saved_supplier_product = match diesel::insert_into(supplier_products::table)
            .values(&supplier_product)
            .get_result::<SupplierProduct>(&conn) {
            Ok(sp) => sp,
            Err(e) => {
                error!("Failed to save supplier product: {}", e);
                return Err(ServiceError::DatabaseError);
            }
        };

        if let Err(e) = event_sender.send(Event::SupplierProductAdded(self.supplier_id, self.product_id)).await {
            error!("Failed to send SupplierProductAdded event: {}", e);
            return Err(ServiceError::EventError(e.to_string()));
        }

        info!("Supplier product added successfully: {:?}", saved_supplier_product);
        Ok(saved_supplier_product)
    }
}