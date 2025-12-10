use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{errors::ServiceError, AppState};

#[derive(Serialize, ToSchema)]
pub struct OutboxItem {
    pub id: Uuid,
    pub aggregate_type: String,
    pub aggregate_id: Option<Uuid>,
    pub event_type: String,
    pub status: String,
    pub attempts: i32,
    pub available_at: String,
    pub created_at: String,
    pub error_message: Option<String>,
}

pub fn router() -> Router<crate::AppState> {
    Router::new()
        .route("/", get(list_outbox))
        .route("/:id/retry", post(retry_outbox))
}

#[utoipa::path(
    get,
    path = "/api/v1/admin/outbox",
    responses(
        (status = 200, description = "List outbox items", body = [OutboxItem])
    ),
    security(("bearer_auth" = [])),
    tag = "Admin"
)]
async fn list_outbox(State(state): State<AppState>) -> Result<Json<Vec<OutboxItem>>, ServiceError> {
    let db: &DatabaseConnection = &state.db;
    let sql = r#"
        SELECT id, aggregate_type, aggregate_id, event_type, status, attempts, available_at, created_at, error_message
        FROM outbox_events
        WHERE status IN ('pending','failed','processing')
        ORDER BY created_at DESC
        LIMIT 100
    "#;
    let rows = db
        .query_all(Statement::from_string(DbBackend::Postgres, sql.to_string()))
        .await
        .map_err(ServiceError::db_error)?;

    let mut items = Vec::new();
    for row in rows {
        let id: Uuid = row.try_get("", "id").unwrap_or_default();
        let aggregate_type: String = row.try_get("", "aggregate_type").unwrap_or_default();
        let aggregate_id: Option<Uuid> = row.try_get("", "aggregate_id").ok();
        let event_type: String = row.try_get("", "event_type").unwrap_or_default();
        let status: String = row.try_get("", "status").unwrap_or_default();
        let attempts: i32 = row.try_get("", "attempts").unwrap_or(0);
        let available_at: String = row
            .try_get("", "available_at")
            .map(|d: chrono::DateTime<chrono::Utc>| d.to_rfc3339())
            .unwrap_or_default();
        let created_at: String = row
            .try_get("", "created_at")
            .map(|d: chrono::DateTime<chrono::Utc>| d.to_rfc3339())
            .unwrap_or_default();
        let error_message: Option<String> = row.try_get("", "error_message").ok();
        items.push(OutboxItem {
            id,
            aggregate_type,
            aggregate_id,
            event_type,
            status,
            attempts,
            available_at,
            created_at,
            error_message,
        });
    }

    Ok(Json(items))
}

#[utoipa::path(
    post,
    path = "/api/v1/admin/outbox/:id/retry",
    params(
        ("id" = Uuid, Path, description = "Outbox event id")
    ),
    responses(
        (status = 200, description = "Event scheduled for retry"),
        (status = 404, description = "Not found", body = crate::errors::ErrorResponse)
    ),
    security(("bearer_auth" = [])),
    tag = "Admin"
)]
async fn retry_outbox(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ServiceError> {
    let db: &DatabaseConnection = &state.db;
    let sql = r#"UPDATE outbox_events SET status='pending', available_at = NOW(), updated_at = NOW(), error_message = NULL WHERE id = $1"#;
    let res = db
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            sql,
            vec![id.into()],
        ))
        .await
        .map_err(ServiceError::db_error)?;
    if res.rows_affected() == 0 {
        return Err(ServiceError::NotFound(format!("outbox {} not found", id)));
    }
    Ok(Json(serde_json::json!({"ok": true, "id": id})))
}
