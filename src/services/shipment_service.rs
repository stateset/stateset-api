use crate::models::Shipment;
use crate::db::DbPool;
use crate::errors::ApiError;
use crate::events::{EventSender, Event};

pub struct ShipmentService {
    db_pool: Arc<DbPool>,
    event_sender: EventSender,
}

impl ShipmentService {
    pub fn new(db_pool: Arc<DbPool>, event_sender: EventSender) -> Self {
        Self { db_pool, event_sender }
    }

    pub async fn create_shipment(&self, order_id: Uuid) -> Result<Shipment, ApiError> {
        let conn = self.db_pool.get().map_err(|_| ApiError::DatabaseError)?;

        let shipment = conn.transaction::<_, ApiError, _>(|| {
            let shipment = diesel::insert_into(shipments::table)
                .values(&NewShipment { order_id })
                .get_result::<Shipment>(&conn)?;

            Ok(shipment)
        })?;

        self.event_sender.send(Event::ShipmentCreated(shipment.id))?;

        Ok(shipment)
    }

    pub async fn update_shipment_status(&self, id: Uuid, new_status: ShipmentStatus) -> Result<Shipment, ApiError> {
        let conn = self.db_pool.get().map_err(|_| ApiError::DatabaseError)?;

        let shipment = conn.transaction::<_, ApiError, _>(|| {
            let shipment = diesel::update(shipments::table.find(id))
                .set(shipments::status.eq(new_status))
                .get_result::<Shipment>(&conn)?;

            Ok(shipment)
        })?;

        self.event_sender.send(Event::ShipmentUpdated(id))?;

        Ok(shipment)
    }
}

pub async fn create_shipment(pool: &DbPool, new_shipment: NewShipment) -> Result<Shipment, ServiceError> {
    let conn = pool.get()?;
    let shipment = conn.transaction::<_, diesel::result::Error, _>(|| {
        let shipment = diesel::insert_into(shipments::table)
            .values(&new_shipment)
            .get_result::<Shipment>(&conn)?;

        // Update the associated order's status
        update_order_status(&conn, shipment.order_id, "Shipped")?;

        Ok(shipment)
    })?;

    // Notify customer about shipment
    notify_shipment_created(&shipment).await?;

    Ok(shipment)
}

pub async fn get_shipment(pool: &DbPool, id: i32) -> Result<Shipment, ServiceError> {
    let conn = pool.get()?;
    let shipment = shipments::table
        .filter(shipments::id.eq(id))
        .first::<Shipment>(&conn)?;
    Ok(shipment)
}

pub async fn update_shipment(pool: &DbPool, id: i32, updated_shipment: Shipment) -> Result<Shipment, ServiceError> {
    let conn = pool.get()?;
    let shipment = diesel::update(shipments::table)
        .filter(shipments::id.eq(id))
        .set(&updated_shipment)
        .get_result::<Shipment>(&conn)?;

    // If status changed to Delivered, update the order status
    if updated_shipment.status == ShipmentStatus::Delivered {
        update_order_status(&conn, shipment.order_id, "Delivered")?;
    }

    Ok(shipment)
}

pub async fn delete_shipment(pool: &DbPool, id: i32) -> Result<(), ServiceError> {
    let conn = pool.get()?;
    diesel::delete(shipments::table)
        .filter(shipments::id.eq(id))
        .execute(&conn)?;
    Ok(())
}

pub async fn list_shipments(pool: &DbPool, pagination: PaginationParams) -> Result<Vec<Shipment>, ServiceError> {
    let conn = pool.get()?;
    let shipments = shipments::table
        .order(shipments::id.desc())
        .limit(pagination.limit)
        .offset(pagination.offset)
        .load::<Shipment>(&conn)?;
    Ok(shipments)
}

pub async fn search_shipments(pool: &DbPool, search: ShipmentSearchParams) -> Result<Vec<Shipment>, ServiceError> {
    let conn = pool.get()?;
    let mut query = shipments::table.into_boxed();
    
    if let Some(order_id) = search.order_id {
        query = query.filter(shipments::order_id.eq(order_id));
    }
    
    if let Some(tracking_number) = search.tracking_number {
        query = query.filter(shipments::tracking_number.eq(tracking_number));
    }
    
    if let Some(carrier) = search.carrier {
        query = query.filter(shipments::carrier.eq(carrier));
    }
    
    if let Some(status) = search.status {
        query = query.filter(shipments::status.eq(status));
    }
    
    if let Some(shipped_after) = search.shipped_after {
        query = query.filter(shipments::shipped_at.ge(shipped_after));
    }
    
    if let Some(shipped_before) = search.shipped_before {
        query = query.filter(shipments::shipped_at.le(shipped_before));
    }
    
    let shipments = query
        .order(shipments::id.desc())
        .limit(search.limit)
        .offset(search.offset)
        .load::<Shipment>(&conn)?;
    
    Ok(shipments)
}