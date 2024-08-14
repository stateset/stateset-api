// RemoveSupplierProductCommand: Handles removing a product from a supplier's catalog
#[derive(Debug, Serialize, Deserialize)]
pub struct RemoveSupplierProductCommand {
    pub supplier_id: i32,
    pub product_id: i32,
}

#[async_trait]
impl Command for RemoveSupplierProductCommand {
    type Result = ();

    #[instrument(skip(db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|e| {
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError
        })?;

        let deleted_count = diesel::delete(
            supplier_products::table
                .filter(supplier_products::supplier_id.eq(self.supplier_id))
                .filter(supplier_products::product_id.eq(self.product_id))
        )
        .execute(&conn)
        .map_err(|e| {
            error!("Failed to remove supplier product: {}", e);
            ServiceError::DatabaseError
        })?;

        if deleted_count == 0 {
            return Err(ServiceError::NotFound);
        }

        if let Err(e) = event_sender.send(Event::SupplierProductRemoved(self.supplier_id, self.product_id)).await {
            error!("Failed to send SupplierProductRemoved event: {}", e);
            return Err(ServiceError::EventError(e.to_string()));
        }

        info!("Supplier product removed successfully: supplier_id={}, product_id={}", self.supplier_id, self.product_id);
        Ok(())
    }
}