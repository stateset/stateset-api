use std::sync::Arc;

use rust_decimal::{prelude::ToPrimitive, Decimal};
use tonic::{Request, Response, Status};
use tracing::{error, info, instrument};
use uuid::Uuid;

use crate::{
    db::{DatabaseAccess, DbPool},
    proto::{
        order::*,
        inventory::*,
        return_order::*,
        warranty::*,
        shipment::*,
        work_order::*,
    },
    services::order_status::OrderStatusService,
    services::orders::{OrderService as OrdersService, CreateOrderRequest as ServiceCreateOrderRequest},
    services::inventory::{InventoryService as InvService, AdjustInventoryCommand},
    events::EventSender,
};

#[derive(Clone)]
pub struct StateSetApi {
    pub db: Arc<DatabaseAccess>,
    pub db_pool: Arc<DbPool>,
    event_sender: EventSender,
    inventory_service: Arc<InvService>,
    order_service: Arc<OrdersService>,
    order_status_service: Arc<OrderStatusService>,
}

impl StateSetApi {
    pub fn new(db: Arc<DatabaseAccess>, db_pool: Arc<DbPool>) -> Self {
        let event_sender = EventSender::new(tokio::sync::mpsc::channel(1000).0);
        let inventory_service = Arc::new(InvService::new(db_pool.clone(), event_sender.clone()));
        let order_service = Arc::new(OrdersService::new(db_pool.clone(), Some(Arc::new(event_sender.clone()))));
        let order_status_service = Arc::new(OrderStatusService::new(db_pool.clone()));
        
        Self {
            db,
            db_pool,
            event_sender,
            inventory_service,
            order_service,
            order_status_service,
        }
    }
    
    pub fn with_event_sender(db: Arc<DatabaseAccess>, db_pool: Arc<DbPool>, event_sender: EventSender) -> Self {
        let inventory_service = Arc::new(InvService::new(db_pool.clone(), event_sender.clone()));
        let order_service = Arc::new(OrdersService::new(db_pool.clone(), Some(Arc::new(event_sender.clone()))));
        let order_status_service = Arc::new(OrderStatusService::new(db_pool.clone()));
        
        Self {
            db,
            db_pool,
            event_sender,
            inventory_service,
            order_service,
            order_status_service,
        }
    }
}

// Status mapping helpers
mod status_helpers {
    use super::*;
    
    pub fn map_status_str_to_proto(status: &str) -> OrderStatus {
        match status.to_ascii_lowercase().as_str() {
            "pending" => OrderStatus::Pending,
            "processing" => OrderStatus::Processing,
            "shipped" => OrderStatus::Shipped,
            "delivered" => OrderStatus::Delivered,
            "cancelled" | "canceled" => OrderStatus::Canceled,
            "returned" => OrderStatus::Returned,
            _ => OrderStatus::Unknown,
        }
    }

    pub fn map_proto_status_to_str(status_i32: i32) -> Result<&'static str, Status> {
        match OrderStatus::try_from(status_i32).map_err(|_| Status::invalid_argument("invalid status"))? {
            OrderStatus::Pending => Ok("pending"),
            OrderStatus::Processing => Ok("processing"),
            OrderStatus::Shipped => Ok("shipped"),
            OrderStatus::Delivered => Ok("delivered"),
            OrderStatus::Canceled => Ok("cancelled"),
            OrderStatus::Returned => Ok("returned"),
            _ => Err(Status::invalid_argument("invalid status")),
        }
    }
}

use status_helpers::{map_status_str_to_proto, map_proto_status_to_str};

fn map_service_order_to_proto(o: &crate::services::orders::OrderResponse) -> Order {
    let created_at_ts = prost_types::Timestamp {
        seconds: o.created_at.timestamp(),
        nanos: o.created_at.timestamp_subsec_nanos() as i32,
    };
    let amount_i64 = o.total_amount.to_i64().unwrap_or(0);
    Order {
        id: o.id.to_string(),
        customer_id: o.customer_id.to_string(),
        items: vec![],
        total_amount: Some(crate::proto::common::Money { currency: o.currency.clone(), amount: amount_i64 }),
        status: map_status_str_to_proto(&o.status) as i32,
        created_at: Some(created_at_ts),
        shipping_address: None,
        billing_address: None,
        payment_method_id: o.payment_method.clone().unwrap_or_default(),
        shipment_id: o.tracking_number.clone().unwrap_or_default(),
    }
}

// Order Service Implementation
#[tonic::async_trait]
impl order_service_server::OrderService for StateSetApi {
    #[instrument(skip(self, request), fields(customer_id))]
    async fn create_order(
        &self,
        request: Request<CreateOrderRequest>,
    ) -> Result<Response<CreateOrderResponse>, Status> {
        let req = request.into_inner();
        let order = req.order.ok_or_else(|| Status::invalid_argument("order is required"))?;

        let customer_id = Uuid::parse_str(&order.customer_id)
            .map_err(|_| Status::invalid_argument("invalid customer_id"))?;

        // Generate order number with cleaner format
        let generated_order_number = format!("ORD-{}", &Uuid::new_v4().to_string()[..8].to_uppercase());

        // Map Money -> Decimal with proper scale handling
        let (total_amount_decimal, currency) = order.total_amount
            .map(|m| {
                let d = Decimal::from_i128_with_scale(m.amount as i128, 2); // Assume cents
                (d, m.currency)
            })
            .unwrap_or_else(|| (Decimal::from_i128_with_scale(0, 2), "USD".to_string()));

        let created = self.order_service
            .create_order(ServiceCreateOrderRequest {
                customer_id,
                order_number: generated_order_number,
                total_amount: total_amount_decimal,
                currency,
                payment_status: "pending".to_string(),
                fulfillment_status: "unfulfilled".to_string(),
                payment_method: (!order.payment_method_id.is_empty()).then(|| order.payment_method_id),
                shipping_method: None,
                notes: None,
                shipping_address: None,
                billing_address: None,
            })
            .await
            .map_err(|e| {
                error!("Failed to create order: {}", e);
                Status::internal(format!("Failed to create order: {}", e))
            })?;

        info!("Order created successfully: {}", created.id);
        
        let resp = CreateOrderResponse {
            order_id: created.id.to_string(),
            status: map_status_str_to_proto(&created.status) as i32,
            created_at: Some(prost_types::Timestamp {
                seconds: created.created_at.timestamp(),
                nanos: created.created_at.timestamp_subsec_nanos() as i32,
            }),
        };
        Ok(Response::new(resp))
    }

    #[instrument(skip(self, request), fields(order_id))]
    async fn get_order(
        &self,
        request: Request<GetOrderRequest>,
    ) -> Result<Response<GetOrderResponse>, Status> {
        let order_id = &request.get_ref().order_id;
        
        let id = Uuid::parse_str(order_id)
            .map_err(|_| Status::invalid_argument("invalid order_id"))?;

        let order = self.order_service
            .get_order(id)
            .await
            .map_err(|e| {
                error!("Failed to get order {}: {}", order_id, e);
                Status::internal(format!("Failed to get order: {}", e))
            })?
            .ok_or_else(|| Status::not_found("Order not found"))?;

        let response = GetOrderResponse { order: Some(map_service_order_to_proto(&order)) };
        Ok(Response::new(response))
    }

    #[instrument(skip(self, request), fields(order_id, new_status))]
    async fn update_order_status(
        &self,
        request: Request<UpdateOrderStatusRequest>,
    ) -> Result<Response<UpdateOrderStatusResponse>, Status> {
        let req = request.into_inner();
        let order_id = Uuid::parse_str(&req.order_id)
            .map_err(|_| Status::invalid_argument("invalid order_id"))?;
        let new_status_str = map_proto_status_to_str(req.new_status)?.to_string();

        self.order_status_service
            .update_status(order_id, new_status_str.clone())
            .await
            .map_err(|e| {
                error!("Failed to update order status: {}", e);
                match e {
                    crate::errors::ServiceError::ValidationError(msg) => Status::invalid_argument(msg),
                    crate::errors::ServiceError::NotFound(msg) => Status::not_found(msg),
                    _ => Status::internal(format!("Failed to update status: {}", e)),
                }
            })?;
        
        info!("Order {} status updated to {}", req.order_id, new_status_str);
        
        let response = UpdateOrderStatusResponse {
            order_id: req.order_id,
            status: req.new_status,
        };
        
        Ok(Response::new(response))
    }

    async fn list_orders(
        &self,
        _request: Request<ListOrdersRequest>,
    ) -> Result<Response<ListOrdersResponse>, Status> {
        info!("Listing orders");
        let svc = OrdersService::new(self.db_pool.clone(), None);
        let page = 1u64;
        let per_page = 20u64;
        let list = svc.list_orders(page, per_page).await.map_err(|e| Status::internal(e.to_string()))?;
        let orders: Vec<Order> = list.orders.iter().map(map_service_order_to_proto).collect();
        let response = ListOrdersResponse { orders, pagination: None };
        Ok(Response::new(response))
    }
}

// Inventory Service Implementation
#[tonic::async_trait]
impl inventory_service_server::InventoryService for StateSetApi {
    #[instrument(skip(self, request), fields(product_id, warehouse_id))]
    async fn get_inventory(
        &self,
        request: Request<GetInventoryRequest>,
    ) -> Result<Response<GetInventoryResponse>, Status> {
        let req = request.into_inner();
        let product_id = Uuid::parse_str(&req.product_id)
            .map_err(|_| Status::invalid_argument("invalid product_id"))?;
        let warehouse_id = Uuid::parse_str(&req.warehouse_id)
            .map_err(|_| Status::invalid_argument("invalid warehouse_id"))?;

        let inv = self.inventory_service
            .get_inventory(&product_id, &warehouse_id)
            .await
            .map_err(|e| {
                error!("Failed to get inventory: {}", e);
                Status::internal(format!("Failed to get inventory: {}", e))
            })?;

        let item = inv.map(|inv| InventoryItem {
            product_id: inv.sku,
            quantity: inv.available,
            warehouse_id: inv.warehouse,
            location: String::new(),
            last_updated: Some(prost_types::Timestamp {
                seconds: inv.updated_at.timestamp(),
                nanos: inv.updated_at.timestamp_subsec_nanos() as i32,
            }),
        });
        
        Ok(Response::new(GetInventoryResponse { item }))
    }

    #[instrument(skip(self, request), fields(product_id, warehouse_id, quantity_change))]
    async fn update_inventory(
        &self,
        request: Request<UpdateInventoryRequest>,
    ) -> Result<Response<UpdateInventoryResponse>, Status> {
        let req = request.into_inner();
        
        let product_id = Uuid::parse_str(&req.product_id)
            .map_err(|_| Status::invalid_argument("invalid product_id"))?;
        let warehouse_id = Uuid::parse_str(&req.warehouse_id)
            .map_err(|_| Status::invalid_argument("invalid warehouse_id"))?;
            
        let command = AdjustInventoryCommand {
            product_id: Some(product_id),
            location_id: Some(warehouse_id),
            adjustment_quantity: Some(req.quantity_change),
            reason: (!req.reason.is_empty()).then(|| req.reason.clone()),
        };
        
        self.inventory_service
            .adjust_inventory(command)
            .await
            .map_err(|e| {
                error!("Failed to update inventory: {}", e);
                Status::internal(format!("Failed to update inventory: {}", e))
            })?;
            
        // Get updated inventory to return actual quantity
        let updated = self.inventory_service
            .get_inventory(&product_id, &warehouse_id)
            .await
            .ok()
            .flatten();
            
        let new_quantity = updated.map(|inv| inv.available).unwrap_or(0);
        
        let response = UpdateInventoryResponse {
            product_id: req.product_id,
            new_quantity,
            warehouse_id: req.warehouse_id,
        };
        Ok(Response::new(response))
    }

    async fn list_inventory(
        &self,
        _request: Request<ListInventoryRequest>,
    ) -> Result<Response<ListInventoryResponse>, Status> {
        info!("Listing inventory");
        let svc = InvService::new(self.db_pool.clone(), crate::events::EventSender::new(tokio::sync::mpsc::channel(1).0));
        let page = 1u64;
        let limit = 50u64;
        let (models, _total) = svc
            .list_inventory(page, limit)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        let items: Vec<InventoryItem> = models
            .into_iter()
            .map(|inv| InventoryItem {
                product_id: inv.sku,
                quantity: inv.available,
                warehouse_id: inv.warehouse,
                location: String::new(),
                last_updated: Some(prost_types::Timestamp {
                    seconds: inv.updated_at.timestamp(),
                    nanos: inv.updated_at.timestamp_subsec_nanos() as i32,
                }),
            })
            .collect();
        Ok(Response::new(ListInventoryResponse { items, pagination: None }))
    }

    #[instrument(skip(self, request), fields(product_id, order_id, quantity))]
    async fn reserve_inventory(
        &self,
        request: Request<ReserveInventoryRequest>,
    ) -> Result<Response<ReserveInventoryResponse>, Status> {
        let req = request.into_inner();
        let product_id = Uuid::parse_str(&req.product_id)
            .map_err(|_| Status::invalid_argument("invalid product_id"))?;
        let order_id = Uuid::parse_str(&req.order_id)
            .map_err(|_| Status::invalid_argument("invalid order_id"))?;
            
        let reservation_id = self.inventory_service
            .reserve_inventory_simple(&product_id, &order_id, req.quantity)
            .await
            .map_err(|e| {
                error!("Failed to reserve inventory: {}", e);
                match e {
                    crate::errors::ServiceError::InsufficientStock(_) => {
                        Status::failed_precondition(format!("Insufficient stock: {}", e))
                    }
                    _ => Status::internal(format!("Failed to reserve inventory: {}", e)),
                }
            })?;
            
        info!("Inventory reserved successfully: {}", reservation_id);
        
        Ok(Response::new(ReserveInventoryResponse {
            success: true,
            reservation_id,
        }))
    }
}

// Return Service Implementation
#[tonic::async_trait]
impl return_service_server::ReturnService for StateSetApi {
    async fn create_return(
        &self,
        request: Request<CreateReturnRequest>,
    ) -> Result<Response<CreateReturnResponse>, Status> {
        info!("Creating return: {:?}", request.get_ref());
        
        let response = CreateReturnResponse {
            return_id: Uuid::new_v4().to_string(),
            status: ReturnStatus::Requested as i32,
        };
        
        Ok(Response::new(response))
    }

    async fn get_return(
        &self,
        request: Request<GetReturnRequest>,
    ) -> Result<Response<GetReturnResponse>, Status> {
        let return_id = &request.get_ref().return_id;
        info!("Getting return: {}", return_id);
        
        let response = GetReturnResponse {
            r#return: Some(Return {
                id: return_id.clone(),
                order_id: String::new(),
                customer_id: String::new(),
                items: vec![],
                status: ReturnStatus::Requested as i32,
                reason: String::new(),
                created_at: Some(prost_types::Timestamp {
                    seconds: chrono::Utc::now().timestamp() - 3600,
                    nanos: 0,
                }),
                updated_at: None,
            }),
        };
        
        Ok(Response::new(response))
    }

    async fn update_return_status(
        &self,
        request: Request<UpdateReturnStatusRequest>,
    ) -> Result<Response<UpdateReturnStatusResponse>, Status> {
        info!("Updating return status: {:?}", request.get_ref());
        let req = request.into_inner();
        let _id = Uuid::parse_str(&req.return_id)
            .map_err(|_| Status::invalid_argument("invalid return_id"))?;

        let new_status = ReturnStatus::try_from(req.new_status).map_err(|_| Status::invalid_argument("invalid status"))?;
        let response = UpdateReturnStatusResponse {
            return_id: req.return_id,
            status: new_status as i32,
        };
        Ok(Response::new(response))
    }

    async fn list_returns(
        &self,
        _request: Request<ListReturnsRequest>,
    ) -> Result<Response<ListReturnsResponse>, Status> {
        info!("Listing returns");
        let response = ListReturnsResponse { returns: vec![], pagination: None };
        Ok(Response::new(response))
    }
}

// Basic placeholder implementations for other services
#[tonic::async_trait]
impl warranty_service_server::WarrantyService for StateSetApi {
    async fn create_warranty(
        &self,
        _request: Request<CreateWarrantyRequest>,
    ) -> Result<Response<CreateWarrantyResponse>, Status> {
        Ok(Response::new(CreateWarrantyResponse { 
            warranty_id: Uuid::new_v4().to_string() 
        }))
    }

    async fn get_warranty(
        &self,
        request: Request<GetWarrantyRequest>,
    ) -> Result<Response<GetWarrantyResponse>, Status> {
        let id = &request.get_ref().warranty_id;
        let warranty = Warranty {
            id: id.clone(),
            order_id: String::new(),
            product_id: String::new(),
            customer_id: String::new(),
            start_date: None,
            end_date: None,
            status: String::from("active"),
            terms: String::new(),
        };
        Ok(Response::new(GetWarrantyResponse { warranty: Some(warranty) }))
    }

    async fn update_warranty(
        &self,
        request: Request<UpdateWarrantyRequest>,
    ) -> Result<Response<UpdateWarrantyResponse>, Status> {
        let w = request.into_inner().warranty.ok_or_else(|| Status::invalid_argument("warranty is required"))?;
        Ok(Response::new(UpdateWarrantyResponse { warranty: Some(w) }))
    }

    async fn list_warranties(
        &self,
        _request: Request<ListWarrantiesRequest>,
    ) -> Result<Response<ListWarrantiesResponse>, Status> {
        Ok(Response::new(ListWarrantiesResponse { warranties: vec![], pagination: None }))
    }
}

#[tonic::async_trait]
impl shipment_service_server::ShipmentService for StateSetApi {
    async fn create_shipment(
        &self,
        _request: Request<CreateShipmentRequest>,
    ) -> Result<Response<CreateShipmentResponse>, Status> {
        Ok(Response::new(CreateShipmentResponse { 
            shipment_id: Uuid::new_v4().to_string() 
        }))
    }

    async fn get_shipment(
        &self,
        request: Request<GetShipmentRequest>,
    ) -> Result<Response<GetShipmentResponse>, Status> {
        let shipment_id = &request.get_ref().shipment_id;
        let shipment = Shipment {
            id: shipment_id.clone(),
            order_id: String::new(),
            carrier: String::new(),
            tracking_number: String::new(),
            shipping_address: None,
            status: String::from("created"),
            created_at: Some(prost_types::Timestamp { seconds: chrono::Utc::now().timestamp() - 600, nanos: 0 }),
            updated_at: None,
            items: vec![],
        };
        Ok(Response::new(GetShipmentResponse { shipment: Some(shipment) }))
    }

    async fn update_shipment_status(
        &self,
        request: Request<UpdateShipmentStatusRequest>,
    ) -> Result<Response<UpdateShipmentStatusResponse>, Status> {
        let req = request.into_inner();
        let _id = Uuid::parse_str(&req.shipment_id)
            .map_err(|_| Status::invalid_argument("invalid shipment_id"))?;
        Ok(Response::new(UpdateShipmentStatusResponse { shipment_id: req.shipment_id, status: req.new_status }))
    }

    async fn list_shipments(
        &self,
        _request: Request<ListShipmentsRequest>,
    ) -> Result<Response<ListShipmentsResponse>, Status> {
        Ok(Response::new(ListShipmentsResponse { shipments: vec![], pagination: None }))
    }
}

#[tonic::async_trait]
impl work_order_service_server::WorkOrderService for StateSetApi {
    async fn create_work_order(
        &self,
        _request: Request<CreateWorkOrderRequest>,
    ) -> Result<Response<CreateWorkOrderResponse>, Status> {
        Err(Status::unimplemented("Work order service not yet implemented"))
    }

    async fn get_work_order(
        &self,
        _request: Request<GetWorkOrderRequest>,
    ) -> Result<Response<GetWorkOrderResponse>, Status> {
        Err(Status::unimplemented("Work order service not yet implemented"))
    }

    async fn update_work_order(
        &self,
        _request: Request<UpdateWorkOrderRequest>,
    ) -> Result<Response<UpdateWorkOrderResponse>, Status> {
        Err(Status::unimplemented("Work order service not yet implemented"))
    }

    async fn list_work_orders(
        &self,
        _request: Request<ListWorkOrdersRequest>,
    ) -> Result<Response<ListWorkOrdersResponse>, Status> {
        Err(Status::unimplemented("Work order service not yet implemented"))
    }

    async fn delete_work_order(
        &self,
        _request: Request<DeleteWorkOrderRequest>,
    ) -> Result<Response<DeleteWorkOrderResponse>, Status> {
        Err(Status::unimplemented("Work order service not yet implemented"))
    }

    async fn assign_work_order(
        &self,
        _request: Request<AssignWorkOrderRequest>,
    ) -> Result<Response<AssignWorkOrderResponse>, Status> {
        Err(Status::unimplemented("Work order service not yet implemented"))
    }

    async fn complete_work_order(
        &self,
        _request: Request<CompleteWorkOrderRequest>,
    ) -> Result<Response<CompleteWorkOrderResponse>, Status> {
        Err(Status::unimplemented("Work order service not yet implemented"))
    }
}