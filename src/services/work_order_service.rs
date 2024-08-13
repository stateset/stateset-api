use diesel::prelude::*;
use crate::db::DbPool;
use crate::models::work_order::{NewWorkOrder, WorkOrder, WorkOrderSearchParams, WorkOrderStatus};
use crate::errors::ServiceError;
use crate::schema::work_orders;
use crate::services::inventory::InventoryService;
use crate::utils::PaginationParams;
use std::sync::Arc;
use chrono::NaiveDateTime;

pub struct WorkOrderService {
    pool: DbPool,
    inventory_service: Arc<InventoryService>,
}

impl WorkOrderService {
    pub fn new(pool: DbPool, inventory_service: Arc<InventoryService>) -> Self {
        Self { pool, inventory_service }
    }

    pub async fn create_work_order(&self, new_work_order: NewWorkOrder, created_by: i32) -> Result<WorkOrder, ServiceError> {
        let conn = self.pool.get()?;
        let work_order = diesel::insert_into(work_orders::table)
            .values(&new_work_order)
            .get_result::<WorkOrder>(&conn)?;

        // Notify relevant parties about the new work order
        self.notify_new_work_order(&work_order, created_by).await?;

        Ok(work_order)
    }

    pub async fn get_work_order(&self, id: i32) -> Result<WorkOrder, ServiceError> {
        let conn = self.pool.get()?;
        let work_order = work_orders::table
            .filter(work_orders::id.eq(id))
            .first::<WorkOrder>(&conn)?;
        Ok(work_order)
    }

    pub async fn update_work_order(&self, id: i32, updated_work_order: WorkOrder) -> Result<WorkOrder, ServiceError> {
        let conn = self.pool.get()?;
        let work_order = diesel::update(work_orders::table)
            .filter(work_orders::id.eq(id))
            .set(&updated_work_order)
            .get_result::<WorkOrder>(&conn)?;
        Ok(work_order)
    }

    pub async fn delete_work_order(&self, id: i32) -> Result<(), ServiceError> {
        let conn = self.pool.get()?;
        diesel::delete(work_orders::table)
            .filter(work_orders::id.eq(id))
            .execute(&conn)?;
        Ok(())
    }

    pub async fn list_work_orders(&self, pagination: PaginationParams) -> Result<Vec<WorkOrder>, ServiceError> {
        let conn = self.pool.get()?;
        let work_orders = work_orders::table
            .order(work_orders::id.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load::<WorkOrder>(&conn)?;
        Ok(work_orders)
    }

    pub async fn search_work_orders(&self, search: WorkOrderSearchParams) -> Result<Vec<WorkOrder>, ServiceError> {
        let conn = self.pool.get()?;
        let mut query = work_orders::table.into_boxed();
        
        if let Some(title) = search.title {
            query = query.filter(work_orders::title.ilike(format!("%{}%", title)));
        }
        
        if let Some(status) = search.status {
            query = query.filter(work_orders::status.eq(status));
        }
        
        if let Some(priority) = search.priority {
            query = query.filter(work_orders::priority.eq(priority));
        }
        
        if let Some(assigned_to) = search.assigned_to {
            query = query.filter(work_orders::assigned_to.eq(assigned_to));
        }
        
        if let Some(due_date_from) = search.due_date_from {
            query = query.filter(work_orders::due_date.ge(due_date_from));
        }
        
        if let Some(due_date_to) = search.due_date_to {
            query = query.filter(work_orders::due_date.le(due_date_to));
        }
        
        let work_orders = query
            .order(work_orders::id.desc())
            .limit(search.limit)
            .offset(search.offset)
            .load::<WorkOrder>(&conn)?;
        
        Ok(work_orders)
    }

    pub async fn assign_work_order(&self, id: i32, user_id: i32) -> Result<WorkOrder, ServiceError> {
        let conn = self.pool.get()?;
        let work_order = diesel::update(work_orders::table)
            .filter(work_orders::id.eq(id))
            .set((
                work_orders::assigned_to.eq(user_id),
                work_orders::status.eq(WorkOrderStatus::InProgress),
            ))
            .get_result::<WorkOrder>(&conn)?;

        // Notify the assigned user
        self.notify_work_order_assignment(&work_order).await?;

        Ok(work_order)
    }

    pub async fn complete_work_order(&self, work_order_id: i32) -> Result<WorkOrder, ServiceError> {
        let conn = self.pool.get()?;
        
        conn.transaction(|| {
            let work_order = diesel::update(work_orders::table.filter(work_orders::id.eq(work_order_id)))
                .set(work_orders::status.eq(WorkOrderStatus::Completed))
                .get_result::<WorkOrder>(&conn)?;

            // Update inventory
            self.inventory_service.adjust_stock(work_order.product_id, work_order.quantity).await?;

            Ok(work_order)
        })
    }

    async fn notify_new_work_order(&self, work_order: &WorkOrder, created_by: i32) -> Result<(), ServiceError> {
        // Implement notification logic for new work orders
        Ok(())
    }

    async fn notify_work_order_assignment(&self, work_order: &WorkOrder) -> Result<(), ServiceError> {
        // Implement notification logic for work order assignments
        Ok(())
    }
}