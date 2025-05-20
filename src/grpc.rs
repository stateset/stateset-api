use std::net::SocketAddr;
use tonic::{transport::Server, Request, Response, Status};
use uuid::Uuid;
use crate::services::{self, AppServices};
use crate::proto::{self,
    order_service_server::{OrderService, OrderServiceServer},
    warranty_service_server::{WarrantyService, WarrantyServiceServer},
    inventory_service_server::{InventoryService, InventoryServiceServer},
    return_service_server::{ReturnService, ReturnServiceServer},
    shipment_service_server::{ShipmentService, ShipmentServiceServer},
};

pub struct OrderGrpcService {
    pub svc: services::orders::OrderService,
}

#[tonic::async_trait]
impl OrderService for OrderGrpcService {
    async fn create_order(
        &self,
        request: Request<proto::CreateOrderRequest>,
    ) -> Result<Response<proto::CreateOrderResponse>, Status> {
        let order = request
            .into_inner()
            .order
            .ok_or_else(|| Status::invalid_argument("order missing"))?;
        let customer_id = Uuid::parse_str(&order.customer_id)
            .map_err(|_| Status::invalid_argument("invalid customer_id"))?;
        let items = order
            .items
            .into_iter()
            .map(|i| services::orders::create_order_command::OrderItem {
                product_id: Uuid::parse_str(&i.product_id).unwrap_or_default(),
                quantity: i.quantity,
            })
            .collect();
        let cmd = services::orders::create_order_command::CreateOrderCommand {
            customer_id,
            items,
        };
        let id = self
            .svc
            .create_order(cmd)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(proto::CreateOrderResponse {
            order_id: id.to_string(),
            status: "PENDING".to_string(),
        }))
    }

    async fn get_order(
        &self,
        request: Request<proto::GetOrderRequest>,
    ) -> Result<Response<proto::GetOrderResponse>, Status> {
        let id = Uuid::parse_str(&request.into_inner().order_id)
            .map_err(|_| Status::invalid_argument("invalid order_id"))?;
        let maybe = self
            .svc
            .get_order(&id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        if let Some(order) = maybe {
            let resp = proto::Order {
                id: order.id.to_string(),
                customer_id: order.customer_id.to_string(),
                items: vec![],
                total_amount: None,
                status: order.status.to_string(),
                created_at: None,
                shipping_address: None,
                billing_address: None,
                payment_method_id: String::new(),
                shipment_id: String::new(),
            };
            Ok(Response::new(proto::GetOrderResponse { order: Some(resp) }))
        } else {
            Err(Status::not_found("order not found"))
        }
    }

    async fn update_order_status(
        &self,
        request: Request<proto::UpdateOrderStatusRequest>,
    ) -> Result<Response<proto::UpdateOrderStatusResponse>, Status> {
        let req = request.into_inner();
        let order_id = Uuid::parse_str(&req.order_id)
            .map_err(|_| Status::invalid_argument("invalid order_id"))?;
        let cmd = services::orders::update_order_status_command::UpdateOrderStatusCommand {
            order_id,
            new_status: req.new_status.clone(),
        };
        self
            .svc
            .update_order_status(cmd)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(proto::UpdateOrderStatusResponse {
            order_id: req.order_id,
            status: req.new_status,
        }))
    }

    async fn list_orders(
        &self,
        _request: Request<proto::ListOrdersRequest>,
    ) -> Result<Response<proto::ListOrdersResponse>, Status> {
        Ok(Response::new(proto::ListOrdersResponse {
            orders: vec![],
            pagination: None,
        }))
    }
}

pub struct WarrantyGrpcService {
    pub svc: services::warranties::WarrantyService,
}

#[tonic::async_trait]
impl WarrantyService for WarrantyGrpcService {
    async fn create_warranty(
        &self,
        _request: Request<proto::CreateWarrantyRequest>,
    ) -> Result<Response<proto::CreateWarrantyResponse>, Status> {
        Err(Status::unimplemented("create_warranty not implemented"))
    }

    async fn get_warranty(
        &self,
        _request: Request<proto::GetWarrantyRequest>,
    ) -> Result<Response<proto::GetWarrantyResponse>, Status> {
        Err(Status::unimplemented("get_warranty not implemented"))
    }

    async fn update_warranty(
        &self,
        _request: Request<proto::UpdateWarrantyRequest>,
    ) -> Result<Response<proto::UpdateWarrantyResponse>, Status> {
        Err(Status::unimplemented("update_warranty not implemented"))
    }

    async fn list_warranties(
        &self,
        _request: Request<proto::ListWarrantiesRequest>,
    ) -> Result<Response<proto::ListWarrantiesResponse>, Status> {
        Ok(Response::new(proto::ListWarrantiesResponse {
            warranties: vec![],
            pagination: None,
        }))
    }
}

pub struct InventoryGrpcService {
    pub svc: services::inventory::InventoryService,
}

#[tonic::async_trait]
impl InventoryService for InventoryGrpcService {
    async fn update_inventory(
        &self,
        _request: Request<proto::UpdateInventoryRequest>,
    ) -> Result<Response<proto::UpdateInventoryResponse>, Status> {
        Err(Status::unimplemented("update_inventory not implemented"))
    }

    async fn get_inventory(
        &self,
        _request: Request<proto::GetInventoryRequest>,
    ) -> Result<Response<proto::GetInventoryResponse>, Status> {
        Err(Status::unimplemented("get_inventory not implemented"))
    }

    async fn list_inventory(
        &self,
        _request: Request<proto::ListInventoryRequest>,
    ) -> Result<Response<proto::ListInventoryResponse>, Status> {
        Ok(Response::new(proto::ListInventoryResponse {
            items: vec![],
            pagination: None,
        }))
    }

    async fn reserve_inventory(
        &self,
        _request: Request<proto::ReserveInventoryRequest>,
    ) -> Result<Response<proto::ReserveInventoryResponse>, Status> {
        Err(Status::unimplemented("reserve_inventory not implemented"))
    }
}

pub struct ReturnGrpcService {
    pub svc: services::returns::ReturnService,
}

#[tonic::async_trait]
impl ReturnService for ReturnGrpcService {
    async fn create_return(
        &self,
        _request: Request<proto::CreateReturnRequest>,
    ) -> Result<Response<proto::CreateReturnResponse>, Status> {
        Err(Status::unimplemented("create_return not implemented"))
    }

    async fn get_return(
        &self,
        _request: Request<proto::GetReturnRequest>,
    ) -> Result<Response<proto::GetReturnResponse>, Status> {
        Err(Status::unimplemented("get_return not implemented"))
    }

    async fn update_return_status(
        &self,
        _request: Request<proto::UpdateReturnStatusRequest>,
    ) -> Result<Response<proto::UpdateReturnStatusResponse>, Status> {
        Err(Status::unimplemented("update_return_status not implemented"))
    }

    async fn list_returns(
        &self,
        _request: Request<proto::ListReturnsRequest>,
    ) -> Result<Response<proto::ListReturnsResponse>, Status> {
        Ok(Response::new(proto::ListReturnsResponse {
            returns: vec![],
            pagination: None,
        }))
    }
}

pub struct ShipmentGrpcService {
    pub svc: services::shipments::ShipmentService,
}

#[tonic::async_trait]
impl ShipmentService for ShipmentGrpcService {
    async fn create_shipment(
        &self,
        _request: Request<proto::CreateShipmentRequest>,
    ) -> Result<Response<proto::CreateShipmentResponse>, Status> {
        Err(Status::unimplemented("create_shipment not implemented"))
    }

    async fn get_shipment(
        &self,
        _request: Request<proto::GetShipmentRequest>,
    ) -> Result<Response<proto::GetShipmentResponse>, Status> {
        Err(Status::unimplemented("get_shipment not implemented"))
    }

    async fn update_shipment_status(
        &self,
        _request: Request<proto::UpdateShipmentStatusRequest>,
    ) -> Result<Response<proto::UpdateShipmentStatusResponse>, Status> {
        Err(Status::unimplemented("update_shipment_status not implemented"))
    }

    async fn list_shipments(
        &self,
        _request: Request<proto::ListShipmentsRequest>,
    ) -> Result<Response<proto::ListShipmentsResponse>, Status> {
        Ok(Response::new(proto::ListShipmentsResponse {
            shipments: vec![],
            pagination: None,
        }))
    }
}

pub async fn serve(app_services: AppServices, addr: SocketAddr) -> Result<(), Box<dyn std::error::Error>> {
    let order_svc = OrderGrpcService { svc: app_services.orders };
    let warranty_svc = WarrantyGrpcService { svc: app_services.warranties };
    let inventory_svc = InventoryGrpcService { svc: app_services.inventory };
    let return_svc = ReturnGrpcService { svc: app_services.returns };
    let shipment_svc = ShipmentGrpcService { svc: app_services.shipments };

    Server::builder()
        .add_service(OrderServiceServer::new(order_svc))
        .add_service(WarrantyServiceServer::new(warranty_svc))
        .add_service(InventoryServiceServer::new(inventory_svc))
        .add_service(ReturnServiceServer::new(return_svc))
        .add_service(ShipmentServiceServer::new(shipment_svc))
        .serve(addr)
        .await?;
    Ok(())
}

