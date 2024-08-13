pub struct RefundOrderCommand {
    pub order_id: i32,
    pub refund_amount: f64,
    pub reason: String,
}

#[async_trait]
impl Command for RefundOrderCommand {
    type Result = Order;

    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;

        // Update the order to reflect the refund
        let updated_order = diesel::update(orders::table.find(self.order_id))
            .set(orders::total_amount.eq(orders::total_amount - self.refund_amount))
            .get_result::<Order>(&conn)
            .map_err(|e| ServiceError::DatabaseError)?;

        // Log the refund reason
        diesel::insert_into(order_notes::table)
            .values(&NewOrderNote { order_id: self.order_id, note: format!("Refunded: {} - Reason: {}", self.refund_amount, self.reason) })
            .execute(&conn)
            .map_err(|e| ServiceError::DatabaseError)?;

        event_sender.send(Event::OrderRefunded(self.order_id)).await.map_err(|e| ServiceError::EventError(e.to_string()))?;

        Ok(updated_order)
    }
}