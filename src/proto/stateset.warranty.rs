#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Warranty {
    #[prost(string, tag = "1")]
    pub id: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub order_id: ::prost::alloc::string::String,
    #[prost(string, tag = "3")]
    pub product_id: ::prost::alloc::string::String,
    #[prost(string, tag = "4")]
    pub customer_id: ::prost::alloc::string::String,
    #[prost(message, optional, tag = "5")]
    pub start_date: ::core::option::Option<::prost_types::Timestamp>,
    #[prost(message, optional, tag = "6")]
    pub end_date: ::core::option::Option<::prost_types::Timestamp>,
    #[prost(string, tag = "7")]
    pub status: ::prost::alloc::string::String,
    #[prost(string, tag = "8")]
    pub terms: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CreateWarrantyRequest {
    #[prost(message, optional, tag = "1")]
    pub warranty: ::core::option::Option<Warranty>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CreateWarrantyResponse {
    #[prost(string, tag = "1")]
    pub warranty_id: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetWarrantyRequest {
    #[prost(string, tag = "1")]
    pub warranty_id: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetWarrantyResponse {
    #[prost(message, optional, tag = "1")]
    pub warranty: ::core::option::Option<Warranty>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UpdateWarrantyRequest {
    #[prost(message, optional, tag = "1")]
    pub warranty: ::core::option::Option<Warranty>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UpdateWarrantyResponse {
    #[prost(message, optional, tag = "1")]
    pub warranty: ::core::option::Option<Warranty>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListWarrantiesRequest {
    #[prost(string, tag = "1")]
    pub customer_id: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub product_id: ::prost::alloc::string::String,
    #[prost(string, tag = "3")]
    pub order_id: ::prost::alloc::string::String,
    #[prost(string, tag = "4")]
    pub status: ::prost::alloc::string::String,
    #[prost(message, optional, tag = "5")]
    pub pagination: ::core::option::Option<super::common::Pagination>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListWarrantiesResponse {
    #[prost(message, repeated, tag = "1")]
    pub warranties: ::prost::alloc::vec::Vec<Warranty>,
    #[prost(message, optional, tag = "2")]
    pub pagination: ::core::option::Option<super::common::PaginatedResponse>,
}
/// Generated client implementations.
pub mod warranty_service_client {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    use tonic::codegen::http::Uri;
    #[derive(Debug, Clone)]
    pub struct WarrantyServiceClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl WarrantyServiceClient<tonic::transport::Channel> {
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
    impl<T> WarrantyServiceClient<T>
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
        ) -> WarrantyServiceClient<InterceptedService<T, F>>
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
            WarrantyServiceClient::new(InterceptedService::new(inner, interceptor))
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
        pub async fn create_warranty(
            &mut self,
            request: impl tonic::IntoRequest<super::CreateWarrantyRequest>,
        ) -> Result<tonic::Response<super::CreateWarrantyResponse>, tonic::Status> {
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
                "/stateset.warranty.WarrantyService/CreateWarranty",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
        pub async fn get_warranty(
            &mut self,
            request: impl tonic::IntoRequest<super::GetWarrantyRequest>,
        ) -> Result<tonic::Response<super::GetWarrantyResponse>, tonic::Status> {
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
                "/stateset.warranty.WarrantyService/GetWarranty",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
        pub async fn update_warranty(
            &mut self,
            request: impl tonic::IntoRequest<super::UpdateWarrantyRequest>,
        ) -> Result<tonic::Response<super::UpdateWarrantyResponse>, tonic::Status> {
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
                "/stateset.warranty.WarrantyService/UpdateWarranty",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
        pub async fn list_warranties(
            &mut self,
            request: impl tonic::IntoRequest<super::ListWarrantiesRequest>,
        ) -> Result<tonic::Response<super::ListWarrantiesResponse>, tonic::Status> {
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
                "/stateset.warranty.WarrantyService/ListWarranties",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
    }
}
/// Generated server implementations.
pub mod warranty_service_server {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    /// Generated trait containing gRPC methods that should be implemented for use with WarrantyServiceServer.
    #[async_trait]
    pub trait WarrantyService: Send + Sync + 'static {
        async fn create_warranty(
            &self,
            request: tonic::Request<super::CreateWarrantyRequest>,
        ) -> Result<tonic::Response<super::CreateWarrantyResponse>, tonic::Status>;
        async fn get_warranty(
            &self,
            request: tonic::Request<super::GetWarrantyRequest>,
        ) -> Result<tonic::Response<super::GetWarrantyResponse>, tonic::Status>;
        async fn update_warranty(
            &self,
            request: tonic::Request<super::UpdateWarrantyRequest>,
        ) -> Result<tonic::Response<super::UpdateWarrantyResponse>, tonic::Status>;
        async fn list_warranties(
            &self,
            request: tonic::Request<super::ListWarrantiesRequest>,
        ) -> Result<tonic::Response<super::ListWarrantiesResponse>, tonic::Status>;
    }
    #[derive(Debug)]
    pub struct WarrantyServiceServer<T: WarrantyService> {
        inner: _Inner<T>,
        accept_compression_encodings: EnabledCompressionEncodings,
        send_compression_encodings: EnabledCompressionEncodings,
    }
    struct _Inner<T>(Arc<T>);
    impl<T: WarrantyService> WarrantyServiceServer<T> {
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
    impl<T, B> tonic::codegen::Service<http::Request<B>> for WarrantyServiceServer<T>
    where
        T: WarrantyService,
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
                "/stateset.warranty.WarrantyService/CreateWarranty" => {
                    #[allow(non_camel_case_types)]
                    struct CreateWarrantySvc<T: WarrantyService>(pub Arc<T>);
                    impl<
                        T: WarrantyService,
                    > tonic::server::UnaryService<super::CreateWarrantyRequest>
                    for CreateWarrantySvc<T> {
                        type Response = super::CreateWarrantyResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::CreateWarrantyRequest>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move {
                                (*inner).create_warranty(request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = CreateWarrantySvc(inner);
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
                "/stateset.warranty.WarrantyService/GetWarranty" => {
                    #[allow(non_camel_case_types)]
                    struct GetWarrantySvc<T: WarrantyService>(pub Arc<T>);
                    impl<
                        T: WarrantyService,
                    > tonic::server::UnaryService<super::GetWarrantyRequest>
                    for GetWarrantySvc<T> {
                        type Response = super::GetWarrantyResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::GetWarrantyRequest>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move {
                                (*inner).get_warranty(request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = GetWarrantySvc(inner);
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
                "/stateset.warranty.WarrantyService/UpdateWarranty" => {
                    #[allow(non_camel_case_types)]
                    struct UpdateWarrantySvc<T: WarrantyService>(pub Arc<T>);
                    impl<
                        T: WarrantyService,
                    > tonic::server::UnaryService<super::UpdateWarrantyRequest>
                    for UpdateWarrantySvc<T> {
                        type Response = super::UpdateWarrantyResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::UpdateWarrantyRequest>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move {
                                (*inner).update_warranty(request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = UpdateWarrantySvc(inner);
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
                "/stateset.warranty.WarrantyService/ListWarranties" => {
                    #[allow(non_camel_case_types)]
                    struct ListWarrantiesSvc<T: WarrantyService>(pub Arc<T>);
                    impl<
                        T: WarrantyService,
                    > tonic::server::UnaryService<super::ListWarrantiesRequest>
                    for ListWarrantiesSvc<T> {
                        type Response = super::ListWarrantiesResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::ListWarrantiesRequest>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move {
                                (*inner).list_warranties(request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = ListWarrantiesSvc(inner);
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
    impl<T: WarrantyService> Clone for WarrantyServiceServer<T> {
        fn clone(&self) -> Self {
            let inner = self.inner.clone();
            Self {
                inner,
                accept_compression_encodings: self.accept_compression_encodings,
                send_compression_encodings: self.send_compression_encodings,
            }
        }
    }
    impl<T: WarrantyService> Clone for _Inner<T> {
        fn clone(&self) -> Self {
            Self(self.0.clone())
        }
    }
    impl<T: std::fmt::Debug> std::fmt::Debug for _Inner<T> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{:?}", self.0)
        }
    }
    impl<T: WarrantyService> tonic::server::NamedService for WarrantyServiceServer<T> {
        const NAME: &'static str = "stateset.warranty.WarrantyService";
    }
}
