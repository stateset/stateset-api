#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct PackOrderCommand {
    pub order_id: i32,
    #[validate(length(min = 1))]
    pub packed_items: Vec<PackedItem>, // Items that have been packed
}

#[async_trait]
impl Command for PackOrderCommand {
    type Result = ();

    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;

        // Begin transaction to ensure atomicity
        conn.transaction(|| {
            for item in &self.packed_items {
                // Mark items as packed in inventory
                diesel::update(inventory::table.find(item.product_id))
                    .set(inventory::packed_quantity.eq(inventory::packed_quantity + item.quantity))
                    .execute(&conn)
                    .map_err(|e| ServiceError::DatabaseError)?;

                // Log and trigger events for each item
                info!("Item packed for order ID: {}. Product ID: {}, Quantity: {}", self.order_id, item.product_id, item.quantity);
                event_sender.send(Event::OrderItemPacked(self.order_id, item.product_id, item.quantity)).await.map_err(|e| ServiceError::EventError(e.to_string()))?;
            }

            // Mark the order as ready for shipment
            diesel::update(orders::table.find(self.order_id))
                .set(orders::status.eq(OrderStatus::ReadyForShipment))
                .execute(&conn)
                .map_err(|e| ServiceError::DatabaseError)?;

            Ok(())
        })
    }
}