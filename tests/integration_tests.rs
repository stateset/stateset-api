use tonic::Request;
use your_crate_name::proto::order_service_client::OrderServiceClient;
use your_crate_name::proto::{CreateOrderRequest, Order, OrderItem};

#[tokio::test]
async fn test_create_order() {
    let mut client = OrderServiceClient::connect("http://[::1]:50051").await.unwrap();

    let order = Order {
        id: "".to_string(),
        customer_id: "test_customer".to_string(),
        items: vec![OrderItem {
            product_id: "test_product".to_string(),
            quantity: 2,
            price: 10.0,
        }],
        total_amount: 20.0,
        status: "PENDING".to_string(),
        created_at: None,
    };

    let request = Request::new(CreateOrderRequest {
        order: Some(order),
    });

    let response = client.create_order(request).await.unwrap();
    let response = response.into_inner();

    assert!(!response.order_id.is_empty());
    assert_eq!(response.status, "CREATED");
}

#[tokio::test]
async fn test_get_order() {
    let mut client = OrderServiceClient::connect("http://[::1]:50051").await.unwrap();

    // First, create an order
    let create_order_response = client.create_order(Request::new(CreateOrderRequest {
        order: Some(Order {
            id: "".to_string(),
            customer_id: "test_customer".to_string(),
            items: vec![OrderItem {
                product_id: "test_product".to_string(),
                quantity: 2,
                price: 10.0,
            }],
            total_amount: 20.0,
            status: "PENDING".to_string(),
            created_at: None,
        }),
    })).await.unwrap().into_inner();

    // Now, get the order
    let get_order_response = client.get_order(Request::new(your_crate_name::proto::GetOrderRequest {
        order_id: create_order_response.order_id,
    })).await.unwrap().into_inner();

    let order = get_order_response.order.unwrap();
    assert_eq!(order.customer_id, "test_customer");
    assert_eq!(order.total_amount, 20.0);
    assert_eq!(order.status, "PENDING");
}