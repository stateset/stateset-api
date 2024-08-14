#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteSupplierCommand {
    pub id: i32,
}

#[async_trait]
impl Command for DeleteSupplierCommand {
    type Result = ();

    #[instrument(skip(db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|e| {
            SUPPLIER_DELETION_FAILURES.inc();
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError
        })?;

        let deleted_count = diesel::delete(suppliers::table.find(self.id))
            .execute(&conn)
            .map_err(|e| {
                SUPPLIER_DELETION_FAILURES.inc();
                error!("Failed to delete supplier: {}", e);
                ServiceError::DatabaseError
            })?;

        if deleted_count == 0 {
            SUPPLIER_DELETION_FAILURES.inc();
            return Err(ServiceError::NotFound);
        }

        if let Err(e) = event_sender.send(Event::SupplierDeleted(self.id)).await {
            SUPPLIER_DELETION_FAILURES.inc();
            error!("Failed to send SupplierDeleted event: {}", e);
            return Err(ServiceError::EventError(e.to_string()));
        }

        SUPPLIER_DELETIONS.inc();
        info!("Supplier deleted successfully: id={}", self.id);
        Ok(())
    }
}