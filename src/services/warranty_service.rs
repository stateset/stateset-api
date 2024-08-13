use crate::models::Warranty;
use crate::db::DbPool;
use crate::errors::ApiError;
use crate::events::{EventSender, Event};

pub struct WarrantyService {
    db_pool: Arc<DbPool>,
    event_sender: EventSender,
}

impl WarrantyService {
    pub fn new(db_pool: Arc<DbPool>, event_sender: EventSender) -> Self {
        Self { db_pool, event_sender }
    }

    pub async fn create_warranty(&self, new_warranty: NewWarranty) -> Result<Warranty, ApiError> {
        let conn = self.db_pool.get().map_err(|_| ApiError::DatabaseError)?;

        let warranty = conn.transaction::<_, ApiError, _>(|| {
            let warranty = diesel::insert_into(warranties::table)
                .values(&new_warranty)
                .get_result::<Warranty>(&conn)?;

            Ok(warranty)
        })?;

        self.event_sender.send(Event::WarrantyCreated(warranty.id))?;

        Ok(warranty)
    }

    pub async fn claim_warranty(&self, id: Uuid) -> Result<Warranty, ApiError> {
        let conn = self.db_pool.get().map_err(|_| ApiError::DatabaseError)?;

        let warranty = conn.transaction::<_, ApiError, _>(|| {
            let warranty = diesel::update(warranties::table.find(id))
                .set(warranties::status.eq(WarrantyStatus::Claimed))
                .get_result::<Warranty>(&conn)?;

            Ok(warranty)
        })?;

        self.event_sender.send(Event::WarrantyClaimed(id))?;

        Ok(warranty)
    }
}
