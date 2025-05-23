use std::net::SocketAddr;
use tonic::{transport::Server, Request, Response, Status};
use uuid::Uuid;
use chrono::{DateTime, Utc, TimeZone};
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
        request: Request<proto::CreateWarrantyRequest>,
    ) -> Result<Response<proto::CreateWarrantyResponse>, Status> {
        let req = request.into_inner();
        let warranty = req
            .warranty
            .ok_or_else(|| Status::invalid_argument("warranty missing"))?;

        let product_id = Uuid::parse_str(&warranty.product_id)
            .map_err(|_| Status::invalid_argument("invalid product_id"))?;
        let customer_id = Uuid::parse_str(&warranty.customer_id)
            .map_err(|_| Status::invalid_argument("invalid customer_id"))?;

        let expiration_date = warranty
            .end_date
            .ok_or_else(|| Status::invalid_argument("end_date missing"))?;
        let expiration_date = chrono::DateTime::from_timestamp(
            expiration_date.seconds,
            expiration_date.nanos as u32,
        )
        .ok_or_else(|| Status::invalid_argument("invalid end_date"))?;

        let cmd = services::warranties::create_warranty_command::CreateWarrantyCommand {
            product_id,
            customer_id,
            serial_number: String::new(),
            warranty_type: "standard".to_string(),
            expiration_date,
            terms: warranty.terms.clone(),
        };

        let id = self
            .svc
            .create_warranty(cmd)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(proto::CreateWarrantyResponse {
            warranty_id: id.to_string(),
        }))
    }

    async fn get_warranty(
        &self,
        request: Request<proto::GetWarrantyRequest>,
    ) -> Result<Response<proto::GetWarrantyResponse>, Status> {
        let id = Uuid::parse_str(&request.into_inner().warranty_id)
            .map_err(|_| Status::invalid_argument("invalid warranty_id"))?;

        let maybe = self
            .svc
            .get_warranty(&id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        if let Some(w) = maybe {
            let resp = proto::Warranty {
                id: w.id.to_string(),
                order_id: w.order_id.unwrap_or_default().to_string(),
                product_id: w.product_id.to_string(),
                customer_id: w.customer_id.to_string(),
                start_date: None,
                end_date: None,
                status: w.status,
                terms: w.terms.unwrap_or_default(),
            };
            Ok(Response::new(proto::GetWarrantyResponse { warranty: Some(resp) }))
        } else {
            Err(Status::not_found("warranty not found"))
        }
    }

    async fn update_warranty(
        &self,
        request: Request<proto::UpdateWarrantyRequest>,
    ) -> Result<Response<proto::UpdateWarrantyResponse>, Status> {
        // For now this simply returns the provided warranty object
        let warranty = request
            .into_inner()
            .warranty
            .ok_or_else(|| Status::invalid_argument("warranty missing"))?;

        Ok(Response::new(proto::UpdateWarrantyResponse { warranty: Some(warranty) }))
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
        request: Request<proto::UpdateInventoryRequest>,
    ) -> Result<Response<proto::UpdateInventoryResponse>, Status> {
        let req = request.into_inner();
        let product_id = Uuid::parse_str(&req.product_id)
            .map_err(|_| Status::invalid_argument("invalid product_id"))?;
        let warehouse_id = Uuid::parse_str(&req.warehouse_id)
            .map_err(|_| Status::invalid_argument("invalid warehouse_id"))?;

        let cmd = crate::commands::inventory::adjust_inventory_command::AdjustInventoryCommand {
            product_id,
            location_id: warehouse_id,
            adjustment: req.quantity_change,
            reason: req.reason,
        };

        self
            .svc
            .adjust_inventory(cmd)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(proto::UpdateInventoryResponse {
            product_id: req.product_id,
            new_quantity: 0,
            warehouse_id: req.warehouse_id,
        }))
    }

    async fn get_inventory(
        &self,
        request: Request<proto::GetInventoryRequest>,
    ) -> Result<Response<proto::GetInventoryResponse>, Status> {
        let req = request.into_inner();
        let product_id = Uuid::parse_str(&req.product_id)
            .map_err(|_| Status::invalid_argument("invalid product_id"))?;
        let warehouse_id = Uuid::parse_str(&req.warehouse_id)
            .map_err(|_| Status::invalid_argument("invalid warehouse_id"))?;

        let item = self
            .svc
            .get_inventory(&product_id, &warehouse_id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        if let Some(i) = item {
            let resp = proto::InventoryItem {
                product_id: i.product_id.to_string(),
                quantity: i.quantity,
                warehouse_id: i.location_id.to_string(),
                location: String::new(),
                last_updated: None,
            };
            Ok(Response::new(proto::GetInventoryResponse { item: Some(resp) }))
        } else {
            Err(Status::not_found("inventory not found"))
        }
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
        request: Request<proto::ReserveInventoryRequest>,
    ) -> Result<Response<proto::ReserveInventoryResponse>, Status> {
        let req = request.into_inner();
        let product_id = Uuid::parse_str(&req.product_id)
            .map_err(|_| Status::invalid_argument("invalid product_id"))?;
        let cmd = crate::commands::inventory::reserve_inventory_command::ReserveInventoryCommand {
            warehouse_id: String::new(),
            reference_id: Uuid::parse_str(&req.order_id).unwrap_or_default(),
            reference_type: "ORDER".to_string(),
            items: vec![crate::commands::inventory::reserve_inventory_command::ReservationRequest {
                product_id,
                quantity: req.quantity,
                lot_numbers: None,
                location_id: None,
                substitutes: None,
            }],
            reservation_type: crate::commands::inventory::reserve_inventory_command::ReservationType::SalesOrder,
            duration_days: None,
            priority: None,
            notes: None,
            reservation_strategy: crate::commands::inventory::reserve_inventory_command::ReservationStrategy::Strict,
        };

        let result = self
            .svc
            .reserve_inventory(cmd)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(proto::ReserveInventoryResponse {
            success: result.fully_reserved,
            reservation_id: result.reference_id.to_string(),
        }))
    }
}

pub struct ReturnGrpcService {
    pub svc: services::returns::ReturnService,
}

#[tonic::async_trait]
impl ReturnService for ReturnGrpcService {
    async fn create_return(
        &self,
        request: Request<proto::CreateReturnRequest>,
    ) -> Result<Response<proto::CreateReturnResponse>, Status> {
        let ret = request
            .into_inner()
            .return_
            .ok_or_else(|| Status::invalid_argument("return missing"))?;

        let order_id = Uuid::parse_str(&ret.order_id)
            .map_err(|_| Status::invalid_argument("invalid order_id"))?;
        let cmd = services::returns::create_return_command::InitiateReturnCommand {
            order_id,
            reason: ret.reason.clone(),
        };

        let id = self
            .svc
            .create_return(cmd)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(proto::CreateReturnResponse {
            return_id: id.to_string(),
            status: proto::return_order::ReturnStatus::Requested as i32,
        }))
    }

    async fn get_return(
        &self,
        request: Request<proto::GetReturnRequest>,
    ) -> Result<Response<proto::GetReturnResponse>, Status> {
        let id = Uuid::parse_str(&request.into_inner().return_id)
            .map_err(|_| Status::invalid_argument("invalid return_id"))?;
        let maybe = self
            .svc
            .get_return(&id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        if let Some(ret) = maybe {
            let status_enum = proto::return_order::ReturnStatus::from_str_name(&ret.status)
                .unwrap_or(proto::return_order::ReturnStatus::Unknown);
            let resp = proto::Return {
                id: ret.id.to_string(),
                order_id: ret.order_id.to_string(),
                customer_id: String::new(),
                items: vec![],
                status: status_enum as i32,
                reason: ret.reason,
                created_at: None,
                updated_at: None,
            };
            Ok(Response::new(proto::GetReturnResponse { return_: Some(resp) }))
        } else {
            Err(Status::not_found("return not found"))
        }
    }

    async fn update_return_status(
        &self,
        request: Request<proto::UpdateReturnStatusRequest>,
    ) -> Result<Response<proto::UpdateReturnStatusResponse>, Status> {
        let req = request.into_inner();
        let return_id = Uuid::parse_str(&req.return_id)
            .map_err(|_| Status::invalid_argument("invalid return_id"))?;
        // Map new_status to command
        match proto::return_order::ReturnStatus::from_i32(req.new_status) {
            Some(proto::return_order::ReturnStatus::Approved) => {
                let cmd = services::returns::approve_return_command::ApproveReturnCommand { return_id };
                self.svc.approve_return(cmd)
                    .await
                    .map_err(|e| Status::internal(e.to_string()))?;
            }
            Some(proto::return_order::ReturnStatus::Rejected) => {
                let cmd = services::returns::reject_return_command::RejectReturnCommand { return_id };
                self.svc.reject_return(cmd)
                    .await
                    .map_err(|e| Status::internal(e.to_string()))?;
            }
            Some(proto::return_order::ReturnStatus::Received) => {
                let cmd = services::returns::complete_return_command::CompleteReturnCommand { return_id };
                self.svc.complete_return(cmd)
                    .await
                    .map_err(|e| Status::internal(e.to_string()))?;
            }
            _ => {}
        }

        Ok(Response::new(proto::UpdateReturnStatusResponse {
            return_id: req.return_id,
            status: req.new_status,
        }))
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
        request: Request<proto::CreateShipmentRequest>,
    ) -> Result<Response<proto::CreateShipmentResponse>, Status> {
        let shipment = request
            .into_inner()
            .shipment
            .ok_or_else(|| Status::invalid_argument("shipment missing"))?;

        let order_id = Uuid::parse_str(&shipment.order_id)
            .map_err(|_| Status::invalid_argument("invalid order_id"))?;
        let cmd = services::shipments::create_shipment_command::CreateShipmentCommand {
            order_id,
            recipient_name: "Recipient".to_string(),
            shipping_address: shipment
                .shipping_address
                .as_ref()
                .map(|a| a.street_line1.clone())
                .unwrap_or_default(),
            carrier: if shipment.carrier.is_empty() { None } else { Some(shipment.carrier.clone()) },
            tracking_number: if shipment.tracking_number.is_empty() { None } else { Some(shipment.tracking_number.clone()) },
        };

        let id = self
            .svc
            .create_shipment(cmd)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(proto::CreateShipmentResponse {
            shipment_id: id.to_string(),
        }))
    }

    async fn get_shipment(
        &self,
        request: Request<proto::GetShipmentRequest>,
    ) -> Result<Response<proto::GetShipmentResponse>, Status> {
        let id = Uuid::parse_str(&request.into_inner().shipment_id)
            .map_err(|_| Status::invalid_argument("invalid shipment_id"))?;

        let maybe = self
            .svc
            .get_shipment(&id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        if let Some(sh) = maybe {
            let resp = proto::Shipment {
                id: sh.id.to_string(),
                order_id: sh.order_id.to_string(),
                carrier: sh.carrier.to_string(),
                tracking_number: sh.tracking_number,
                shipping_address: None,
                status: sh.status.to_string(),
                created_at: None,
                updated_at: None,
                items: vec![],
            };
            Ok(Response::new(proto::GetShipmentResponse { shipment: Some(resp) }))
        } else {
            Err(Status::not_found("shipment not found"))
        }
    }

    async fn update_shipment_status(
        &self,
        request: Request<proto::UpdateShipmentStatusRequest>,
    ) -> Result<Response<proto::UpdateShipmentStatusResponse>, Status> {
        let req = request.into_inner();
        let shipment_id: i32 = req
            .shipment_id
            .parse()
            .map_err(|_| Status::invalid_argument("invalid shipment_id"))?;

        let status = match req.new_status.as_str() {
            "ReadyToShip" => crate::models::shipment::ShipmentStatus::ReadyToShip,
            "Shipped" => crate::models::shipment::ShipmentStatus::Shipped,
            "InTransit" => crate::models::shipment::ShipmentStatus::InTransit,
            "OutForDelivery" => crate::models::shipment::ShipmentStatus::OutForDelivery,
            "Delivered" => crate::models::shipment::ShipmentStatus::Delivered,
            "Failed" => crate::models::shipment::ShipmentStatus::Failed,
            "Returned" => crate::models::shipment::ShipmentStatus::Returned,
            "Cancelled" => crate::models::shipment::ShipmentStatus::Cancelled,
            _ => crate::models::shipment::ShipmentStatus::Processing,
        };
        let cmd = services::shipments::update_shipment_command::UpdateShipmentStatusCommand {
            shipment_id,
            new_status: status,
        };

        self
            .svc
            .update_shipment(cmd)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(proto::UpdateShipmentStatusResponse {
            shipment_id: req.shipment_id,
            status: req.new_status,
        }))
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

