use actix_web::{post, get, put, delete, web, HttpResponse};
use crate::db::DbPool;
use crate::models::work_order::{NewWorkOrder, WorkOrder, WorkOrderSearchParams, WorkOrderStatus};
use crate::errors::ServiceError;
use crate::services::work_orders::{create_work_order, get_work_order, update_work_order, delete_work_order, list_work_orders, search_work_orders, assign_work_order, complete_work_order};
use crate::auth::AuthenticatedUser;
use validator::Validate;

#[post("")]
async fn create_work_order(
    db_pool: web::Data<Arc<DbPool>>,
    work_order_info: web::Json<NewWorkOrder>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, ServiceError> {
    work_order_info.validate()?;

    let command = CreateWorkOrderCommand {
        work_order_info: work_order_info.into_inner(),
        user_id: user.user_id,
    };

    let created_work_order = command.execute(db_pool.get_ref().clone()).await?;
    Ok(HttpResponse::Created().json(created_work_order))
}

#[get("/{id}")]
async fn get_work_order(
    db_pool: web::Data<Arc<DbPool>>,
    id: web::Path<i32>,
) -> Result<HttpResponse, ServiceError> {
    let command = GetWorkOrderCommand { id: id.into_inner() };
    let work_order = command.execute(db_pool.get_ref().clone()).await?;
    Ok(HttpResponse::Ok().json(work_order))
}

#[put("/{id}")]
async fn update_work_order(
    db_pool: web::Data<Arc<DbPool>>,
    id: web::Path<i32>,
    work_order_info: web::Json<WorkOrder>,
) -> Result<HttpResponse, ServiceError> {
    work_order_info.validate()?;

    let command = UpdateWorkOrderCommand {
        id: id.into_inner(),
        work_order_info: work_order_info.into_inner(),
    };

    let updated_work_order = command.execute(db_pool.get_ref().clone()).await?;
    Ok(HttpResponse::Ok().json(updated_work_order))
}

#[delete("/{id}")]
async fn delete_work_order(
    db_pool: web::Data<Arc<DbPool>>,
    id: web::Path<i32>,
) -> Result<HttpResponse, ServiceError> {
    let command = DeleteWorkOrderCommand { id: id.into_inner() };
    command.execute(db_pool.get_ref().clone()).await?;
    Ok(HttpResponse::NoContent().finish())
}

#[get("")]
async fn list_work_orders(
    pool: web::Data<DbPool>,
    query: web::Query<PaginationParams>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, ServiceError> {
    let work_orders = list_work_orders(&pool, query.into_inner()).await?;
    Ok(HttpResponse::Ok().json(work_orders))
}

#[get("/search")]
async fn search_work_orders(
    pool: web::Data<DbPool>,
    query: web::Query<WorkOrderSearchParams>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, ServiceError> {
    let work_orders = search_work_orders(&pool, query.into_inner()).await?;
    Ok(HttpResponse::Ok().json(work_orders))
}

#[post("/{id}/assign")]
async fn assign_work_order(
    db_pool: web::Data<Arc<DbPool>>,
    work_order_id: web::Path<i32>,
    user_id: web::Json<i32>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, ServiceError> {
    let command = AssignWorkOrderCommand {
        work_order_id: work_order_id.into_inner(),
        user_id: user_id.into_inner(),
    };

    let result = command.execute(db_pool.get_ref().clone()).await?;
    Ok(HttpResponse::Ok().json(result))
}

#[post("/{id}/unassign")]
async fn unassign_work_order(
    db_pool: web::Data<Arc<DbPool>>,
    work_order_id: web::Path<i32>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, ServiceError> {
    let command = UnassignWorkOrderCommand {
        work_order_id: work_order_id.into_inner(),
    };

    let result = command.execute(db_pool.get_ref().clone()).await?;
    Ok(HttpResponse::Ok().json(result))
}

#[post("/{id}/complete")]
async fn complete_work_order(
    db_pool: web::Data<Arc<DbPool>>,
    work_order_id: web::Path<i32>,
    actual_duration: web::Json<i32>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, ServiceError> {
    let command = CompleteWorkOrderCommand {
        work_order_id: work_order_id.into_inner(),
        actual_duration: actual_duration.into_inner(),
        completed_by: user.user_id,
    };

    let result = command.execute(db_pool.get_ref().clone()).await?;
    Ok(HttpResponse::Ok().json(result))
}
