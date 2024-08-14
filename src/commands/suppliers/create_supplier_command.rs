


#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreateSupplierCommand {
    #[validate(length(min = 1, max = 255))]
    pub name: String,
    #[validate(email)]
    pub email: String,
    #[validate(length(min = 1, max = 255))]
    pub address: String,
}

#[async_trait]
impl Command for CreateSupplierCommand {
    type Result = Supplier;

    #[instrument(skip(db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        self.validate().map_err(|e| {
            SUPPLIER_CREATION_FAILURES.inc();
            error!("Validation error: {}", e);
            ServiceError::ValidationError(e.to_string())
        })?;

        let conn = db_pool.get().map_err(|e| {
            SUPPLIER_CREATION_FAILURES.inc();
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError
        })?;

        let new_supplier = Supplier {
            name: self.name.clone(),
            email: self.email.clone(),
            address: self.address.clone(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let saved_supplier = match diesel::insert_into(suppliers::table)
            .values(&new_supplier)
            .get_result::<Supplier>(&conn) {
            Ok(supplier) => supplier,
            Err(e) => {
                SUPPLIER_CREATION_FAILURES.inc();
                error!("Failed to save supplier: {}", e);
                return Err(ServiceError::DatabaseError);
            }
        };

        if let Err(e) = event_sender.send(Event::SupplierCreated(saved_supplier.id)).await {
            SUPPLIER_CREATION_FAILURES.inc();
            error!("Failed to send SupplierCreated event: {}", e);
            return Err(ServiceError::EventError(e.to_string()));
        }

        SUPPLIER_CREATIONS.inc();
        info!("Supplier created successfully: {:?}", saved_supplier);
        Ok(saved_supplier)
    }
}