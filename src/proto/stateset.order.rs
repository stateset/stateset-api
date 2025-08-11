#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Order {
    #[prost(string, tag = "1")]
    pub id: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub customer_id: ::prost::alloc::string::String,
    #[prost(message, repeated, tag = "3")]
    pub items: ::prost::alloc::vec::Vec<OrderItem>,
    #[prost(message, optional, tag = "4")]
    pub total_amount: ::core::option::Option<super::common::Money>,
    #[prost(string, tag = "5")]
    pub status: ::prost::alloc::string::String,
    #[prost(message, optional, tag = "6")]
    pub created_at: ::core::option::Option<::prost_types::Timestamp>,
    #[prost(message, optional, tag = "7")]
    pub shipping_address: ::core::option::Option<super::common::Address>,
    #[prost(message, optional, tag = "8")]
    pub billing_address: ::core::option::Option<super::common::Address>,
    #[prost(string, tag = "9")]
    pub payment_method_id: ::prost::alloc::string::String,
    #[prost(string, tag = "10")]
    pub shipment_id: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct OrderItem {
    #[prost(string, tag = "1")]
    pub product_id: ::prost::alloc::string::String,
    #[prost(int32, tag = "2")]
    pub quantity: i32,
    #[prost(message, optional, tag = "3")]
    pub unit_price: ::core::option::Option<super::common::Money>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CreateOrderRequest {
    #[prost(message, optional, tag = "1")]
    pub order: ::core::option::Option<Order>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CreateOrderResponse {
    #[prost(string, tag = "1")]
    pub order_id: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub status: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetOrderRequest {
    #[prost(string, tag = "1")]
    pub order_id: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetOrderResponse {
    #[prost(message, optional, tag = "1")]
    pub order: ::core::option::Option<Order>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UpdateOrderStatusRequest {
    #[prost(string, tag = "1")]
    pub order_id: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub new_status: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UpdateOrderStatusResponse {
    #[prost(string, tag = "1")]
    pub order_id: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub status: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListOrdersRequest {
    #[prost(string, tag = "1")]
    pub customer_id: ::prost::alloc::string::String,
    #[prost(message, optional, tag = "2")]
    pub start_date: ::core::option::Option<::prost_types::Timestamp>,
    #[prost(message, optional, tag = "3")]
    pub end_date: ::core::option::Option<::prost_types::Timestamp>,
    #[prost(string, tag = "4")]
    pub status: ::prost::alloc::string::String,
    #[prost(message, optional, tag = "5")]
    pub pagination: ::core::option::Option<super::common::Pagination>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListOrdersResponse {
    #[prost(message, repeated, tag = "1")]
    pub orders: ::prost::alloc::vec::Vec<Order>,
    #[prost(message, optional, tag = "2")]
    pub pagination: ::core::option::Option<super::common::PaginatedResponse>,
}
/// Generated client implementations.
pub mod order_service_client {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    use tonic::codegen::http::Uri;
    #[derive(Debug, Clone)]
    pub struct OrderServiceClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl OrderServiceClient<tonic::transport::Channel> {
        /// Attempt to create a new client by connecting to a given endpoint.
        pub async fn connect<D>(dst: D) -> Result<Self, tonic::transport::Error>
        where
            D: std::convert::TryInto<tonic::transport::Endpoint>,
            D::Error: Into<StdError>,
        {
            let conn = tonic::transport::Endpoint::new(dst)?.connect().await?;
            Ok(Self::new(conn))
        }
    }
    impl<T> OrderServiceClient<T>
    where
        T: tonic::client::GrpcService<tonic::body::BoxBody>,
        T::Error: Into<StdError>,
        T::ResponseBody: Body<Data = Bytes> + Send + 'static,
        <T::ResponseBody as Body>::Error: Into<StdError> + Send,
    {
        pub fn new(inner: T) -> Self {
            let inner = tonic::client::Grpc::new(inner);
            Self { inner }
        }
        pub fn with_origin(inner: T, origin: Uri) -> Self {
            let inner = tonic::client::Grpc::with_origin(inner, origin);
            Self { inner }
        }
        pub fn with_interceptor<F>(
            inner: T,
            interceptor: F,
        ) -> OrderServiceClient<InterceptedService<T, F>>
        where
            F: tonic::service::Interceptor,
            T::ResponseBody: Default,
            T: tonic::codegen::Service<
                http::Request<tonic::body::BoxBody>,
                Response = http::Response<
                    <T as tonic::client::GrpcService<tonic::body::BoxBody>>::ResponseBody,
                >,
            >,
            <T as tonic::codegen::Service<
                http::Request<tonic::body::BoxBody>,
            >>::Error: Into<StdError> + Send + Sync,
        {
            OrderServiceClient::new(InterceptedService::new(inner, interceptor))
        }
        /// Compress requests with the given encoding.
        ///
        /// This requires the server to support it otherwise it might respond with an
        /// error.
        #[must_use]
        pub fn send_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.inner = self.inner.send_compressed(encoding);
            self
        }
        /// Enable decompressing responses.
        #[must_use]
        pub fn accept_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.inner = self.inner.accept_compressed(encoding);
            self
        }
        pub async fn create_order(
            &mut self,
            request: impl tonic::IntoRequest<super::CreateOrderRequest>,
        ) -> Result<tonic::Response<super::CreateOrderResponse>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/stateset.order.OrderService/CreateOrder",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
        pub async fn get_order(
            &mut self,
            request: impl tonic::IntoRequest<super::GetOrderRequest>,
        ) -> Result<tonic::Response<super::GetOrderResponse>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/stateset.order.OrderService/GetOrder",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
        pub async fn update_order_status(
            &mut self,
            request: impl tonic::IntoRequest<super::UpdateOrderStatusRequest>,
        ) -> Result<tonic::Response<super::UpdateOrderStatusResponse>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/stateset.order.OrderService/UpdateOrderStatus",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
        pub async fn list_orders(
            &mut self,
            request: impl tonic::IntoRequest<super::ListOrdersRequest>,
        ) -> Result<tonic::Response<super::ListOrdersResponse>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/stateset.order.OrderService/ListOrders",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
    }
}
/// Generated server implementations.
pub mod order_service_server {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    /// Generated trait containing gRPC methods that should be implemented for use with OrderServiceServer.
    #[async_trait]
    pub trait OrderService: Send + Sync + 'static {
        async fn create_order(
            &self,
            request: tonic::Request<super::CreateOrderRequest>,
        ) -> Result<tonic::Response<super::CreateOrderResponse>, tonic::Status>;
        async fn get_order(
            &self,
            request: tonic::Request<super::GetOrderRequest>,
        ) -> Result<tonic::Response<super::GetOrderResponse>, tonic::Status>;
        async fn update_order_status(
            &self,
            request: tonic::Request<super::UpdateOrderStatusRequest>,
        ) -> Result<tonic::Response<super::UpdateOrderStatusResponse>, tonic::Status>;
        async fn list_orders(
            &self,
            request: tonic::Request<super::ListOrdersRequest>,
        ) -> Result<tonic::Response<super::ListOrdersResponse>, tonic::Status>;
    }
    #[derive(Debug)]
    pub struct OrderServiceServer<T: OrderService> {
        inner: _Inner<T>,
        accept_compression_encodings: EnabledCompressionEncodings,
        send_compression_encodings: EnabledCompressionEncodings,
    }
    struct _Inner<T>(Arc<T>);
    impl<T: OrderService> OrderServiceServer<T> {
        pub fn new(inner: T) -> Self {
            Self::from_arc(Arc::new(inner))
        }
        pub fn from_arc(inner: Arc<T>) -> Self {
            let inner = _Inner(inner);
            Self {
                inner,
                accept_compression_encodings: Default::default(),
                send_compression_encodings: Default::default(),
            }
        }
        pub fn with_interceptor<F>(
            inner: T,
            interceptor: F,
        ) -> InterceptedService<Self, F>
        where
            F: tonic::service::Interceptor,
        {
            InterceptedService::new(Self::new(inner), interceptor)
        }
        /// Enable decompressing requests with the given encoding.
        #[must_use]
        pub fn accept_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.accept_compression_encodings.enable(encoding);
            self
        }
        /// Compress responses with the given encoding, if the client supports it.
        #[must_use]
        pub fn send_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.send_compression_encodings.enable(encoding);
            self
        }
    }
    impl<T, B> tonic::codegen::Service<http::Request<B>> for OrderServiceServer<T>
    where
        T: OrderService,
        B: Body + Send + 'static,
        B::Error: Into<StdError> + Send + 'static,
    {
        type Response = http::Response<tonic::body::BoxBody>;
        type Error = std::convert::Infallible;
        type Future = BoxFuture<Self::Response, Self::Error>;
        fn poll_ready(
            &mut self,
            _cx: &mut Context<'_>,
        ) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
        fn call(&mut self, req: http::Request<B>) -> Self::Future {
            let inner = self.inner.clone();
            match req.uri().path() {
                "/stateset.order.OrderService/CreateOrder" => {
                    #[allow(non_camel_case_types)]
                    struct CreateOrderSvc<T: OrderService>(pub Arc<T>);
                    impl<
                        T: OrderService,
                    > tonic::server::UnaryService<super::CreateOrderRequest>
                    for CreateOrderSvc<T> {
                        type Response = super::CreateOrderResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::CreateOrderRequest>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move {
                                (*inner).create_order(request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = CreateOrderSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/stateset.order.OrderService/GetOrder" => {
                    #[allow(non_camel_case_types)]
                    struct GetOrderSvc<T: OrderService>(pub Arc<T>);
                    impl<
                        T: OrderService,
                    > tonic::server::UnaryService<super::GetOrderRequest>
                    for GetOrderSvc<T> {
                        type Response = super::GetOrderResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::GetOrderRequest>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move { (*inner).get_order(request).await };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = GetOrderSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/stateset.order.OrderService/UpdateOrderStatus" => {
                    #[allow(non_camel_case_types)]
                    struct UpdateOrderStatusSvc<T: OrderService>(pub Arc<T>);
                    impl<
                        T: OrderService,
                    > tonic::server::UnaryService<super::UpdateOrderStatusRequest>
                    for UpdateOrderStatusSvc<T> {
                        type Response = super::UpdateOrderStatusResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::UpdateOrderStatusRequest>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move {
                                (*inner).update_order_status(request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = UpdateOrderStatusSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/stateset.order.OrderService/ListOrders" => {
                    #[allow(non_camel_case_types)]
                    struct ListOrdersSvc<T: OrderService>(pub Arc<T>);
                    impl<
                        T: OrderService,
                    > tonic::server::UnaryService<super::ListOrdersRequest>
                    for ListOrdersSvc<T> {
                        type Response = super::ListOrdersResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::ListOrdersRequest>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move { (*inner).list_orders(request).await };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = ListOrdersSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                _ => {
                    Box::pin(async move {
                        Ok(
                            http::Response::builder()
                                .status(200)
                                .header("grpc-status", "12")
                                .header("content-type", "application/grpc")
                                .body(empty_body())
                                .unwrap(),
                        )
                    })
                }
            }
        }
    }
    impl<T: OrderService> Clone for OrderServiceServer<T> {
        fn clone(&self) -> Self {
            let inner = self.inner.clone();
            Self {
                inner,
                accept_compression_encodings: self.accept_compression_encodings,
                send_compression_encodings: self.send_compression_encodings,
            }
        }
    }
    impl<T: OrderService> Clone for _Inner<T> {
        fn clone(&self) -> Self {
            Self(self.0.clone())
        }
    }
    impl<T: std::fmt::Debug> std::fmt::Debug for _Inner<T> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{:?}", self.0)
        }
    }
    impl<T: OrderService> tonic::server::NamedService for OrderServiceServer<T> {
        const NAME: &'static str = "stateset.order.OrderService";
    }
}
