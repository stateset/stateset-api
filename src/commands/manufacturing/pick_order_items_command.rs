#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct PickOrderItemsCommand {
    pub order_id: i32,
    #[validate(length(min = 1))]
    pub items_to_pick: Vec<PickedItem>, // Items to pick from inventory
}

#[async_trait]
impl Command for PickOrderItemsCommand {
    type Result = ();

    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;

        // Begin transaction to ensure atomicity
        conn.transaction(|| {
            for item in &self.items_to_pick {
                // Mark items as picked in inventory
                diesel::update(inventory::table.find(item.product_id))
                    .set(inventory::picked_quantity.eq(inventory::picked_quantity + item.quantity))
                    .execute(&conn)
                    .map_err(|e| ServiceError::DatabaseError)?;

                // Log and trigger events for each item
                info!("Item picked for order ID: {}. Product ID: {}, Quantity: {}", self.order_id, item.product_id, item.quantity);
                event_sender.send(Event::OrderItemPicked(self.order_id, item.product_id, item.quantity)).await.map_err(|e| ServiceError::EventError(e.to_string()))?;
            }

            Ok(())
        })
    }
}