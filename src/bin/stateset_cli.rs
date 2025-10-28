use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
    time::Duration,
};

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use clap::{ArgAction, Args, Parser, Subcommand, ValueEnum};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use stateset_api::{
    auth::{AuthConfig, AuthService, LoginCredentials, TokenPair},
    config::{self, AppConfig},
    db::{self, DbPool},
    entities::{
        commerce::{customer, CustomerAddressModel, ProductModel, ProductVariantModel},
        order_item,
    },
    events::{Event, EventSender},
    services::{
        commerce::{
            customer_service::{
                AddAddressInput, CustomerResponse, CustomerService, RegisterCustomerInput,
            },
            product_catalog_service::{
                CreateProductInput, CreateVariantInput, ProductCatalogService,
                ProductSearchQuery, ProductSearchResult, UpdateProductInput,
            },
        },
        orders::{
            OrderResponse, OrderSearchQuery, OrderService, OrderSortField, SortDirection,
            UpdateOrderStatusRequest,
        },
    },
};
use sea_orm::{ColumnTrait, Condition, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use tokio::sync::mpsc;
use tracing::debug;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let context = CliContext::initialize().await?;

    match cli.command {
        Commands::Auth(command) => handle_auth_command(&context, command, cli.json).await?,
        Commands::Orders(command) => handle_orders_command(&context, command, cli.json).await?,
        Commands::Products(command) => handle_products_command(&context, command, cli.json).await?,
        Commands::Customers(command) => handle_customers_command(&context, command, cli.json).await?,
        Commands::Create(command) => handle_create_command(&context, command, cli.json).await?,
    }

    Ok(())
}

async fn handle_customer_login(
    context: &CliContext,
    args: CustomerLoginArgs,
    json: bool,
) -> Result<()> {
    let service = context.customer_service();
    let credentials = LoginCredentials {
        email: args.email.clone(),
        password: args.password.clone(),
    };

    let response = service
        .login(credentials)
        .await
        .context("failed to authenticate customer")?;

    let saved_path = persist_session(args.save, &args.email, &response.tokens)?;

    if json {
        print_json(&response)?;
    } else {
        println!(
            "Customer {} authenticated (id {})",
            response.customer.email, response.customer.id
        );
        if let Some(path) = saved_path {
            println!("Session saved to: {}", path);
        }
    }

    Ok(())
}

#[derive(Parser)]
#[command(name = "stateset", about = "Stateset CLI for auth and resource management", version)]
struct Cli {
    #[arg(
        long,
        global = true,
        action = ArgAction::SetTrue,
        help = "Render command output as pretty JSON when available"
    )]
    json: bool,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(subcommand)]
    Auth(AuthCommands),
    #[command(subcommand)]
    Orders(OrdersCommands),
    #[command(subcommand)]
    Products(ProductsCommands),
    #[command(subcommand)]
    Customers(CustomersCommands),
    #[command(subcommand)]
    Create(CreateCommands),
}

#[derive(Subcommand)]
enum AuthCommands {
    Login(AuthLoginArgs),
    Refresh(AuthRefreshArgs),
    Whoami(AuthWhoAmIArgs),
    Logout(AuthLogoutArgs),
}

#[derive(Args)]
struct AuthLoginArgs {
    #[arg(long, help = "Email address for the account")]
    email: String,
    #[arg(long, help = "Password for the account")]
    password: String,
    #[arg(
        long,
        action = ArgAction::SetTrue,
        help = "Persist the issued tokens to disk for reuse"
    )]
    save: bool,
}

#[derive(Args)]
struct AuthRefreshArgs {
    #[arg(long, help = "Refresh token to exchange; defaults to saved session")]
    refresh_token: Option<String>,
    #[arg(
        long,
        help = "Email to associate with refreshed session (required if no saved session)"
    )]
    email: Option<String>,
    #[arg(
        long,
        action = ArgAction::SetTrue,
        help = "Persist refreshed tokens to disk"
    )]
    save: bool,
}

#[derive(Args)]
struct AuthWhoAmIArgs {
    #[arg(long, help = "Access token to inspect; defaults to saved session")]
    token: Option<String>,
    #[arg(
        long,
        help = "Also print the stored refresh token details when reading from session",
        action = ArgAction::SetTrue
    )]
    include_refresh: bool,
}

#[derive(Args)]
struct AuthLogoutArgs {
    #[arg(long, help = "Access token to revoke; defaults to saved session")]
    token: Option<String>,
    #[arg(long, help = "Refresh token to revoke; defaults to saved session")]
    refresh_token: Option<String>,
    #[arg(
        long,
        action = ArgAction::SetTrue,
        help = "Also delete the saved session file if present"
    )]
    clear: bool,
}

#[derive(Subcommand)]
enum CreateCommands {
    Order(CreateOrderArgs),
    Product(CreateProductArgs),
    Customer(CreateCustomerArgs),
}

#[derive(Subcommand)]
enum OrdersCommands {
    Create(CreateOrderArgs),
    Get(GetOrderArgs),
    List(ListOrdersArgs),
    Items(OrderItemsArgs),
    AddItem(AddOrderItemArgs),
    UpdateStatus(UpdateOrderStatusArgs),
    Delete(DeleteOrderArgs),
}

#[derive(Subcommand)]
enum ProductsCommands {
    Create(CreateProductArgs),
    Get(GetProductArgs),
    Search(SearchProductsArgs),
    Variants(ProductVariantsArgs),
    Update(UpdateProductArgs),
    CreateVariant(CreateVariantArgs),
}

#[derive(Subcommand)]
enum CustomersCommands {
    Create(CreateCustomerArgs),
    Get(GetCustomerArgs),
    List(ListCustomersArgs),
    Addresses(CustomerAddressesArgs),
    Login(CustomerLoginArgs),
    AddAddress(AddCustomerAddressArgs),
}

#[derive(Args)]
struct CreateOrderArgs {
    #[arg(long, value_parser = clap::value_parser!(Uuid), help = "Customer identifier (UUID)")]
    customer_id: Uuid,
    #[arg(long, help = "Optional free-form notes to attach to the order")]
    notes: Option<String>,
    #[arg(long, help = "Three-letter ISO currency code (defaults to USD)")]
    currency: Option<String>,
    #[arg(long, help = "Shipping address to associate with the order")]
    shipping_address: Option<String>,
    #[arg(long, help = "Billing address to associate with the order")]
    billing_address: Option<String>,
    #[arg(long, help = "Payment method identifier or description")]
    payment_method: Option<String>,
    #[arg(
        long = "item",
        value_parser = parse_order_item,
        action = ArgAction::Append,
        help = "Order line in key=value pairs (e.g. sku=SKU1,quantity=2,price=19.99[,product_id=UUID][,name=Optional][,tax_rate=0.07])"
    )]
    items: Vec<OrderItemInput>,
}

#[derive(Args)]
struct CreateProductArgs {
    #[arg(long, help = "Display name for the product")]
    name: String,
    #[arg(long, help = "Unique SKU for the product")]
    sku: String,
    #[arg(long, value_parser = parse_decimal, help = "Base price for the product")]
    price: Decimal,
    #[arg(long, help = "ISO currency code (defaults to USD)")]
    currency: Option<String>,
    #[arg(long, help = "Optional long-form description")]
    description: Option<String>,
    #[arg(long, help = "Product brand name")]
    brand: Option<String>,
    #[arg(long, help = "Manufacturer name")]
    manufacturer: Option<String>,
    #[arg(long, help = "Public image URL")]
    image_url: Option<String>,
    #[arg(long, help = "Comma-separated product tags")]
    tags: Option<String>,
    #[arg(long, help = "Dimensions in centimeters (free-form text)")]
    dimensions: Option<String>,
    #[arg(long, value_parser = parse_decimal, help = "Weight in kilograms")]
    weight: Option<Decimal>,
    #[arg(long, value_parser = parse_decimal, help = "Internal cost price")]
    cost_price: Option<Decimal>,
    #[arg(long, value_parser = parse_decimal, help = "Manufacturer suggested retail price")]
    msrp: Option<Decimal>,
    #[arg(long, value_parser = parse_decimal, help = "Tax rate expressed as decimal (e.g. 0.07)")]
    tax_rate: Option<Decimal>,
    #[arg(long, help = "Optional SEO meta title")]
    meta_title: Option<String>,
    #[arg(long, help = "Optional SEO meta description")]
    meta_description: Option<String>,
    #[arg(long, help = "Reorder point threshold")]
    reorder_point: Option<i32>,
    #[arg(long, action = ArgAction::SetTrue, help = "Mark the product as digital (no physical fulfillment)")]
    digital: bool,
    #[arg(long, action = ArgAction::SetTrue, help = "Create the product as inactive")]
    inactive: bool,
}

#[derive(Args)]
struct CreateCustomerArgs {
    #[arg(long, help = "Customer email address")]
    email: String,
    #[arg(long, help = "Customer password")]
    password: String,
    #[arg(long, help = "Customer first name")]
    first_name: String,
    #[arg(long, help = "Customer last name")]
    last_name: String,
    #[arg(long, help = "Optional phone number")]
    phone: Option<String>,
    #[arg(
        long,
        action = ArgAction::SetTrue,
        help = "Flag the customer as accepting marketing messages"
    )]
    accepts_marketing: bool,
}

#[derive(Args)]
struct GetOrderArgs {
    #[arg(long, value_parser = clap::value_parser!(Uuid), help = "Order identifier")]
    id: Uuid,
}

#[derive(Args)]
struct OrderItemsArgs {
    #[arg(long, value_parser = clap::value_parser!(Uuid), help = "Order identifier")]
    id: Uuid,
}

#[derive(Args)]
struct AddOrderItemArgs {
    #[arg(long, value_parser = clap::value_parser!(Uuid), help = "Order identifier")]
    order_id: Uuid,
    #[arg(long, help = "SKU to attach to the order line")]
    sku: String,
    #[arg(long, value_parser = parse_decimal, help = "Unit price for the order line")]
    price: Decimal,
    #[arg(long, help = "Quantity for the order line")]
    quantity: i32,
    #[arg(long, help = "Optional human readable item name")]
    name: Option<String>,
    #[arg(long, value_parser = clap::value_parser!(Uuid), help = "Product identifier to associate")]
    product_id: Option<Uuid>,
    #[arg(long, value_parser = parse_decimal, help = "Tax rate expressed as decimal (e.g. 0.07)")]
    tax_rate: Option<Decimal>,
}

#[derive(Clone, Copy, ValueEnum)]
enum OrderSortFieldArg {
    CreatedAt,
    OrderDate,
    TotalAmount,
    OrderNumber,
}

impl From<OrderSortFieldArg> for OrderSortField {
    fn from(value: OrderSortFieldArg) -> Self {
        match value {
            OrderSortFieldArg::CreatedAt => OrderSortField::CreatedAt,
            OrderSortFieldArg::OrderDate => OrderSortField::OrderDate,
            OrderSortFieldArg::TotalAmount => OrderSortField::TotalAmount,
            OrderSortFieldArg::OrderNumber => OrderSortField::OrderNumber,
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum SortDirectionArg {
    Asc,
    Desc,
}

impl From<SortDirectionArg> for SortDirection {
    fn from(value: SortDirectionArg) -> Self {
        match value {
            SortDirectionArg::Asc => SortDirection::Asc,
            SortDirectionArg::Desc => SortDirection::Desc,
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum OrderStatusArg {
    Pending,
    Processing,
    Shipped,
    Delivered,
    Cancelled,
    Refunded,
}

impl From<OrderStatusArg> for String {
    fn from(value: OrderStatusArg) -> Self {
        match value {
            OrderStatusArg::Pending => "pending",
            OrderStatusArg::Processing => "processing",
            OrderStatusArg::Shipped => "shipped",
            OrderStatusArg::Delivered => "delivered",
            OrderStatusArg::Cancelled => "cancelled",
            OrderStatusArg::Refunded => "refunded",
        }
        .to_string()
    }
}

#[derive(Args)]
struct ListOrdersArgs {
    #[arg(long, default_value_t = 1, help = "Page number (1-indexed)")]
    page: u64,
    #[arg(long, default_value_t = 25, help = "Items per page", value_parser = parse_positive_u64)]
    per_page: u64,
    #[arg(long, value_parser = clap::value_parser!(Uuid), help = "Filter by customer identifier")]
    customer_id: Option<Uuid>,
    #[arg(long, help = "Filter by status (exact match)")]
    status: Option<String>,
    #[arg(long, help = "Search term to match order number or notes")]
    search: Option<String>,
    #[arg(
        long,
        value_parser = parse_datetime,
        help = "Filter orders created after this RFC3339 timestamp"
    )]
    from: Option<DateTime<Utc>>,
    #[arg(
        long,
        value_parser = parse_datetime,
        help = "Filter orders created before this RFC3339 timestamp"
    )]
    to: Option<DateTime<Utc>>,
    #[arg(long, value_enum, default_value_t = OrderSortFieldArg::CreatedAt)]
    sort: OrderSortFieldArg,
    #[arg(long, value_enum, default_value_t = SortDirectionArg::Desc)]
    direction: SortDirectionArg,
}

#[derive(Args)]
struct GetProductArgs {
    #[arg(long, value_parser = clap::value_parser!(Uuid), help = "Product identifier")]
    id: Uuid,
}

#[derive(Args)]
struct SearchProductsArgs {
    #[arg(long, help = "Search string to match against name and SKU")]
    query: Option<String>,
    #[arg(long, help = "Return only active products", default_value_t = true)]
    only_active: bool,
    #[arg(long, help = "Maximum number of results", default_value_t = 25)]
    limit: u64,
    #[arg(long, help = "Offset into the result set", default_value_t = 0)]
    offset: u64,
}

#[derive(Args)]
struct ProductVariantsArgs {
    #[arg(long, value_parser = clap::value_parser!(Uuid), help = "Product identifier")]
    product_id: Uuid,
}

#[derive(Args)]
struct GetCustomerArgs {
    #[arg(long, value_parser = clap::value_parser!(Uuid), help = "Customer identifier")]
    id: Uuid,
}

#[derive(Args)]
struct ListCustomersArgs {
    #[arg(long, help = "Optional search term to match name or email")]
    search: Option<String>,
    #[arg(long, help = "Maximum number of customers to return", default_value_t = 25)]
    limit: u64,
    #[arg(long, help = "Offset for pagination", default_value_t = 0)]
    offset: u64,
}

#[derive(Args)]
struct CustomerAddressesArgs {
    #[arg(long, value_parser = clap::value_parser!(Uuid), help = "Customer identifier")]
    id: Uuid,
}

#[derive(Args)]
struct CustomerLoginArgs {
    #[arg(long, help = "Customer email address")]
    email: String,
    #[arg(long, help = "Customer password")]
    password: String,
    #[arg(
        long,
        action = ArgAction::SetTrue,
        help = "Persist the issued tokens to disk"
    )]
    save: bool,
}

#[derive(Args)]
struct UpdateOrderStatusArgs {
    #[arg(long, value_parser = clap::value_parser!(Uuid), help = "Order identifier")]
    order_id: Uuid,
    #[arg(long, value_enum, help = "New status value")]
    status: OrderStatusArg,
    #[arg(long, help = "Optional notes to include with the status change")]
    notes: Option<String>,
}

#[derive(Args)]
struct DeleteOrderArgs {
    #[arg(long, value_parser = clap::value_parser!(Uuid), help = "Order identifier")]
    order_id: Uuid,
}

#[derive(Args)]
struct UpdateProductArgs {
    #[arg(long, value_parser = clap::value_parser!(Uuid), help = "Product identifier")]
    id: Uuid,
    #[arg(long, help = "New product name")]
    name: Option<String>,
    #[arg(long, help = "New SKU value")]
    sku: Option<String>,
    #[arg(long, help = "Updated description")]
    description: Option<String>,
    #[arg(long, value_parser = parse_decimal, help = "Updated price")]
    price: Option<Decimal>,
    #[arg(long, help = "Updated currency code")]
    currency: Option<String>,
    #[arg(long, value_parser = parse_decimal, help = "Updated cost price")]
    cost_price: Option<Decimal>,
    #[arg(long, value_parser = parse_decimal, help = "Updated MSRP")]
    msrp: Option<Decimal>,
    #[arg(long, value_parser = parse_decimal, help = "Updated tax rate")]
    tax_rate: Option<Decimal>,
    #[arg(long, help = "Updated brand")]
    brand: Option<String>,
    #[arg(long, help = "Updated manufacturer")]
    manufacturer: Option<String>,
    #[arg(long, help = "Updated image URL")]
    image_url: Option<String>,
    #[arg(long, help = "Updated tags")]
    tags: Option<String>,
    #[arg(long, help = "Updated meta title")]
    meta_title: Option<String>,
    #[arg(long, help = "Updated meta description")]
    meta_description: Option<String>,
    #[arg(long, help = "Updated dimensions")]
    dimensions: Option<String>,
    #[arg(long, value_parser = parse_decimal, help = "Updated weight in kilograms")]
    weight_kg: Option<Decimal>,
    #[arg(long, help = "Set active state", action = ArgAction::SetTrue)]
    activate: bool,
    #[arg(long, help = "Set inactive state", action = ArgAction::SetTrue)]
    deactivate: bool,
    #[arg(long, help = "Mark product as digital", action = ArgAction::SetTrue)]
    digital: bool,
    #[arg(long, help = "Mark product as physical", action = ArgAction::SetTrue)]
    physical: bool,
    #[arg(long, help = "Updated reorder point")]
    reorder_point: Option<i32>,
}

#[derive(Args)]
struct CreateVariantArgs {
    #[arg(long, value_parser = clap::value_parser!(Uuid), help = "Parent product identifier")]
    product_id: Uuid,
    #[arg(long, help = "Variant SKU")]
    sku: String,
    #[arg(long, help = "Variant display name")]
    name: String,
    #[arg(long, value_parser = parse_decimal, help = "Variant price")]
    price: Decimal,
    #[arg(long, value_parser = parse_decimal, help = "Compare-at price")]
    compare_at_price: Option<Decimal>,
    #[arg(long, value_parser = parse_decimal, help = "Cost value")]
    cost: Option<Decimal>,
    #[arg(long, help = "Weight in kilograms")]
    weight: Option<f64>,
    #[arg(long, help = "Variant position", default_value_t = 1)]
    position: i32,
    #[arg(
        long = "option",
        value_parser = parse_key_value,
        action = ArgAction::Append,
        help = "Variant option key/value pair (repeatable)"
    )]
    options: Vec<(String, String)>,
    #[arg(
        long = "no-inventory-tracking",
        default_value_t = true,
        action = ArgAction::SetFalse,
        help = "Disable inventory tracking for this variant"
    )]
    inventory_tracking: bool,
}

#[derive(Args)]
struct AddCustomerAddressArgs {
    #[arg(long, value_parser = clap::value_parser!(Uuid), help = "Customer identifier")]
    customer_id: Uuid,
    #[arg(long, help = "First name for the recipient")]
    first_name: String,
    #[arg(long, help = "Last name for the recipient")]
    last_name: String,
    #[arg(long, help = "Company name")]
    company: Option<String>,
    #[arg(long, help = "Address line 1")]
    address_line_1: String,
    #[arg(long, help = "Address line 2")]
    address_line_2: Option<String>,
    #[arg(long, help = "City name")]
    city: String,
    #[arg(long, help = "Province or state")]
    province: String,
    #[arg(long, help = "Country code")]
    country_code: String,
    #[arg(long, help = "Postal code")]
    postal_code: String,
    #[arg(long, help = "Phone number")]
    phone: Option<String>,
    #[arg(long, action = ArgAction::SetTrue, help = "Mark as default shipping address")]
    default_shipping: bool,
    #[arg(long, action = ArgAction::SetTrue, help = "Mark as default billing address")]
    default_billing: bool,
}
#[derive(Debug, Clone)]
struct OrderItemInput {
    sku: String,
    product_id: Option<Uuid>,
    name: Option<String>,
    quantity: i32,
    unit_price: Decimal,
    tax_rate: Option<Decimal>,
}

#[derive(Serialize)]
struct AuthLoginOutput {
    user_id: Uuid,
    email: String,
    access_token: String,
    refresh_token: String,
    token_type: String,
    expires_in: i64,
    refresh_expires_in: i64,
    saved_session_path: Option<String>,
}

#[derive(Serialize)]
struct OrderCreationOutput {
    order: OrderResponse,
    items: Vec<order_item::Model>,
}

#[derive(Serialize, Deserialize)]
struct StoredSession {
    email: String,
    access_token: String,
    refresh_token: String,
    token_type: String,
    expires_in: i64,
    refresh_expires_in: i64,
    saved_at: DateTime<Utc>,
}

struct CliContext {
    _config: AppConfig,
    db: Arc<DbPool>,
    event_sender: Arc<EventSender>,
    auth_service: Arc<AuthService>,
}

impl CliContext {
    async fn initialize() -> Result<Self> {
        let config = config::load_config().context("failed to load application config")?;
        config::init_tracing(config.log_level(), config.log_json);

        let db_pool = db::establish_connection_from_app_config(&config)
            .await
            .context("failed to connect to database")?;
        let db = Arc::new(db_pool);

        let auth_config = AuthConfig::new(
            config.jwt_secret.clone(),
            "stateset-api".to_string(),
            "stateset-auth".to_string(),
            Duration::from_secs(config.jwt_expiration as u64),
            Duration::from_secs(config.refresh_token_expiration as u64),
            "sk_".to_string(),
        );

        let auth_service = Arc::new(AuthService::new(auth_config, db.clone()));

        let (event_tx, mut event_rx) = mpsc::channel::<Event>(32);
        let event_sender = Arc::new(EventSender::new(event_tx));

        tokio::spawn(async move {
            while let Some(event) = event_rx.recv().await {
                debug!(target: "stateset_cli", event = ?event, "received async event");
            }
        });

        Ok(Self {
            _config: config,
            db,
            event_sender,
            auth_service,
        })
    }

    fn product_service(&self) -> ProductCatalogService {
        ProductCatalogService::new(self.db.clone(), self.event_sender.clone())
    }

    fn customer_service(&self) -> CustomerService {
        CustomerService::new(
            self.db.clone(),
            self.event_sender.clone(),
            self.auth_service.clone(),
        )
    }

    fn order_service(&self) -> OrderService {
        OrderService::new(self.db.clone(), Some(self.event_sender.clone()))
    }
}

async fn handle_auth_command(context: &CliContext, command: AuthCommands, json: bool) -> Result<()> {
    match command {
        AuthCommands::Login(args) => handle_auth_login(context, args, json).await,
        AuthCommands::Refresh(args) => handle_auth_refresh(context, args, json).await,
        AuthCommands::Whoami(args) => handle_auth_whoami(context, args, json).await,
        AuthCommands::Logout(args) => handle_auth_logout(context, args).await,
    }
}

async fn handle_create_command(
    context: &CliContext,
    command: CreateCommands,
    json: bool,
) -> Result<()> {
    match command {
        CreateCommands::Order(args) => handle_create_order(context, args, json).await,
        CreateCommands::Product(args) => handle_create_product(context, args, json).await,
        CreateCommands::Customer(args) => handle_create_customer(context, args, json).await,
    }
}

async fn handle_orders_command(
    context: &CliContext,
    command: OrdersCommands,
    json: bool,
) -> Result<()> {
    let service = context.order_service();
    match command {
        OrdersCommands::Create(args) => handle_create_order(context, args, json).await,
        OrdersCommands::Get(args) => {
            let order = service
                .get_order(args.id)
                .await
                .with_context(|| format!("failed to fetch order {}", args.id))?
                .ok_or_else(|| anyhow!("order {} not found", args.id))?;
            if json {
                print_json(&order)?;
            } else {
                render_order(&order);
            }
            Ok(())
        }
        OrdersCommands::List(args) => {
            let response = service
                .search_orders(OrderSearchQuery {
                    customer_id: args.customer_id,
                    status: args.status.clone(),
                    from_date: args.from,
                    to_date: args.to,
                    search: args.search.clone(),
                    sort_field: args.sort.into(),
                    sort_direction: args.direction.into(),
                    page: args.page,
                    per_page: args.per_page,
                })
                .await
                .context("failed to list orders")?;
            if json {
                print_json(&response)?;
            } else {
                println!(
                    "Orders page {} ({} per page) total {}",
                    response.page, response.per_page, response.total
                );
                for order in &response.orders {
                    render_order(order);
                }
            }
            Ok(())
        }
        OrdersCommands::Items(args) => {
            let items = service
                .get_order_items(args.id)
                .await
                .with_context(|| format!("failed to fetch items for order {}", args.id))?;
            if json {
                print_json(&items)?;
            } else if items.is_empty() {
                println!("Order {} has no items", args.id);
            } else {
                println!("Order {} items ({} total):", args.id, items.len());
                for item in &items {
                    render_order_item(item);
                }
            }
            Ok(())
        }
        OrdersCommands::AddItem(args) => {
            if args.quantity <= 0 {
                return Err(anyhow!("quantity must be positive"));
            }
            let saved = service
                .add_order_item(
                    args.order_id,
                    args.sku.clone(),
                    args.product_id,
                    args.name.clone(),
                    args.quantity,
                    args.price,
                    args.tax_rate,
                )
                .await
                .with_context(|| format!("failed to add item to order {}", args.order_id))?;
            if json {
                print_json(&saved)?;
            } else {
                println!(
                    "Added {} x {} to order {} (line id {})",
                    saved.quantity, saved.sku, args.order_id, saved.id
                );
            }
            Ok(())
        }
        OrdersCommands::UpdateStatus(args) => {
            let request = UpdateOrderStatusRequest {
                status: args.status.into(),
                notes: args.notes.clone(),
            };
            let updated = service
                .update_order_status(args.order_id, request)
                .await
                .with_context(|| format!("failed to update status for order {}", args.order_id))?;
            if json {
                print_json(&updated)?;
            } else {
                println!(
                    "Updated order {} status to {}",
                    updated.id, updated.status
                );
                render_order(&updated);
            }
            Ok(())
        }
        OrdersCommands::Delete(args) => {
            service
                .delete_order(args.order_id)
                .await
                .with_context(|| format!("failed to delete order {}", args.order_id))?;
            if json {
                print_json(&serde_json::json!({
                    "order_id": args.order_id,
                    "status": "deleted"
                }))?;
            } else {
                println!("Order {} archived (delete)", args.order_id);
            }
            Ok(())
        }
    }
}

async fn handle_products_command(
    context: &CliContext,
    command: ProductsCommands,
    json: bool,
) -> Result<()> {
    let service = context.product_service();
    match command {
        ProductsCommands::Create(args) => handle_create_product(context, args, json).await,
        ProductsCommands::Get(args) => {
            let product = service
                .get_product(args.id)
                .await
                .with_context(|| format!("failed to fetch product {}", args.id))?;
            if json {
                print_json(&product)?;
            } else {
                render_product(&product);
            }
            Ok(())
        }
        ProductsCommands::Search(args) => {
            let result = service
                .search_products(ProductSearchQuery {
                    search: args.query.clone(),
                    is_active: if args.only_active { Some(true) } else { None },
                    limit: Some(args.limit),
                    offset: Some(args.offset),
                })
                .await
                .context("failed to search products")?;
            if json {
                print_json(&result)?;
            } else {
                render_product_search(&result);
            }
            Ok(())
        }
        ProductsCommands::Variants(args) => {
            let variants = service
                .get_product_variants(args.product_id)
                .await
                .with_context(|| format!("failed to fetch variants for product {}", args.product_id))?;
            if json {
                print_json(&variants)?;
            } else if variants.is_empty() {
                println!("Product {} has no variants", args.product_id);
            } else {
                println!(
                    "Product {} variants ({} total):",
                    args.product_id,
                    variants.len()
                );
                for variant in &variants {
                    render_variant(variant);
                }
            }
            Ok(())
        }
        ProductsCommands::Update(args) => handle_update_product(context, args, json).await,
        ProductsCommands::CreateVariant(args) => {
            let variant = handle_create_variant(context, args).await?;
            if json {
                print_json(&variant)?;
            } else {
                println!(
                    "Created variant {} (SKU {}) for product {}",
                    variant.id, variant.sku, variant.product_id
                );
            }
            Ok(())
        }
    }
}

async fn handle_customers_command(
    context: &CliContext,
    command: CustomersCommands,
    json: bool,
) -> Result<()> {
    let service = context.customer_service();
    match command {
        CustomersCommands::Create(args) => handle_create_customer(context, args, json).await,
        CustomersCommands::Get(args) => {
            let customer = service
                .get_customer(args.id)
                .await
                .with_context(|| format!("failed to fetch customer {}", args.id))?;
            let response: CustomerResponse = customer.into();
            if json {
                print_json(&response)?;
            } else {
                render_customer(&response);
            }
            Ok(())
        }
        CustomersCommands::List(args) => {
            let customers = list_customers(&context.db, &args).await?;
            if json {
                print_json(&customers)?;
            } else if customers.is_empty() {
                println!("No customers matched the provided filters.");
            } else {
                println!("Customers ({} total rows)", customers.len());
                for customer in &customers {
                    render_customer(customer);
                }
            }
            Ok(())
        }
        CustomersCommands::Addresses(args) => {
            let addresses = service
                .get_addresses(args.id)
                .await
                .with_context(|| format!("failed to fetch addresses for customer {}", args.id))?;
            if json {
                print_json(&addresses)?;
            } else if addresses.is_empty() {
                println!("Customer {} has no saved addresses", args.id);
            } else {
                println!("Customer {} addresses:", args.id);
                for address in &addresses {
                    render_address(address);
                }
            }
            Ok(())
        }
        CustomersCommands::Login(args) => handle_customer_login(context, args, json).await,
        CustomersCommands::AddAddress(args) => {
            let address = handle_add_customer_address(context, args).await?;
            if json {
                print_json(&address)?;
            } else {
                println!("Added address {} to customer {}", address.id, address.customer_id);
            }
            Ok(())
        }
    }
}

async fn handle_auth_login(context: &CliContext, args: AuthLoginArgs, json: bool) -> Result<()> {
    let user = context
        .auth_service
        .authenticate_user(&args.email, &args.password)
        .await
        .map_err(|err| anyhow!("authentication failed: {}", err))?;

    let tokens = context
        .auth_service
        .generate_token(&user)
        .await
        .map_err(|err| anyhow!("failed to generate tokens: {}", err))?;

    let saved_path = persist_session(args.save, &args.email, &tokens)?;

    if json {
        let output = AuthLoginOutput {
            user_id: user.id,
            email: user.email,
            access_token: tokens.access_token,
            refresh_token: tokens.refresh_token,
            token_type: tokens.token_type,
            expires_in: tokens.expires_in,
            refresh_expires_in: tokens.refresh_expires_in,
            saved_session_path: saved_path.clone(),
        };
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!("Authenticated user: {}", args.email);
        println!("Access Token: {}", tokens.access_token);
        println!("Refresh Token: {}", tokens.refresh_token);
        println!("Token Type: {}", tokens.token_type);
        println!("Expires In: {} seconds", tokens.expires_in);
        if let Some(path) = saved_path {
            println!("Session saved to: {}", path);
        }
    }

    Ok(())
}

async fn handle_auth_refresh(
    context: &CliContext,
    args: AuthRefreshArgs,
    json: bool,
) -> Result<()> {
    let session = read_session()?;
    let refresh_token = if let Some(token) = args.refresh_token {
        token
    } else if let Some((_, stored)) = &session {
        stored.refresh_token.clone()
    } else {
        return Err(anyhow!(
            "no refresh token provided and no saved session found; supply --refresh-token"
        ));
    };

    let tokens = context
        .auth_service
        .refresh_token(&refresh_token)
        .await
        .context("failed to refresh token")?;

    let email = args
        .email
        .or_else(|| session.as_ref().map(|(_, s)| s.email.clone()))
        .ok_or_else(|| anyhow!("email is required when refreshing without a saved session"))?;

    let saved_path = persist_session(args.save, &email, &tokens)?;

    if json {
        let output = AuthLoginOutput {
            user_id: Uuid::nil(),
            email,
            access_token: tokens.access_token,
            refresh_token: tokens.refresh_token,
            token_type: tokens.token_type,
            expires_in: tokens.expires_in,
            refresh_expires_in: tokens.refresh_expires_in,
            saved_session_path: saved_path.clone(),
        };
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!("Issued new access token (expires in {}s)", tokens.expires_in);
        if let Some(path) = saved_path {
            println!("Session saved to: {}", path);
        }
    }

    Ok(())
}

async fn handle_auth_whoami(
    context: &CliContext,
    args: AuthWhoAmIArgs,
    json: bool,
) -> Result<()> {
    let session = read_session()?;
    let token = if let Some(token) = args.token {
        token
    } else if let Some((_, stored)) = &session {
        stored.access_token.clone()
    } else {
        return Err(anyhow!(
            "no access token provided and no saved session found; supply --token"
        ));
    };

    let claims = context
        .auth_service
        .validate_token(&token)
        .await
        .context("failed to validate token")?;

    if json {
        print_json(&claims)?;
    } else {
        println!("Subject: {}", claims.sub);
        if let Some(email) = claims.email.as_ref() {
            println!("Email: {}", email);
        }
        if !claims.roles.is_empty() {
            println!("Roles: {}", claims.roles.join(", "));
        }
        if !claims.permissions.is_empty() {
            println!("Permissions: {}", claims.permissions.join(", "));
        }
        println!(
            "Issued at: {}",
            timestamp_to_utc(claims.iat).unwrap_or_else(|| "unknown".to_string())
        );
        println!(
            "Expires at: {}",
            timestamp_to_utc(claims.exp).unwrap_or_else(|| "unknown".to_string())
        );
        if let Some((path, stored)) = &session {
            println!("Loaded from session: {}", path.display());
            if args.include_refresh {
                println!(
                    "Stored refresh token expires in {} seconds",
                    stored.refresh_expires_in
                );
            }
        }
    }

    Ok(())
}

async fn handle_auth_logout(context: &CliContext, args: AuthLogoutArgs) -> Result<()> {
    let session = read_session()?;
    let access_token = match (args.token, &session) {
        (Some(token), _) => Some(token),
        (None, Some((_, stored))) => Some(stored.access_token.clone()),
        (None, None) => None,
    };

    if let Some(token) = access_token {
        context
            .auth_service
            .revoke_token(&token)
            .await
            .context("failed to revoke access token")?;
    }

    let refresh_token = match (args.refresh_token, &session) {
        (Some(token), _) => Some(token),
        (None, Some((_, stored))) => Some(stored.refresh_token.clone()),
        (None, None) => None,
    };

    if let Some(token) = refresh_token {
        context
            .auth_service
            .revoke_token(&token)
            .await
            .context("failed to revoke refresh token")?;
    }

    if args.clear {
        if let Some((path, _)) = session {
            if let Err(err) = clear_session_file(&path) {
                eprintln!("Failed to remove session file {}: {}", path.display(), err);
            } else {
                println!("Cleared session file {}", path.display());
            }
        }
    }

    println!("Tokens revoked successfully.");
    Ok(())
}

async fn handle_create_customer(
    context: &CliContext,
    args: CreateCustomerArgs,
    json: bool,
) -> Result<()> {
    let service = context.customer_service();
    let input = RegisterCustomerInput {
        email: args.email,
        password: args.password,
        first_name: args.first_name,
        last_name: args.last_name,
        phone: args.phone,
        accepts_marketing: args.accepts_marketing,
    };

    let customer = service
        .register_customer(input)
        .await
        .context("failed to register customer")?;

    if json {
        println!("{}", serde_json::to_string_pretty(&customer)?);
    } else {
        println!(
            "Created customer {} ({})",
            customer.id,
            customer.email
        );
    }

    Ok(())
}

async fn handle_create_product(
    context: &CliContext,
    args: CreateProductArgs,
    json: bool,
) -> Result<()> {
    let service = context.product_service();
    let currency = args
        .currency
        .map(|c| c.trim().to_ascii_uppercase())
        .filter(|c| !c.is_empty())
        .unwrap_or_else(|| "USD".to_string());

    let input = CreateProductInput {
        name: args.name,
        sku: args.sku,
        description: args.description,
        price: args.price,
        currency,
        is_active: !args.inactive,
        is_digital: args.digital,
        image_url: args.image_url,
        brand: args.brand,
        manufacturer: args.manufacturer,
        weight_kg: args.weight,
        dimensions_cm: args.dimensions,
        tags: args.tags,
        cost_price: args.cost_price,
        msrp: args.msrp,
        tax_rate: args.tax_rate,
        meta_title: args.meta_title,
        meta_description: args.meta_description,
        reorder_point: args.reorder_point,
    };

    let product = service
        .create_product(input)
        .await
        .context("failed to create product")?;

    if json {
        println!("{}", serde_json::to_string_pretty(&product)?);
    } else {
        println!("Created product {} ({})", product.id, product.name);
    }

    Ok(())
}

async fn handle_update_product(
    context: &CliContext,
    args: UpdateProductArgs,
    json: bool,
) -> Result<()> {
    if args.activate && args.deactivate {
        return Err(anyhow!(
            "Cannot use --activate and --deactivate simultaneously"
        ));
    }
    if args.digital && args.physical {
        return Err(anyhow!(
            "Cannot use --digital and --physical simultaneously"
        ));
    }

    let service = context.product_service();
    let mut input = UpdateProductInput::default();

    if let Some(name) = normalize_optional_string(args.name) {
        input.name = Some(name);
    }
    if let Some(sku) = normalize_optional_string(args.sku) {
        input.sku = Some(sku);
    }
    if let Some(description) = normalize_optional_string(args.description) {
        input.description = Some(description);
    }
    if let Some(price) = args.price {
        input.price = Some(price);
    }
    if let Some(currency) = normalize_optional_string(args.currency) {
        input.currency = Some(currency.to_ascii_uppercase());
    }
    if let Some(cost) = args.cost_price {
        input.cost_price = Some(cost);
    }
    if let Some(msrp) = args.msrp {
        input.msrp = Some(msrp);
    }
    if let Some(tax_rate) = args.tax_rate {
        input.tax_rate = Some(tax_rate);
    }
    if let Some(brand) = normalize_optional_string(args.brand) {
        input.brand = Some(brand);
    }
    if let Some(manufacturer) = normalize_optional_string(args.manufacturer) {
        input.manufacturer = Some(manufacturer);
    }
    if let Some(image_url) = normalize_optional_string(args.image_url) {
        input.image_url = Some(image_url);
    }
    if let Some(tags) = normalize_optional_string(args.tags) {
        input.tags = Some(tags);
    }
    if let Some(meta_title) = normalize_optional_string(args.meta_title) {
        input.meta_title = Some(meta_title);
    }
    if let Some(meta_description) = normalize_optional_string(args.meta_description) {
        input.meta_description = Some(meta_description);
    }
    if let Some(dimensions) = normalize_optional_string(args.dimensions) {
        input.dimensions_cm = Some(dimensions);
    }
    if let Some(weight) = args.weight_kg {
        input.weight_kg = Some(weight);
    }
    if let Some(reorder_point) = args.reorder_point {
        input.reorder_point = Some(reorder_point);
    }
    if args.activate {
        input.is_active = Some(true);
    } else if args.deactivate {
        input.is_active = Some(false);
    }
    if args.digital {
        input.is_digital = Some(true);
    } else if args.physical {
        input.is_digital = Some(false);
    }

    let updated = service
        .update_product(args.id, input)
        .await
        .with_context(|| format!("failed to update product {}", args.id))?;

    if json {
        print_json(&updated)?;
    } else {
        println!("Updated product {} ({})", updated.id, updated.name);
    }
    Ok(())
}

async fn handle_create_order(
    context: &CliContext,
    args: CreateOrderArgs,
    json: bool,
) -> Result<()> {
    if args.items.is_empty() {
        return Err(anyhow!(
            "at least one --item argument is required. \
             Format: sku=SKU,quantity=1,price=9.99[,product_id=UUID][,name=Label][,tax_rate=0.07]"
        ));
    }

    // Confirm the customer exists for clearer errors.
    let customer_service = context.customer_service();
    customer_service
        .get_customer(args.customer_id)
        .await
        .with_context(|| format!("failed to locate customer {}", args.customer_id))?;

    let order_service = context.order_service();

    let total_amount = args
        .items
        .iter()
        .fold(Decimal::ZERO, |acc, item| {
            acc + item.unit_price * Decimal::from(item.quantity)
        });

    let currency = args
        .currency
        .map(|c| c.trim().to_ascii_uppercase())
        .filter(|c| !c.is_empty());

    let order = order_service
        .create_order_minimal(
            args.customer_id,
            total_amount,
            currency,
            args.notes.clone(),
            args.shipping_address.clone(),
            args.billing_address.clone(),
            args.payment_method.clone(),
        )
        .await
        .context("failed to create order")?;

    let mut created_items = Vec::new();
    for item in &args.items {
        let saved = order_service
            .add_order_item(
                order.id,
                item.sku.clone(),
                item.product_id,
                item.name.clone(),
                item.quantity,
                item.unit_price,
                item.tax_rate,
            )
            .await
            .with_context(|| format!("failed to add item {}", item.sku))?;
        created_items.push(saved);
    }

    if json {
        let output = OrderCreationOutput {
            order,
            items: created_items,
        };
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!("Created order {}", order.id);
        println!(
            "Total amount: {} {} ({} items)",
            order.total_amount,
            order.currency,
            created_items.len()
        );
    }

    Ok(())
}

async fn handle_create_variant(
    context: &CliContext,
    args: CreateVariantArgs,
) -> Result<ProductVariantModel> {
    let service = context.product_service();
    let options_map: HashMap<String, String> = args
        .options
        .into_iter()
        .map(|(k, v)| (k, v))
        .collect();

    let input = CreateVariantInput {
        product_id: args.product_id,
        sku: normalize_string(args.sku),
        name: normalize_string(args.name),
        price: args.price,
        compare_at_price: args.compare_at_price,
        cost: args.cost,
        weight: args.weight,
        dimensions: None,
        options: options_map,
        inventory_tracking: args.inventory_tracking,
        position: args.position,
    };

    service
        .create_variant(input)
        .await
        .context("failed to create variant")
}

async fn handle_add_customer_address(
    context: &CliContext,
    args: AddCustomerAddressArgs,
) -> Result<CustomerAddressModel> {
    let service = context.customer_service();
    let input = AddAddressInput {
        first_name: normalize_string(args.first_name),
        last_name: normalize_string(args.last_name),
        company: normalize_optional_string(args.company),
        address_line_1: normalize_string(args.address_line_1),
        address_line_2: normalize_optional_string(args.address_line_2),
        city: normalize_string(args.city),
        province: normalize_string(args.province),
        country_code: normalize_string(args.country_code).to_ascii_uppercase(),
        postal_code: normalize_string(args.postal_code),
        phone: normalize_optional_string(args.phone),
        is_default_shipping: if args.default_shipping {
            Some(true)
        } else {
            None
        },
        is_default_billing: if args.default_billing {
            Some(true)
        } else {
            None
        },
    };

    service
        .add_address(args.customer_id, input)
        .await
        .with_context(|| format!("failed to add address for customer {}", args.customer_id))
}

fn print_json<T: Serialize>(value: &T) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

fn render_order(order: &OrderResponse) {
    println!(
        "- Order {} • customer {} • status {} • total {} {}",
        order.id, order.customer_id, order.status, order.total_amount, order.currency
    );
}

fn render_order_item(item: &order_item::Model) {
    println!(
        "  • {} x {} @ {} (total {})",
        item.quantity, item.sku, item.unit_price, item.total_price
    );
}

fn render_product(product: &ProductModel) {
    println!(
        "- Product {} • {} • SKU {} • price {} {}",
        product.id, product.name, product.sku, product.price, product.currency
    );
}

fn render_product_search(result: &ProductSearchResult) {
    println!(
        "Products {} result(s) (total {})",
        result.products.len(),
        result.total
    );
    for product in &result.products {
        render_product(product);
    }
}

fn render_variant(variant: &ProductVariantModel) {
    println!(
        "  • Variant {} • SKU {} • {} @ {}",
        variant.id, variant.sku, variant.name, variant.price
    );
}

fn render_customer(customer: &CustomerResponse) {
    println!(
        "- Customer {} • {} {} • {} • status {:?}",
        customer.id, customer.first_name, customer.last_name, customer.email, customer.status
    );
}

fn render_address(address: &CustomerAddressModel) {
    let name = address
        .name
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or("Unnamed");
    let company = address
        .company
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(|c| format!(" ({c})"))
        .unwrap_or_default();
    let mut flags = Vec::new();
    if address.is_default_shipping {
        flags.push("default shipping");
    }
    if address.is_default_billing {
        flags.push("default billing");
    }
    let flag_suffix = if flags.is_empty() {
        String::new()
    } else {
        format!(" [{}]", flags.join(", "))
    };
    println!(
        "  • {}{} | {}, {} {} {} {}{}",
        name,
        company,
        address.address_line_1,
        address.city,
        address.province,
        address.postal_code,
        address.country_code,
        flag_suffix
    );
}

fn normalize_string(value: String) -> String {
    value.trim().to_string()
}

fn normalize_optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|v| {
            let trimmed = v.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        })
        .flatten()
}

fn parse_order_item(raw: &str) -> Result<OrderItemInput, String> {
    let mut sku = None;
    let mut quantity = None;
    let mut unit_price = None;
    let mut product_id = None;
    let mut name = None;
    let mut tax_rate = None;

    for part in raw.split(',') {
        let (key, value) = part
            .split_once('=')
            .ok_or_else(|| format!("invalid segment '{part}', expected key=value"))?;
        let key = key.trim();
        let value = value.trim();

        match key {
            "sku" => {
                if value.is_empty() {
                    return Err("sku cannot be empty".to_string());
                }
                sku = Some(value.to_string());
            }
            "quantity" => {
                let qty: i32 = value
                    .parse()
                    .map_err(|_| format!("invalid quantity '{value}'"))?;
                if qty <= 0 {
                    return Err("quantity must be positive".to_string());
                }
                quantity = Some(qty);
            }
            "price" | "unit_price" => {
                let price = Decimal::from_str(value)
                    .map_err(|_| format!("invalid price '{value}'"))?;
                unit_price = Some(price);
            }
            "product_id" => {
                let id = Uuid::parse_str(value)
                    .map_err(|_| format!("invalid product_id '{value}'"))?;
                product_id = Some(id);
            }
            "name" => {
                if !value.is_empty() {
                    name = Some(value.to_string());
                }
            }
            "tax_rate" => {
                let rate = Decimal::from_str(value)
                    .map_err(|_| format!("invalid tax_rate '{value}'"))?;
                tax_rate = Some(rate);
            }
            other => {
                return Err(format!("unrecognized key '{other}' in item definition"));
            }
        }
    }

    let sku = sku.ok_or_else(|| "item must include sku=<value>".to_string())?;
    let quantity =
        quantity.ok_or_else(|| "item must include quantity=<integer>".to_string())?;
    let unit_price =
        unit_price.ok_or_else(|| "item must include price=<decimal>".to_string())?;

    Ok(OrderItemInput {
        sku,
        product_id,
        name,
        quantity,
        unit_price,
        tax_rate,
    })
}

fn parse_decimal(raw: &str) -> Result<Decimal, String> {
    Decimal::from_str(raw).map_err(|_| format!("invalid decimal '{raw}'"))
}

fn parse_positive_u64(raw: &str) -> Result<u64, String> {
    let value: u64 = raw
        .parse()
        .map_err(|_| format!("invalid integer '{raw}'"))?;
    if value == 0 {
        Err("value must be greater than zero".to_string())
    } else {
        Ok(value)
    }
}

fn parse_datetime(raw: &str) -> Result<DateTime<Utc>, String> {
    DateTime::parse_from_rfc3339(raw)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|_| format!("invalid datetime '{}', expected RFC3339", raw))
}

fn parse_key_value(raw: &str) -> Result<(String, String), String> {
    let (key, value) = raw
        .split_once('=')
        .ok_or_else(|| format!("invalid option '{raw}', expected key=value"))?;
    let key = normalize_string(key.to_string());
    let value = normalize_string(value.to_string());
    if key.is_empty() {
        return Err("option key cannot be empty".to_string());
    }
    if value.is_empty() {
        return Err(format!("option '{key}' has empty value"));
    }
    Ok((key, value))
}

fn timestamp_to_utc(ts: i64) -> Option<String> {
    DateTime::<Utc>::from_timestamp(ts, 0).map(|dt| dt.to_rfc3339())
}

fn session_file_path() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("STATESET_CLI_HOME") {
        let mut path = PathBuf::from(dir);
        if path.file_name().is_none() {
            path.push("session.json");
        }
        return Some(path);
    }

    std::env::var("HOME").ok().map(|home| {
        let mut path = PathBuf::from(home);
        path.push(".stateset");
        path.push("session.json");
        path
    })
}

fn save_session(path: &Path, session: &StoredSession) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed creating directory {}", parent.display()))?;
    }

    let payload = serde_json::to_vec_pretty(session)?;
    fs::write(path, payload).with_context(|| format!("failed writing {}", path.display()))?;
    Ok(())
}

fn persist_session(save: bool, email: &str, tokens: &TokenPair) -> Result<Option<String>> {
    if !save {
        return Ok(None);
    }

    if let Some(path) = session_file_path() {
        let session = StoredSession {
            email: email.to_string(),
            access_token: tokens.access_token.clone(),
            refresh_token: tokens.refresh_token.clone(),
            token_type: tokens.token_type.clone(),
            expires_in: tokens.expires_in,
            refresh_expires_in: tokens.refresh_expires_in,
            saved_at: Utc::now(),
        };
        save_session(&path, &session)?;
        Ok(Some(path.display().to_string()))
    } else {
        eprintln!("Skipping session persistence: no suitable directory found.");
        Ok(None)
    }
}

fn read_session() -> Result<Option<(PathBuf, StoredSession)>> {
    let path = match session_file_path() {
        Some(path) => path,
        None => return Ok(None),
    };

    if !path.exists() {
        return Ok(None);
    }

    let data = fs::read_to_string(&path)
        .with_context(|| format!("failed to read session file {}", path.display()))?;
    let session: StoredSession = serde_json::from_str(&data)
        .with_context(|| format!("failed to parse session file {}", path.display()))?;
    Ok(Some((path, session)))
}

fn clear_session_file(path: &Path) -> Result<()> {
    if path.exists() {
        fs::remove_file(path)
            .with_context(|| format!("failed to remove {}", path.display()))?;
    }
    Ok(())
}

async fn list_customers(
    db: &Arc<DbPool>,
    args: &ListCustomersArgs,
) -> Result<Vec<CustomerResponse>> {
    if args.limit == 0 {
        return Err(anyhow!("limit must be greater than zero"));
    }

    let mut query = customer::Entity::find();

    if let Some(search) = args.search.as_ref() {
        let pattern = format!("%{}%", search);
        query = query.filter(
            Condition::any()
                .add(customer::Column::FirstName.like(&pattern))
                .add(customer::Column::LastName.like(&pattern))
                .add(customer::Column::Email.like(&pattern)),
        );
    }

    let models = query
        .order_by_desc(customer::Column::CreatedAt)
        .limit(args.limit)
        .offset(args.offset)
        .all(db.as_ref())
        .await
        .context("failed to fetch customers")?;

    Ok(models.into_iter().map(CustomerResponse::from).collect())
}
