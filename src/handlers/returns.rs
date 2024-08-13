use actix_web::{post, get, put, delete, web, HttpResponse};
use crate::services::returns::ReturnService;
use crate::models::{NewReturn, Return, ReturnStatus, ReturnSearchParams};
use crate::errors::{ServiceError, ReturnError};
use crate::auth::AuthenticatedUser;
use crate::utils::pagination::PaginationParams;
use validator::Validate;
use uuid::Uuid;

// Import the commands
use crate::commands::returns::{
    ApproveReturnCommand,
    RejectReturnCommand,
    CancelReturnCommand,
    CompleteReturnCommand,
    RefundReturnCommand,
    ProcessReturnCommand,
};

#[post("")]
async fn create_return(
    return_info: web::Json<NewReturn>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, ServiceError> {
    let command = CreateReturnCommand {
        return_info: return_info.into_inner(),
        user_id: user.user_id,
    };

    let created_return = command.execute(db_pool.get_ref().clone(), event_sender.get_ref().clone()).await?;
    Ok(HttpResponse::Created().json(created_return))

}

#[post("/{id}/approve")]
async fn approve_return(
    db_pool: web::Data<Arc<DbPool>>,
    event_sender: web::Data<Arc<EventSender>>,
    return_id: web::Path<i32>,
) -> Result<HttpResponse, ServiceError> {
    let command = ApproveReturnCommand {
        return_id: return_id.into_inner(),
    };

    let approved_return = command.execute(db_pool.get_ref().clone(), event_sender.get_ref().clone()).await?;
    Ok(HttpResponse::Ok().json(approved_return))
}

#[post("/{id}/reject")]
async fn reject_return(
    db_pool: web::Data<Arc<DbPool>>,
    event_sender: web::Data<Arc<EventSender>>,
    return_id: web::Path<i32>,
    reject_info: web::Json<RejectReturnCommand>,
) -> Result<HttpResponse, ServiceError> {
    let command = RejectReturnCommand {
        return_id: return_id.into_inner(),
        reason: reject_info.reason.clone(),
    };

    let rejected_return = command.execute(db_pool.get_ref().clone(), event_sender.get_ref().clone()).await?;
    Ok(HttpResponse::Ok().json(rejected_return))
}


#[post("/{id}/cancel")]
async fn cancel_return(
    db_pool: web::Data<Arc<DbPool>>,
    event_sender: web::Data<Arc<EventSender>>,
    return_id: web::Path<i32>,
    cancel_info: web::Json<CancelReturnCommand>,
) -> Result<HttpResponse, ServiceError> {
    let command = CancelReturnCommand {
        return_id: return_id.into_inner(),
        reason: cancel_info.reason.clone(),
    };

    let result = command.execute(db_pool.get_ref().clone(), event_sender.get_ref().clone()).await?;
    Ok(HttpResponse::Ok().json(result))
}

#[get("/{id}")]
async fn get_return(
    return_service: web::Data<ReturnService>,
    id: web::Path<Uuid>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, ServiceError> {
    let return_item = return_service.get_return(id.into_inner(), user.user_id)
        .await
        .map_err(|e| ServiceError::from(ReturnError::from(e)))?;
    Ok(HttpResponse::Ok().json(return_item))
}

#[put("/{id}")]
async fn update_return(
    return_service: web::Data<ReturnService>,
    id: web::Path<Uuid>,
    return_info: web::Json<Return>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, ServiceError> {
    return_info.validate().map_err(|e| ServiceError::BadRequest(e.to_string()))?;
    let updated_return = return_service.update_return(id.into_inner(), return_info.into_inner(), user.user_id)
        .await
        .map_err(|e| ServiceError::from(ReturnError::from(e)))?;
    Ok(HttpResponse::Ok().json(updated_return))
}

#[get("")]
async fn list_returns(
    return_service: web::Data<ReturnService>,
    user: AuthenticatedUser,
    query: web::Query<PaginationParams>,
) -> Result<HttpResponse, ServiceError> {
    let (returns, total) = return_service.list_returns(user.user_id, query.into_inner())
        .await
        .map_err(|e| ServiceError::from(ReturnError::from(e)))?;
    Ok(HttpResponse::Ok().json(json!({
        "returns": returns,
        "total": total,
        "page": query.page,
        "per_page": query.per_page
    })))
}

#[get("/search")]
async fn search_returns(
    return_service: web::Data<ReturnService>,
    user: AuthenticatedUser,
    query: web::Query<ReturnSearchParams>,
    pagination: web::Query<PaginationParams>,
) -> Result<HttpResponse, ServiceError> {
    let (returns, total) = return_service.search_returns(user.user_id, query.into_inner(), pagination.into_inner())
        .await
        .map_err(|e| ServiceError::from(ReturnError::from(e)))?;
    Ok(HttpResponse::Ok().json(json!({
        "returns": returns,
        "total": total,
        "query": query.into_inner(),
        "page": pagination.page,
        "per_page": pagination.per_page
    })))
}

#[post("/{id}/process")]
async fn process_return(
    return_service: web::Data<ReturnService>,
    id: web::Path<Uuid>,
    user: AuthenticatedUser,
    process_info: web::Json<ProcessReturnCommand>,
) -> Result<HttpResponse, ServiceError> {
    let command = ProcessReturnCommand {
        return_id: id.into_inner(),
        process_info: process_info.into_inner(),
    };

    let processed_return = command.execute(db_pool.get_ref().clone(), event_sender.get_ref().clone()).await?;
    Ok(HttpResponse::Ok().json(processed_return))
}

pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/returns")
            .service(create_return)
            .service(get_return)
            .service(update_return)
            .service(list_returns)
            .service(search_returns)
            .service(process_return)
    );
}