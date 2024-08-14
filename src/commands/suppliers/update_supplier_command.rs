#[async_trait]
impl Command for UpdateSupplierCommand {
    type Result = Supplier;

    #[instrument(skip(db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        self.validate().map_err(|e| {
            SUPPLIER_UPDATE_FAILURES.inc();
            error!("Validation error: {}", e);
            ServiceError::ValidationError(e.to_string())
        })?;

        let conn = db_pool.get().map_err(|e| {
            SUPPLIER_UPDATE_FAILURES.inc();
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError
        })?;

        let updated_supplier = conn.transaction::<_, diesel::result::Error, _>(|| {
            let mut supplier = suppliers::table.find(self.id).first::<Supplier>(&conn)?;

            if let Some(ref name) = self.name {
                supplier.name = name.clone();
            }
            if let Some(ref email) = self.email {
                supplier.email = email.clone();
            }
            if let Some(ref address) = self.address {
                supplier.address = address.clone();
            }
            supplier.updated_at = Utc::now();

            diesel::update(suppliers::table.find(self.id))
                .set(&supplier)
                .execute(&conn)?;

            Ok(supplier)
        }).map_err(|e| {
            SUPPLIER_UPDATE_FAILURES.inc();
            error!("Failed to update supplier: {}", e);
            ServiceError::DatabaseError
        })?;

        if let Err(e) = event_sender.send(Event::SupplierUpdated(updated_supplier.id)).await {
            SUPPLIER_UPDATE_FAILURES.inc();
            error!("Failed to send SupplierUpdated event: {}", e);
            return Err(ServiceError::EventError(e.to_string()));
        }

        SUPPLIER_UPDATES.inc();
        info!("Supplier updated successfully: {:?}", updated_supplier);
        Ok(updated_supplier)
    }
}