use std::net::SocketAddr;
use tonic::{transport::Server, Request, Response, Status};
use tracing::info;

use stateset_api::proto::order::{
    order_service_server::{OrderService, OrderServiceServer},
    CreateOrderRequest, CreateOrderResponse, GetOrderRequest, GetOrderResponse, ListOrdersRequest,
    ListOrdersResponse, Order, OrderItem, OrderStatus, UpdateOrderStatusRequest,
    UpdateOrderStatusResponse,
};

#[derive(Clone, Default)]
struct MockOrderApi;

#[tonic::async_trait]
impl OrderService for MockOrderApi {
    async fn create_order(
        &self,
        request: Request<CreateOrderRequest>,
    ) -> Result<Response<CreateOrderResponse>, Status> {
        let _req = request.into_inner();
        let resp = CreateOrderResponse {
            order_id: format!("ord_{}", uuid::Uuid::new_v4().simple()),
            status: OrderStatus::Pending as i32,
            created_at: Some(prost_types::Timestamp {
                seconds: chrono::Utc::now().timestamp(),
                nanos: 0,
            }),
        };
        Ok(Response::new(resp))
    }

    async fn get_order(
        &self,
        request: Request<GetOrderRequest>,
    ) -> Result<Response<GetOrderResponse>, Status> {
        let order_id = request.into_inner().order_id;
        let order = Order {
            id: order_id,
            customer_id: "customer_123".to_string(),
            items: vec![OrderItem {
                product_id: "prod_1".to_string(),
                quantity: 1,
                unit_price: None,
            }],
            total_amount: None,
            status: OrderStatus::Processing as i32,
            created_at: Some(prost_types::Timestamp {
                seconds: chrono::Utc::now().timestamp(),
                nanos: 0,
            }),
            shipping_address: None,
            billing_address: None,
            payment_method_id: String::new(),
            shipment_id: String::new(),
        };
        Ok(Response::new(GetOrderResponse { order: Some(order) }))
    }

    async fn update_order_status(
        &self,
        request: Request<UpdateOrderStatusRequest>,
    ) -> Result<Response<UpdateOrderStatusResponse>, Status> {
        let req = request.into_inner();
        let resp = UpdateOrderStatusResponse {
            order_id: req.order_id,
            status: req.new_status,
        };
        Ok(Response::new(resp))
    }

    async fn list_orders(
        &self,
        _request: Request<ListOrdersRequest>,
    ) -> Result<Response<ListOrdersResponse>, Status> {
        Ok(Response::new(ListOrdersResponse {
            orders: vec![],
            pagination: None,
        }))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let host = std::env::var("GRPC_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port: u16 = std::env::var("GRPC_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(8081);
    let addr: SocketAddr = format!("{}:{}", host, port).parse()?;

    info!("Starting Orders Mock gRPC server on {}", addr);

    let svc = OrderServiceServer::new(MockOrderApi::default());

    Server::builder().add_service(svc).serve(addr).await?;
    Ok(())
}
