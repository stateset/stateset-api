use crate::proto::{
    self,
    // Service imports
    inventory::inventory_service_server::{InventoryService, InventoryServiceServer},
    order::order_service_server::{OrderService, OrderServiceServer},
    return_order::return_service_server::{ReturnService, ReturnServiceServer},
    shipment::shipment_service_server::{ShipmentService, ShipmentServiceServer},
    warranty::warranty_service_server::{WarrantyService, WarrantyServiceServer},
    // Order types
    order::{
        CreateOrderRequest, CreateOrderResponse,
        GetOrderRequest, GetOrderResponse, Order,
        UpdateOrderStatusRequest, UpdateOrderStatusResponse,
        ListOrdersRequest, ListOrdersResponse,
    },
    // Warranty types
    warranty::{
        CreateWarrantyRequest, CreateWarrantyResponse,
        GetWarrantyRequest, GetWarrantyResponse, Warranty,
        UpdateWarrantyRequest, UpdateWarrantyResponse,
        ListWarrantiesRequest, ListWarrantiesResponse,
    },
    // Inventory types
    inventory::{
        UpdateInventoryRequest, UpdateInventoryResponse,
        GetInventoryRequest, GetInventoryResponse, InventoryItem,
        ListInventoryRequest, ListInventoryResponse,
        ReserveInventoryRequest, ReserveInventoryResponse,
    },
    // Return types
    return_order::{
        CreateReturnRequest, CreateReturnResponse,
        GetReturnRequest, GetReturnResponse, Return,
        UpdateReturnStatusRequest, UpdateReturnStatusResponse,
        ListReturnsRequest, ListReturnsResponse,
        ReturnStatus,
    },
    // Shipment types
    shipment::{
        CreateShipmentRequest, CreateShipmentResponse,
        GetShipmentRequest, GetShipmentResponse, Shipment,
        UpdateShipmentStatusRequest, UpdateShipmentStatusResponse,
        ListShipmentsRequest, ListShipmentsResponse,
    },
};
use crate::services;
use crate::handlers::AppServices;
use chrono::{DateTime, TimeZone, Utc};
use std::net::SocketAddr;
use tonic::{transport::Server, Request, Response, Status};
use uuid::Uuid;

pub struct OrderGrpcService {
    pub svc: services::orders::OrderService,
}

#[tonic::async_trait]
impl OrderService for OrderGrpcService {
    async fn create_order(
        &self,
        request: Request<CreateOrderRequest>,
    ) -> Result<Response<CreateOrderResponse>, Status> {
        let order = request
            .into_inner()
            .order
            .ok_or_else(|| Status::invalid_argument("order missing"))?;
        let customer_id = Uuid::parse_str(&order.customer_id)
            .map_err(|_| Status::invalid_argument("invalid customer_id"))?;
        let items = order
            .items
            .into_iter()
            .map(|i| crate::commands::orders::create_order_command::CreateOrderItem {
                product_id: Uuid::parse_str(&i.product_id).unwrap_or_default(),
                quantity: i.quantity,
            })
            .collect();
        let cmd = crate::commands::orders::create_order_command::CreateOrderCommand { customer_id, items };
        let id = self
            .svc
            .create_order(cmd)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        
        // Convert timestamp to prost_types::Timestamp
        let now = chrono::Utc::now();
        let timestamp = prost_types::Timestamp {
            seconds: now.and_utc().timestamp(),
            nanos: now.and_utc().timestamp_subsec_nanos() as i32,
        };
        
        Ok(Response::new(CreateOrderResponse {
            order_id: id.to_string(),
            status: 1, // OrderStatus::PENDING
            created_at: Some(timestamp),
        }))
    }

    async fn get_order(
        &self,
        request: Request<GetOrderRequest>,
    ) -> Result<Response<GetOrderResponse>, Status> {
        let id = Uuid::parse_str(&request.into_inner().order_id)
            .map_err(|_| Status::invalid_argument("invalid order_id"))?;
        let maybe = self
            .svc
            .get_order(&id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        if let Some(order) = maybe {
            let resp = Order {
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
            Ok(Response::new(GetOrderResponse { order: Some(resp) }))
        } else {
            Err(Status::not_found("order not found"))
        }
    }

    async fn update_order_status(
        &self,
        request: Request<UpdateOrderStatusRequest>,
    ) -> Result<Response<UpdateOrderStatusResponse>, Status> {
        let req = request.into_inner();
        let order_id = Uuid::parse_str(&req.order_id)
            .map_err(|_| Status::invalid_argument("invalid order_id"))?;
        
        // Parse the string status to OrderStatus
        let new_status = match req.new_status {
            0 => crate::models::order::OrderStatus::Pending,
            1 => crate::models::order::OrderStatus::Processing,
            2 => crate::models::order::OrderStatus::Shipped,
            3 => crate::models::order::OrderStatus::Delivered,
            4 => crate::models::order::OrderStatus::Cancelled,
            _ => return Err(Status::invalid_argument("Invalid status value")),
        };
        
        let cmd = crate::commands::orders::update_order_status_command::UpdateOrderStatusCommand {
            order_id,
            new_status,
        };
        self.svc
            .update_order_status(cmd)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(UpdateOrderStatusResponse {
            order_id: req.order_id,
            status: req.new_status,
        }))
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

pub struct WarrantyGrpcService {
    pub svc: services::warranties::WarrantyService,
}

#[tonic::async_trait]
impl WarrantyService for WarrantyGrpcService {
    async fn create_warranty(
        &self,
        request: Request<CreateWarrantyRequest>,
    ) -> Result<Response<CreateWarrantyResponse>, Status> {
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
        let expiration_date =
            chrono::DateTime::from_timestamp(expiration_date.seconds, expiration_date.nanos as u32)
                .ok_or_else(|| Status::invalid_argument("invalid end_date"))?;

        let cmd = crate::commands::warranties::create_warranty_command::CreateWarrantyCommand {
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

        Ok(Response::new(CreateWarrantyResponse {
            warranty_id: id.to_string(),
        }))
    }

    async fn get_warranty(
        &self,
        request: Request<GetWarrantyRequest>,
    ) -> Result<Response<GetWarrantyResponse>, Status> {
        let id = Uuid::parse_str(&request.into_inner().warranty_id)
            .map_err(|_| Status::invalid_argument("invalid warranty_id"))?;

        let maybe = self
            .svc
            .get_warranty(&id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        if let Some(w) = maybe {
            let resp = Warranty {
                id: w.id.to_string(),
                order_id: w.order_id.to_string(),
                product_id: String::new(), // Field doesn't exist, using empty string
                customer_id: w.customer_id.to_string(),
                start_date: None,
                end_date: None,
                status: w.status.as_str().to_string(), // Use as_str() method
                terms: String::new(), // Field doesn't exist, using empty string
            };
            Ok(Response::new(GetWarrantyResponse {
                warranty: Some(resp),
            }))
        } else {
            Err(Status::not_found("warranty not found"))
        }
    }

    async fn update_warranty(
        &self,
        request: Request<UpdateWarrantyRequest>,
    ) -> Result<Response<UpdateWarrantyResponse>, Status> {
        // For now this simply returns the provided warranty object
        let warranty = request
            .into_inner()
            .warranty
            .ok_or_else(|| Status::invalid_argument("warranty missing"))?;

        Ok(Response::new(UpdateWarrantyResponse {
            warranty: Some(warranty),
        }))
    }

    async fn list_warranties(
        &self,
        _request: Request<ListWarrantiesRequest>,
    ) -> Result<Response<ListWarrantiesResponse>, Status> {
        Ok(Response::new(ListWarrantiesResponse {
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
        request: Request<UpdateInventoryRequest>,
    ) -> Result<Response<UpdateInventoryResponse>, Status> {
        let req = request.into_inner();
        let product_id = Uuid::parse_str(&req.product_id)
            .map_err(|_| Status::invalid_argument("invalid product_id"))?;
        
        // Fix Option::parse_str usage
        let warehouse_id = if req.warehouse_id.is_empty() {
            None
        } else {
            Some(req.warehouse_id.clone())
        };

        let cmd = crate::commands::inventory::adjust_inventory_command::AdjustInventoryCommand {
            product_id,
            location_id: warehouse_id,
            adjustment_quantity: req.quantity_change,
            reason_code: req.reason,
            notes: None,
            lot_number: None,
            reference_number: None,
            version: 0,
        };

        self.svc
            .adjust_inventory(cmd)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(UpdateInventoryResponse {
            product_id: req.product_id,
            new_quantity: 0,
            warehouse_id: req.warehouse_id,
        }))
    }

    async fn get_inventory(
        &self,
        request: Request<GetInventoryRequest>,
    ) -> Result<Response<GetInventoryResponse>, Status> {
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
            let resp = InventoryItem {
                product_id: product_id.to_string(), // Use the parameter instead
                quantity: i.available, // Use available field as quantity
                warehouse_id: warehouse_id.to_string(), // Use the parameter instead
                location: String::new(),
                last_updated: None, // No last_updated field, use None
            };
            Ok(Response::new(GetInventoryResponse {
                item: Some(resp),
            }))
        } else {
            Err(Status::not_found("inventory not found"))
        }
    }

    async fn list_inventory(
        &self,
        _request: Request<ListInventoryRequest>,
    ) -> Result<Response<ListInventoryResponse>, Status> {
        Ok(Response::new(ListInventoryResponse {
            items: vec![],
            pagination: None,
        }))
    }

    async fn reserve_inventory(
        &self,
        request: Request<ReserveInventoryRequest>,
    ) -> Result<Response<ReserveInventoryResponse>, Status> {
        let req = request.into_inner();
        let product_id = Uuid::parse_str(&req.product_id)
            .map_err(|_| Status::invalid_argument("invalid product_id"))?;
        let cmd = crate::commands::inventory::reserve_inventory_command::ReserveInventoryCommand {
            warehouse_id: String::new(),
            reference_id: Uuid::parse_str(&req.order_id).unwrap_or_default(),
            reference_type: "ORDER".to_string(),
            items: vec![
                crate::commands::inventory::reserve_inventory_command::ReservationRequest {
                    product_id,
                    quantity: req.quantity,
                    lot_numbers: None,
                    location_id: None,
                    substitutes: None,
                },
            ],
            reservation_type:
                crate::commands::inventory::reserve_inventory_command::ReservationType::SalesOrder,
            duration_days: None,
            priority: None,
            notes: None,
            reservation_strategy:
                crate::commands::inventory::reserve_inventory_command::ReservationStrategy::Strict,
        };

        // The return type of reserve_inventory is Result<(), ServiceError>
        // So we can't access fields of the result
        self.svc
            .reserve_inventory(cmd)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        // Since we can't access the result fields, we'll just return a generic response
        Ok(Response::new(ReserveInventoryResponse {
            success: true,
            reservation_id: req.order_id.clone(),
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
        request: Request<CreateReturnRequest>,
    ) -> Result<Response<CreateReturnResponse>, Status> {
        let ret = request
            .into_inner()
            .r#return
            .ok_or_else(|| Status::invalid_argument("return missing"))?;

        let order_id = Uuid::parse_str(&ret.order_id)
            .map_err(|_| Status::invalid_argument("invalid order_id"))?;
        let cmd = crate::commands::returns::create_return_command::InitiateReturnCommand {
            order_id,
            reason: ret.reason.clone(),
        };

        let id = self
            .svc
            .create_return(cmd)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(CreateReturnResponse {
            return_id: id.to_string(),
            status: ReturnStatus::Requested as i32,
        }))
    }

    async fn get_return(
        &self,
        request: Request<GetReturnRequest>,
    ) -> Result<Response<GetReturnResponse>, Status> {
        let id = Uuid::parse_str(&request.into_inner().return_id)
            .map_err(|_| Status::invalid_argument("invalid return_id"))?;
        let maybe = self
            .svc
            .get_return(&id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        if let Some(ret) = maybe {
            let resp = Return {
                id: ret.id.to_string(),
                order_id: ret.order_id.to_string(),
                customer_id: String::new(),
                items: vec![],
                status: ReturnStatus::Requested as i32,
                reason: ret.reason,
                created_at: None,
                updated_at: None,
            };
            Ok(Response::new(GetReturnResponse {
                r#return: Some(resp),
            }))
        } else {
            Err(Status::not_found("return not found"))
        }
    }

    async fn update_return_status(
        &self,
        request: Request<UpdateReturnStatusRequest>,
    ) -> Result<Response<UpdateReturnStatusResponse>, Status> {
        let req = request.into_inner();
        let return_id = Uuid::parse_str(&req.return_id)
            .map_err(|_| Status::invalid_argument("invalid return_id"))?;
        // Map new_status to command
        match ReturnStatus::try_from(req.new_status) {
            Ok(ReturnStatus::Approved) => {
                let cmd =
                    crate::commands::returns::approve_return_command::ApproveReturnCommand { return_id };
                self.svc
                    .approve_return(cmd)
                    .await
                    .map_err(|e| Status::internal(e.to_string()))?;
            }
            Ok(ReturnStatus::Rejected) => {
                let cmd =
                    crate::commands::returns::reject_return_command::RejectReturnCommand { return_id }; // reason is missing
                self.svc
                    .reject_return(cmd)
                    .await
                    .map_err(|e| Status::internal(e.to_string()))?;
            }
            Ok(ReturnStatus::Received) => {
                let cmd =
                    crate::commands::returns::complete_return_command::CompleteReturnCommand { return_id }; //  missing `completed_by`, `metadata` and `notes`
                self.svc
                    .complete_return(cmd)
                    .await
                    .map_err(|e| Status::internal(e.to_string()))?;
            }
            Err(_) => {
                return Err(Status::invalid_argument("Invalid return status"));
            }
            _ => {
                return Err(Status::invalid_argument("Unsupported return status"));
            }
        }

        Ok(Response::new(UpdateReturnStatusResponse {
            return_id: req.return_id,
            status: req.new_status,
        }))
    }

    async fn list_returns(
        &self,
        _request: Request<ListReturnsRequest>,
    ) -> Result<Response<ListReturnsResponse>, Status> {
        Ok(Response::new(ListReturnsResponse {
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
        request: Request<CreateShipmentRequest>,
    ) -> Result<Response<CreateShipmentResponse>, Status> {
        let shipment = request
            .into_inner()
            .shipment
            .ok_or_else(|| Status::invalid_argument("shipment missing"))?;

        let order_id = Uuid::parse_str(&shipment.order_id)
            .map_err(|_| Status::invalid_argument("invalid order_id"))?;
        let cmd = crate::commands::shipments::create_shipment_command::CreateShipmentCommand {
            order_id,
            recipient_name: "Recipient".to_string(),
            shipping_address: shipment
                .shipping_address
                .as_ref()
                .map(|a| a.street_line1.clone())
                .unwrap_or_default(),
            carrier: if shipment.carrier.is_empty() {
                None
            } else {
                Some(shipment.carrier.clone())
            },
            tracking_number: if shipment.tracking_number.is_empty() {
                None
            } else {
                Some(shipment.tracking_number.clone())
            },
        };

        let id = self
            .svc
            .create_shipment(cmd)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(CreateShipmentResponse {
            shipment_id: id.to_string(),
        }))
    }

    async fn get_shipment(
        &self,
        request: Request<GetShipmentRequest>,
    ) -> Result<Response<GetShipmentResponse>, Status> {
        let id = Uuid::parse_str(&request.into_inner().shipment_id)
            .map_err(|_| Status::invalid_argument("invalid shipment_id"))?;

        let maybe = self
            .svc
            .get_shipment(&id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        if let Some(sh) = maybe {
            let resp = Shipment {
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
            Ok(Response::new(GetShipmentResponse {
                shipment: Some(resp),
            }))
        } else {
            Err(Status::not_found("shipment not found"))
        }
    }

    async fn update_shipment_status(
        &self,
        request: Request<UpdateShipmentStatusRequest>,
    ) -> Result<Response<UpdateShipmentStatusResponse>, Status> {
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
        let cmd = crate::commands::shipments::update_shipment_command::UpdateShipmentStatusCommand {
            shipment_id,
            new_status: status,
        };

        self.svc
            .update_shipment(cmd)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(UpdateShipmentStatusResponse {
            shipment_id: req.shipment_id,
            status: req.new_status,
        }))
    }

    async fn list_shipments(
        &self,
        _request: Request<ListShipmentsRequest>,
    ) -> Result<Response<ListShipmentsResponse>, Status> {
        Ok(Response::new(ListShipmentsResponse {
            shipments: vec![],
            pagination: None,
        }))
    }
}

pub async fn serve(
    app_services: AppServices,
    addr: SocketAddr,
) -> Result<(), Box<dyn std::error::Error>> {
    let order_svc = OrderGrpcService {
        svc: app_services.orders,
    };
    let warranty_svc = WarrantyGrpcService {
        svc: app_services.warranties,
    };
    let inventory_svc = InventoryGrpcService {
        svc: app_services.inventory,
    };
    let return_svc = ReturnGrpcService {
        svc: app_services.returns,
    };
    let shipment_svc = ShipmentGrpcService {
        svc: app_services.shipments,
    };

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
