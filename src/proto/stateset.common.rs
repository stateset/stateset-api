#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Money {
    #[prost(string, tag = "1")]
    pub currency: ::prost::alloc::string::String,
    #[prost(int64, tag = "2")]
    pub amount: i64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Address {
    #[prost(string, tag = "1")]
    pub street_line1: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub street_line2: ::prost::alloc::string::String,
    #[prost(string, tag = "3")]
    pub city: ::prost::alloc::string::String,
    #[prost(string, tag = "4")]
    pub state: ::prost::alloc::string::String,
    #[prost(string, tag = "5")]
    pub country: ::prost::alloc::string::String,
    #[prost(string, tag = "6")]
    pub postal_code: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Pagination {
    #[prost(int32, tag = "1")]
    pub page: i32,
    #[prost(int32, tag = "2")]
    pub page_size: i32,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PaginatedResponse {
    #[prost(int32, tag = "1")]
    pub total_items: i32,
    #[prost(int32, tag = "2")]
    pub total_pages: i32,
    #[prost(int32, tag = "3")]
    pub current_page: i32,
}
