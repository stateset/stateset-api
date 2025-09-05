use utoipa::{OpenApi, ToSchema};
use utoipa_swagger_ui::SwaggerUi;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Stateset API",
        version = "1.0.0",
        description = "Stateset API for order, inventory, and supply chain management"
    ),
    paths(
        crate::handlers::orders::list_orders,
        crate::handlers::orders::get_order
    )
)]
pub struct ApiDocV1;

pub fn swagger_ui() -> SwaggerUi {
    SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDocV1::openapi())
}
