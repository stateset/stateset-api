use axum::{
    extract::{Json, Path, Query, State},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;
use validator::Validate;

use crate::{
    errors::ApiError,
    handlers::common::{created_response, map_service_error, success_response, validate_input},
    services::commerce::{CartService, ProductCatalogService},
    AppState,
};

/// Agents API routes: recommend products and act on behalf of customers
pub fn agents_routes() -> Router<AppState> {
    use crate::auth::AuthRouterExt;
    Router::new()
        .route("/recommendations", get(get_recommendations))
        .route(
            "/customers/:customer_id/carts/:cart_id/items",
            post(agent_add_to_cart),
        )
        .with_permission("agents:access")
}

#[derive(Debug, Deserialize, Default)]
pub struct RecommendationsParams {
    pub page: Option<u64>,
    pub per_page: Option<u64>,
    pub search: Option<String>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct RecommendationsResponse {
    pub products: Vec<crate::entities::product::Model>,
    pub total: u64,
}

/// Get product recommendations (simple proxy to product search for now)
async fn get_recommendations(
    State(state): State<AppState>,
    Query(params): Query<RecommendationsParams>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    let per_page = params.per_page.unwrap_or(20);
    let page = params.page.unwrap_or(1);
    let offset = (page.saturating_sub(1)) * per_page;

    // Use a lightweight search using Product entity directly
    use crate::entities::product::{Column as ProductColumn, Entity as Product};
    use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect};

    let mut query = Product::find();
    if let Some(ref search) = params.search {
        let pattern = format!("%{}%", search);
        query = query.filter(
            ProductColumn::Name
                .contains(&pattern)
                .or(ProductColumn::Sku.contains(&pattern)),
        );
    }
    if let Some(active) = params.is_active {
        query = query.filter(ProductColumn::IsActive.eq(active));
    }

    let total = query
        .clone()
        .count(&*state.db)
        .await
        .map_err(|e| crate::errors::ServiceError::db_error(e))
        .map_err(map_service_error)?;

    let products = query
        .order_by_desc(ProductColumn::CreatedAt)
        .limit(per_page)
        .offset(offset)
        .all(&*state.db)
        .await
        .map_err(|e| crate::errors::ServiceError::db_error(e))
        .map_err(map_service_error)?;

    Ok(success_response(RecommendationsResponse {
        products,
        total,
    }))
}

#[derive(Debug, Deserialize, Validate)]
pub struct AgentAddToCartRequest {
    pub variant_id: Uuid,

    pub quantity: i32,
}

/// Agent adds an item to a customer's cart
async fn agent_add_to_cart(
    State(state): State<AppState>,
    Path((_customer_id, cart_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<AgentAddToCartRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&payload)?;

    let input = crate::services::commerce::cart_service::AddToCartInput {
        variant_id: payload.variant_id,
        quantity: payload.quantity,
    };

    let updated = state
        .cart_service()
        .add_item(cart_id, input)
        .await
        .map_err(map_service_error)?;

    Ok(created_response(updated))
}

// Extension methods on AppState to access services used by Agents API
trait AgentsHandlerState {
    fn cart_service(&self) -> CartService;
    fn product_catalog_service(&self) -> ProductCatalogService;
}

impl AgentsHandlerState for AppState {
    fn cart_service(&self) -> CartService {
        crate::services::commerce::CartService::new(
            self.db.clone(),
            Arc::new(self.event_sender.clone()),
            Arc::new(self.config.clone()),
        )
    }
    fn product_catalog_service(&self) -> ProductCatalogService {
        crate::services::commerce::ProductCatalogService::new(
            self.db.clone(),
            Arc::new(self.event_sender.clone()),
        )
    }
}
