use std::sync::Arc;

use chrono::{DateTime, Utc};
use rust_decimal::{prelude::ToPrimitive, Decimal};
use tonic::{Request, Response, Status};
use tracing::{error, info, instrument};
use uuid::Uuid;

use crate::{
    db::{DatabaseAccess, DbPool},
    errors::grpc::IntoGrpcStatus,
    events::EventSender,
    proto::{inventory::*, order::*, return_order::*, shipment::*, warranty::*, work_order::*},
    services::inventory::{
        AdjustInventoryCommand, InventoryService as InvService, InventorySnapshot, LocationBalance,
        ReserveInventoryCommand,
    },
    services::order_status::OrderStatusService,
    services::orders::{
        CreateOrderRequest as ServiceCreateOrderRequest, OrderSearchQuery,
        OrderService as OrdersService, OrderSortField, SortDirection,
    },
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
        let order_service = Arc::new(OrdersService::new(
            db_pool.clone(),
            Some(Arc::new(event_sender.clone())),
        ));
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

    pub fn with_event_sender(
        db: Arc<DatabaseAccess>,
        db_pool: Arc<DbPool>,
        event_sender: EventSender,
    ) -> Self {
        let inventory_service = Arc::new(InvService::new(db_pool.clone(), event_sender.clone()));
        let order_service = Arc::new(OrdersService::new(
            db_pool.clone(),
            Some(Arc::new(event_sender.clone())),
        ));
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
        match OrderStatus::try_from(status_i32)
            .map_err(|_| Status::invalid_argument("invalid status"))?
        {
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

use status_helpers::{map_proto_status_to_str, map_status_str_to_proto};

enum InventoryIdentifier {
    Id(i64),
    Number(String),
}

fn parse_inventory_identifier(value: &str) -> Result<InventoryIdentifier, Status> {
    if let Ok(id) = value.parse::<i64>() {
        Ok(InventoryIdentifier::Id(id))
    } else if !value.trim().is_empty() {
        Ok(InventoryIdentifier::Number(value.trim().to_string()))
    } else {
        Err(Status::invalid_argument("product_id is required"))
    }
}

async fn fetch_inventory_snapshot(
    service: &InvService,
    identifier: InventoryIdentifier,
) -> Result<InventorySnapshot, Status> {
    match identifier {
        InventoryIdentifier::Id(id) => service
            .get_snapshot_by_id(id)
            .await
            .map_err(|e| {
                error!("Failed to load inventory snapshot: {}", e);
                Status::internal("Failed to load inventory")
            })?
            .ok_or_else(|| Status::not_found("Inventory item not found")),
        InventoryIdentifier::Number(number) => service
            .get_snapshot_by_item_number(&number)
            .await
            .map_err(|e| {
                error!("Failed to load inventory snapshot: {}", e);
                Status::internal("Failed to load inventory")
            })?
            .ok_or_else(|| Status::not_found("Inventory item not found")),
    }
}

fn parse_location_id(value: &str) -> Result<i32, Status> {
    value.parse::<i32>().map_err(|_| {
        Status::invalid_argument("warehouse_id/location identifier must be a numeric value")
    })
}

fn balance_to_proto_item(
    item_number: &str,
    balance: &LocationBalance,
) -> Result<InventoryItem, Status> {
    let quantity = decimal_to_i32(balance.quantity_available)?;
    Ok(InventoryItem {
        product_id: item_number.to_string(),
        quantity,
        warehouse_id: balance.location_id.to_string(),
        location: balance
            .location_name
            .clone()
            .unwrap_or_else(|| "unknown".to_string()),
        last_updated: Some(timestamp_from(balance.updated_at)),
    })
}

fn decimal_to_i32(value: Decimal) -> Result<i32, Status> {
    value
        .round()
        .to_i32()
        .ok_or_else(|| Status::internal("quantity out of range"))
}

fn timestamp_from(dt: DateTime<Utc>) -> prost_types::Timestamp {
    prost_types::Timestamp {
        seconds: dt.timestamp(),
        nanos: dt.timestamp_subsec_nanos() as i32,
    }
}

fn map_service_order_to_proto(o: &crate::services::orders::OrderResponse) -> Order {
    let created_at_ts = prost_types::Timestamp {
        seconds: o.created_at.timestamp(),
        nanos: o.created_at.timestamp_subsec_nanos() as i32,
    };
    // Convert Decimal dollars to smallest currency unit (e.g., cents)
    let amount_i64 = (o.total_amount * Decimal::new(100, 0))
        .to_i64()
        .unwrap_or(i64::MAX);
    Order {
        id: o.id.to_string(),
        customer_id: o.customer_id.to_string(),
        items: vec![],
        total_amount: Some(crate::proto::common::Money {
            currency: o.currency.clone(),
            amount: amount_i64,
        }),
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
        let order = req
            .order
            .ok_or_else(|| Status::invalid_argument("order is required"))?;

        let customer_id = Uuid::parse_str(&order.customer_id)
            .map_err(|_| Status::invalid_argument("invalid customer_id"))?;

        // Generate order number with cleaner format
        let generated_order_number =
            format!("ORD-{}", &Uuid::new_v4().to_string()[..8].to_uppercase());

        // Map Money -> Decimal with proper scale handling
        let (total_amount_decimal, currency) = order
            .total_amount
            .map(|m| {
                let d = Decimal::from_i128_with_scale(m.amount as i128, 2); // Assume cents
                (d, m.currency)
            })
            .unwrap_or_else(|| (Decimal::from_i128_with_scale(0, 2), "USD".to_string()));

        let created = self
            .order_service
            .create_order(ServiceCreateOrderRequest {
                customer_id,
                order_number: generated_order_number,
                total_amount: total_amount_decimal,
                currency,
                payment_status: "pending".to_string(),
                fulfillment_status: "unfulfilled".to_string(),
                payment_method: (!order.payment_method_id.is_empty())
                    .then(|| order.payment_method_id),
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

        let id =
            Uuid::parse_str(order_id).map_err(|_| Status::invalid_argument("invalid order_id"))?;

        let order = self
            .order_service
            .get_order(id)
            .await
            .map_err(|e| {
                error!("Failed to get order {}: {}", order_id, e);
                Status::internal(format!("Failed to get order: {}", e))
            })?
            .ok_or_else(|| Status::not_found("Order not found"))?;

        let response = GetOrderResponse {
            order: Some(map_service_order_to_proto(&order)),
        };
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
                e.into_grpc_status()
            })?;

        info!(
            "Order {} status updated to {}",
            req.order_id, new_status_str
        );

        let response = UpdateOrderStatusResponse {
            order_id: req.order_id,
            status: req.new_status,
        };

        Ok(Response::new(response))
    }

    async fn list_orders(
        &self,
        request: Request<ListOrdersRequest>,
    ) -> Result<Response<ListOrdersResponse>, Status> {
        info!("Listing orders");
        let req = request.into_inner();

        let customer_id = if req.customer_id.is_empty() {
            None
        } else {
            Some(
                Uuid::parse_str(&req.customer_id)
                    .map_err(|_| Status::invalid_argument("invalid customer_id"))?,
            )
        };
        let status = if req.status == 0 {
            None
        } else {
            Some(map_proto_status_to_str(req.status)?.to_string())
        };
        let start_date = req.start_date.as_ref().and_then(|ts| {
            chrono::DateTime::<chrono::Utc>::from_timestamp(ts.seconds, ts.nanos as u32)
        });
        let end_date = req.end_date.as_ref().and_then(|ts| {
            chrono::DateTime::<chrono::Utc>::from_timestamp(ts.seconds, ts.nanos as u32)
        });

        let (page, per_page) = match req.pagination {
            Some(p) => {
                let page = if p.page <= 0 { 1 } else { p.page as u64 };
                let mut per_page = if p.per_page <= 0 {
                    20
                } else {
                    p.per_page as u64
                };
                if per_page > 100 {
                    per_page = 100;
                }
                (page, per_page)
            }
            None => (1u64, 20u64),
        };

        let list = self
            .order_service
            .search_orders(OrderSearchQuery {
                customer_id,
                status,
                from_date: start_date,
                to_date: end_date,
                search: None,
                sort_field: OrderSortField::CreatedAt,
                sort_direction: SortDirection::Desc,
                page,
                per_page,
            })
            .await
            .map_err(|e| {
                error!("Failed to list orders: {}", e);
                e.into_grpc_status()
            })?;

        let orders: Vec<Order> = list.orders.iter().map(map_service_order_to_proto).collect();
        let total_pages = ((list.total + per_page - 1) / per_page) as i32;
        let pagination = Some(crate::proto::common::PaginationResponse {
            total_items: list.total as i32,
            total_pages,
            current_page: page as i32,
            items_per_page: per_page as i32,
            has_next_page: (page as u64) < (total_pages as u64),
            has_previous_page: page > 1,
        });
        let response = ListOrdersResponse { orders, pagination };
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
        let identifier = parse_inventory_identifier(&req.product_id)?;
        let location_id = parse_location_id(&req.warehouse_id)?;

        let snapshot = fetch_inventory_snapshot(&self.inventory_service, identifier).await?;
        let balance = self
            .inventory_service
            .get_location_balance(snapshot.inventory_item_id, location_id)
            .await
            .map_err(|e| {
                error!("Failed to get inventory: {}", e);
                Status::internal("Failed to fetch inventory")
            })?
            .ok_or_else(|| Status::not_found("Inventory balance not found"))?;

        let item = balance_to_proto_item(&snapshot.item_number, &balance)?;

        Ok(Response::new(GetInventoryResponse { item: Some(item) }))
    }

    #[instrument(skip(self, request), fields(product_id, warehouse_id, quantity_change))]
    async fn update_inventory(
        &self,
        request: Request<UpdateInventoryRequest>,
    ) -> Result<Response<UpdateInventoryResponse>, Status> {
        let req = request.into_inner();
        let identifier = parse_inventory_identifier(&req.product_id)?;
        let location_id = parse_location_id(&req.warehouse_id)?;
        let snapshot = fetch_inventory_snapshot(&self.inventory_service, identifier).await?;

        self.inventory_service
            .adjust_inventory(AdjustInventoryCommand {
                inventory_item_id: Some(snapshot.inventory_item_id),
                item_number: None,
                location_id,
                quantity_delta: Decimal::from(req.quantity_change),
                reason: (!req.reason.is_empty()).then(|| req.reason.clone()),
            })
            .await
            .map_err(|e| {
                error!("Failed to update inventory: {}", e);
                e.into_grpc_status()
            })?;

        let updated_balance = self
            .inventory_service
            .get_location_balance(snapshot.inventory_item_id, location_id)
            .await
            .map_err(|e| {
                error!("Failed to fetch updated inventory: {}", e);
                Status::internal("Failed to fetch updated inventory")
            })?
            .ok_or_else(|| Status::internal("Inventory balance missing after update"))?;

        let new_quantity = decimal_to_i32(updated_balance.quantity_available)?;

        let response = UpdateInventoryResponse {
            product_id: snapshot.item_number,
            new_quantity,
            warehouse_id: location_id.to_string(),
        };
        Ok(Response::new(response))
    }

    async fn list_inventory(
        &self,
        request: Request<ListInventoryRequest>,
    ) -> Result<Response<ListInventoryResponse>, Status> {
        info!("Listing inventory");
        let req = request.into_inner();
        let (page, per_page) = match req.pagination {
            Some(p) => {
                let page = if p.page <= 0 { 1 } else { p.page as u64 };
                let mut per_page = if p.per_page <= 0 {
                    50
                } else {
                    p.per_page as u64
                };
                if per_page > 100 {
                    per_page = 100;
                }
                (page, per_page)
            }
            None => (1u64, 50u64),
        };
        let location_filter = if !req.warehouse_id.is_empty() {
            Some(parse_location_id(&req.warehouse_id)?)
        } else {
            None
        };

        let product_filters: Vec<InventoryIdentifier> = req
            .product_ids
            .iter()
            .filter_map(|p| parse_inventory_identifier(p).ok())
            .collect();

        let (snapshots, total_items) = self
            .inventory_service
            .list_inventory(page, per_page)
            .await
            .map_err(|e| {
                error!("Failed to list inventory: {}", e);
                e.into_grpc_status()
            })?;

        let mut items = Vec::new();
        for snapshot in snapshots {
            if !product_filters.is_empty()
                && !product_filters.iter().any(|pf| match pf {
                    InventoryIdentifier::Id(id) => *id == snapshot.inventory_item_id,
                    InventoryIdentifier::Number(number) => {
                        snapshot.item_number.eq_ignore_ascii_case(number)
                    }
                })
            {
                continue;
            }

            let locations: Vec<&LocationBalance> = if let Some(loc_id) = location_filter {
                snapshot
                    .locations
                    .iter()
                    .filter(|balance| balance.location_id == loc_id)
                    .collect()
            } else {
                snapshot.locations.iter().collect()
            };

            for balance in locations {
                items.push(balance_to_proto_item(&snapshot.item_number, balance)?);
            }
        }

        let total_pages = ((total_items + per_page - 1) / per_page) as i32;
        let pagination = Some(crate::proto::common::PaginationResponse {
            total_items: items.len() as i32,
            total_pages,
            current_page: page as i32,
            items_per_page: per_page as i32,
            has_next_page: (page as u64) < (total_pages as u64),
            has_previous_page: page > 1,
        });
        Ok(Response::new(ListInventoryResponse { items, pagination }))
    }
    #[instrument(skip(self, request), fields(product_id, order_id, quantity))]
    async fn reserve_inventory(
        &self,
        request: Request<ReserveInventoryRequest>,
    ) -> Result<Response<ReserveInventoryResponse>, Status> {
        let req = request.into_inner();
        let identifier = parse_inventory_identifier(&req.product_id)?;
        let snapshot = fetch_inventory_snapshot(&self.inventory_service, identifier).await?;

        let location_id = if !req.order_id.is_empty() {
            parse_location_id(&req.order_id)?
        } else if snapshot.locations.len() == 1 {
            snapshot.locations[0].location_id
        } else {
            return Err(Status::invalid_argument(
                "order_id must provide a location identifier when multiple locations exist",
            ));
        };

        let reference_id = Uuid::parse_str(&req.order_id).ok();

        let outcome = self
            .inventory_service
            .reserve_inventory(ReserveInventoryCommand {
                inventory_item_id: Some(snapshot.inventory_item_id),
                item_number: None,
                location_id,
                quantity: Decimal::from(req.quantity),
                reference_id,
                reference_type: Some("ORDER".to_string()),
            })
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

        info!(
            "Inventory reserved successfully: {}",
            outcome.reservation_id
        );

        Ok(Response::new(ReserveInventoryResponse {
            success: true,
            reservation_id: outcome.id_str(),
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

        let new_status = ReturnStatus::try_from(req.new_status)
            .map_err(|_| Status::invalid_argument("invalid status"))?;
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
        let response = ListReturnsResponse {
            returns: vec![],
            pagination: None,
        };
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
            warranty_id: Uuid::new_v4().to_string(),
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
        Ok(Response::new(GetWarrantyResponse {
            warranty: Some(warranty),
        }))
    }

    async fn update_warranty(
        &self,
        request: Request<UpdateWarrantyRequest>,
    ) -> Result<Response<UpdateWarrantyResponse>, Status> {
        let w = request
            .into_inner()
            .warranty
            .ok_or_else(|| Status::invalid_argument("warranty is required"))?;
        Ok(Response::new(UpdateWarrantyResponse { warranty: Some(w) }))
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

#[tonic::async_trait]
impl shipment_service_server::ShipmentService for StateSetApi {
    async fn create_shipment(
        &self,
        _request: Request<CreateShipmentRequest>,
    ) -> Result<Response<CreateShipmentResponse>, Status> {
        Ok(Response::new(CreateShipmentResponse {
            shipment_id: Uuid::new_v4().to_string(),
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
            created_at: Some(prost_types::Timestamp {
                seconds: chrono::Utc::now().timestamp() - 600,
                nanos: 0,
            }),
            updated_at: None,
            items: vec![],
        };
        Ok(Response::new(GetShipmentResponse {
            shipment: Some(shipment),
        }))
    }

    async fn update_shipment_status(
        &self,
        request: Request<UpdateShipmentStatusRequest>,
    ) -> Result<Response<UpdateShipmentStatusResponse>, Status> {
        let req = request.into_inner();
        let _id = Uuid::parse_str(&req.shipment_id)
            .map_err(|_| Status::invalid_argument("invalid shipment_id"))?;
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

#[tonic::async_trait]
impl work_order_service_server::WorkOrderService for StateSetApi {
    async fn create_work_order(
        &self,
        _request: Request<CreateWorkOrderRequest>,
    ) -> Result<Response<CreateWorkOrderResponse>, Status> {
        Err(Status::unimplemented(
            "Work order service not yet implemented",
        ))
    }

    async fn get_work_order(
        &self,
        _request: Request<GetWorkOrderRequest>,
    ) -> Result<Response<GetWorkOrderResponse>, Status> {
        Err(Status::unimplemented(
            "Work order service not yet implemented",
        ))
    }

    async fn update_work_order(
        &self,
        _request: Request<UpdateWorkOrderRequest>,
    ) -> Result<Response<UpdateWorkOrderResponse>, Status> {
        Err(Status::unimplemented(
            "Work order service not yet implemented",
        ))
    }

    async fn list_work_orders(
        &self,
        _request: Request<ListWorkOrdersRequest>,
    ) -> Result<Response<ListWorkOrdersResponse>, Status> {
        Err(Status::unimplemented(
            "Work order service not yet implemented",
        ))
    }

    async fn delete_work_order(
        &self,
        _request: Request<DeleteWorkOrderRequest>,
    ) -> Result<Response<DeleteWorkOrderResponse>, Status> {
        Err(Status::unimplemented(
            "Work order service not yet implemented",
        ))
    }

    async fn assign_work_order(
        &self,
        _request: Request<AssignWorkOrderRequest>,
    ) -> Result<Response<AssignWorkOrderResponse>, Status> {
        Err(Status::unimplemented(
            "Work order service not yet implemented",
        ))
    }

    async fn complete_work_order(
        &self,
        _request: Request<CompleteWorkOrderRequest>,
    ) -> Result<Response<CompleteWorkOrderResponse>, Status> {
        Err(Status::unimplemented(
            "Work order service not yet implemented",
        ))
    }
}
