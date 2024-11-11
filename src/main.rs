use axum::{
    routing::{get, post},
    Router, Extension,
};
use std::sync::Arc;
use tokio::sync::broadcast;
use slog::{info, o, Drain, Logger};
use dotenv::dotenv;
use opentelemetry::global;
use tower_http::{compression::CompressionLayer, trace::TraceLayer};

mod config;
mod services;
mod models;
mod handlers;
mod events;
mod commands;
mod queries;
mod errors;
mod logging;
mod cache;
mod rate_limiter;
mod message_queue;
mod circuit_breaker;
mod tracing;
mod health;
mod db;
mod proto;
mod auth;
mod grpc_server;

use config::AppConfig;
use errors::AppError;

/// Application State holding shared resources and services
#[derive(Clone)]
struct AppState {
    config: Arc<AppConfig>,
    db_pool: Arc<db::DbPool>,
    redis_client: Arc<redis::Client>,
    event_sender: broadcast::Sender<events::Event>,
    logger: Logger,
    services: Services,
}

/// Grouped Services for better organization
#[derive(Clone)]
struct Services {
    orders: Arc<services::orders::OrderService>,
    inventory: Arc<services::inventory::InventoryService>,
    returns: Arc<services::returns::ReturnService>,
    warranties: Arc<services::warranties::WarrantyService>,
    shipments: Arc<services::shipments::ShipmentService>,
    work_orders: Arc<services::work_orders::WorkOrderService>,
    bill_of_materials: Arc<services::billofmaterials::BillOfMaterialsService>,
    suppliers: Arc<services::suppliers::SupplierService>,
    customers: Arc<services::customers::CustomerService>,
    procurement: Arc<services::procurement::ProcurementService>,
    packing_lists: Arc<services::packing_lists::PackingListService>,
    packing_list_items: Arc<services::packing_list_items::PackingListItemService>,
    sourcing: Arc<services::sourcing::SourcingService>,
    demand_planning: Arc<services::demand_planning::DemandPlanningService>,
    distribution: Arc<services::distribution::DistributionService>,
    logistics: Arc<services::logistics::LogisticsService>,
    warehousing: Arc<services::warehousing::WarehousingService>,
    invoicing: Arc<services::invoicing::InvoicingService>,
    payments: Arc<services::payments::PaymentService>,
    accounting: Arc<services::accounting::AccountingService>,
    budgeting: Arc<services::budgeting::BudgetingService>,
    financial_reporting: Arc<services::financial_reporting::FinancialReportingService>,
    business_intelligence: Arc<services::business_intelligence::BusinessIntelligenceService>,
    forecasting: Arc<services::forecasting::ForecastingService>,
    trend_analysis: Arc<services::trend_analysis::TrendAnalysisService>,
    kpi_tracking: Arc<services::kpi_tracking::KPITrackingService>,
    leads: Arc<services::leads::LeadsService>,
    accounts: Arc<services::accounts::AccountService>,
    cases: Arc<services::cases::CaseService>,
    vendors: Arc<services::vendors::VendorService>,
    contacts: Arc<services::contacts::ContactService>,
    projects: Arc<services::projects::ProjectService>,
    assets: Arc<services::assets::AssetService>,
    maintenance: Arc<services::maintenance::MaintenanceService>,
    tasks: Arc<services::tasks::TaskService>,
    timesheets: Arc<services::timesheets::TimesheetService>,
    quality: Arc<services::quality::QualityService>,
    inspections: Arc<services::inspections::InspectionService>,
    non_conformance: Arc<services::non_conformance::NonConformanceService>,
    settings: Arc<services::settings::SettingService>,
    configurations: Arc<services::configurations::ConfigurationService>,
    notifications: Arc<services::notifications::NotificationService>,
    logs: Arc<services::logs::LogService>,
    reports: Arc<services::reports::ReportService>,
    exports: Arc<services::exports::ExportService>,
    imports: Arc<services::imports::ImportService>,
    alerts: Arc<services::alerts::AlertService>,
    oauth: Arc<services::oauth::OAuthService>,
    notes: Arc<services::notes::NoteService>,
    comments: Arc<services::comments::CommentService>,
    tags: Arc<services::tags::TagService>,
    events: Arc<services::events::EventService>,
}

#[tokio::main]
async fn main() -> Result<(), AppError> {
    dotenv().ok();
    let config = Arc::new(config::load()?);
    let log = setup_logger(&config);

    info!(log, "Starting StateSet API"; 
        "environment" => &config.environment,
        "version" => env!("CARGO_PKG_VERSION")
    );

    let app_state = build_app_state(&config, &log).await?;

    let schema = Arc::new(graphql::create_schema(
        app_state.services.orders.clone(),
        app_state.services.inventory.clone(),
        app_state.services.returns.clone(),
        app_state.services.warranties.clone(),
        app_state.services.shipments.clone(),
        app_state.services.work_orders.clone(),
        app_state.services.bill_of_materials.clone(),
        app_state.services.manufacturing.clone(),
        app_state.services.suppliers.clone(),
        app_state.services.customers.clone(),
        app_state.services.procurement.clone(),
        app_state.services.packing_lists.clone(),
        app_state.services.packing_list_items.clone(),
        app_state.services.sourcing.clone(),
        app_state.services.demand_planning.clone(),
        app_state.services.distribution.clone(),
        app_state.services.logistics.clone(),
        app_state.services.warehousing.clone(),
        app_state.services.invoicing.clone(),
        app_state.services.payments.clone(),
        app_state.services.accounting.clone(),
        app_state.services.budgeting.clone(),
        app_state.services.financial_reporting.clone(),
        app_state.services.business_intelligence.clone(),
        app_state.services.forecasting.clone(),
        app_state.services.trend_analysis.clone(),
        app_state.services.kpi_tracking.clone(),
        app_state.services.leads.clone(),
        app_state.services.accounts.clone(),
        app_state.services.cases.clone(),
        app_state.services.vendors.clone(),
        app_state.services.contacts.clone(),
        app_state.services.projects.clone(),
        app_state.services.assets.clone(),
        app_state.services.maintenance.clone(),
        app_state.services.tasks.clone(),
        app_state.services.timesheets.clone(),
        app_state.services.quality.clone(),
        app_state.services.inspections.clone(),
        app_state.services.non_conformance.clone(),
        app_state.services.settings.clone(),
        app_state.services.configurations.clone(),
        app_state.services.notifications.clone(),
        app_state.services.logs.clone(),
        app_state.services.reports.clone(),
        app_state.services.exports.clone(),
        app_state.services.imports.clone(),
        app_state.services.alerts.clone(),
        app_state.services.oauth.clone(),
        app_state.services.notes.clone(),
        app_state.services.comments.clone(),
        app_state.services.tags.clone(),
        app_state.services.events.clone(),
    ));

    setup_telemetry(&config)?;

    // Spawn event processing
    tokio::spawn(events::process_events(
        app_state.event_sender.subscribe(),
        app_state.services.clone(),
        log.clone(),
    ));

    // Start gRPC server
    #[cfg(feature = "grpc")]
    let grpc_server = grpc_server::start(config.clone(), app_state.services.clone()).await?;

    // Build our application with routes
    let app = Router::new()
        .route("/health", get(handlers::health::health_check))
        .nest("/orders", handlers::orders::routes())
        .nest("/inventory", handlers::inventory::routes())
        .nest("/returns", handlers::returns::routes())
        .nest("/warranties", handlers::warranties::routes())
        .nest("/shipments", handlers::shipments::routes())
        .nest("/work_orders", handlers::work_orders::routes())
        .nest("/work_order_line_items", handlers::work_order_line_items::routes())
        .nest("/bill_of_materials", handlers::bill_of_materials::routes())
        .nest("/bom_line_items", handlers::bill_of_materials_line_items::routes())
        .nest("/manufacturing", handlers::manufacturing::routes())
        .nest("/manufacture_orders", handlers::manufacture_orders::routes())
        .nest("/manufacture_order_line_items", handlers::manufacture_order_line_items::routes())
        .nest("/asn", handlers::asn::routes())
        .nest("/asn_line_items", handlers::asn_line_items::routes())
        .nest("/suppliers", handlers::suppliers::routes())
        .nest("/customers", handlers::customers::routes())
        .nest("/procurement", handlers::procurement::routes())
        .nest("/packing_lists", handlers::packing_lists::routes())
        .nest("/packing_list_items", handlers::packing_list_items::routes())
        .nest("/sourcing", handlers::sourcing::routes())
        .nest("/demand_planning", handlers::demand_planning::routes())
        .nest("/distribution", handlers::distribution::routes())
        .nest("/logistics", handlers::logistics::routes())
        .nest("/warehousing", handlers::warehousing::routes())
        .nest("/invoicing", handlers::invoicing::routes())
        .nest("/payments", handlers::payments::routes())
        .nest("/accounting", handlers::accounting::routes())
        .nest("/budgeting", handlers::budgeting::routes())
        .nest("/financial_reporting", handlers::financial_reporting::routes())
        .nest("/business_intelligence", handlers::business_intelligence::routes())
        .nest("/forecasting", handlers::forecasting::routes())
        .nest("/trend_analysis", handlers::trend_analysis::routes())
        .nest("/kpi_tracking", handlers::kpi_tracking::routes())
        .nest("/leads", handlers::leads::routes())
        .nest("/accounts", handlers::accounts::routes())
        .nest("/cases", handlers::cases::routes())
        .nest("/vendors", handlers::vendors::routes())
        .nest("/contacts", handlers::contacts::routes())
        .nest("/projects", handlers::projects::routes())
        .nest("/assets", handlers::assets::routes())
        .nest("/maintenance", handlers::maintenance::routes())
        .nest("/tasks", handlers::tasks::routes())
        .nest("/timesheets", handlers::timesheets::routes())
        .nest("/quality", handlers::quality::routes())
        .nest("/inspections", handlers::inspections::routes())
        .nest("/non_conformance", handlers::non_conformance::routes())
        .nest("/settings", handlers::settings::routes())
        .nest("/configurations", handlers::configurations::routes())
        .nest("/notifications", handlers::notifications::routes())
        .nest("/logs", handlers::logs::routes())
        .nest("/reports", handlers::reports::routes())
        .nest("/exports", handlers::exports::routes())
        .nest("/imports", handlers::imports::routes())
        .nest("/alerts", handlers::alerts::routes())
        .nest("/oauth", handlers::oauth::routes())
        .nest("/notes", handlers::notes::routes())
        .nest("/users", handlers::users::routes())
        .route("/proto_endpoint", post(handle_proto_request))
        .layer(Extension(app_state))
        .layer(Extension(schema))
        .layer(TraceLayer::new_for_http())
        .layer(CompressionLayer::new())
        .layer(axum::middleware::from_fn(auth::auth_middleware))
        .layer(axum::middleware::from_fn(rate_limiter::rate_limit_middleware));

    // Run our app with Hyper
    let addr = format!("{}:{}", config.host, config.port);
    info!(log, "StateSet API server running"; "address" => &addr);
    axum::Server::bind(&addr.parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();

    info!(log, "Shutting down");
    Ok(())
}

/// Sets up the logger using slog
fn setup_logger(config: &AppConfig) -> Logger {
    let decorator = slog_term::TermDecorator::new().build();
    let drain = slog_term::FullFormat::new(decorator).build().fuse();
    let drain = slog_async::Async::new(drain).build().fuse();
    let drain = slog::LevelFilter::new(drain, config.log_level.parse().unwrap()).fuse();
    slog::Logger::root(drain, o!())
}

/// Builds the application state by initializing the database, cache, message queues, and services
async fn build_app_state(config: &Arc<AppConfig>, log: &Logger) -> Result<AppState, AppError> {
    let db_pool = Arc::new(db::establish_connection(&config.database_url).await?);
    let redis_client = Arc::new(redis::Client::open(&config.redis_url)?);
    let rabbit_conn = message_queue::connect_rabbitmq(&config.rabbitmq_url).await?;
    let (event_sender, _) = broadcast::channel::<events::Event>(100);

    let services = initialize_services(
        db_pool.clone(),
        redis_client.clone(),
        rabbit_conn,
        event_sender.clone(),
        log.clone(),
    )
    .await?;

    Ok(AppState {
        config: config.clone(),
        db_pool,
        redis_client,
        event_sender,
        logger: log.clone(),
        services,
    })
}

/// Initializes all the services required by the application
async fn initialize_services(
    db_pool: Arc<db::DbPool>,
    redis_client: Arc<redis::Client>,
    rabbit_conn: lapin::Connection,
    event_sender: broadcast::Sender<events::Event>,
    log: Logger,
) -> Result<Services, AppError> {
    // Initialize common components
    let rate_limiter = Arc::new(rate_limiter::RateLimiter::new(redis_client.clone(), "global", 1000, 60));
    let message_queue = Arc::new(message_queue::RabbitMQ::new(rabbit_conn));
    let circuit_breaker = Arc::new(circuit_breaker::CircuitBreaker::new(5, std::time::Duration::from_secs(60)));

    // Helper macro to initialize services
    macro_rules! init_service {
        ($service_type:ty, $service_var:ident) => {
            let $service_var = Arc::new(services::$service_type::new(
                db_pool.clone(),
                event_sender.clone(),
                redis_client.clone(),
                message_queue.clone(),
                circuit_breaker.clone(),
                log.clone(),
            ));
        };
    }

    // Initialize each service using the macro
    init_service!(orders::OrderService, order_service);
    init_service!(inventory::InventoryService, inventory_service);
    init_service!(returns::ReturnService, return_service);
    init_service!(warranties::WarrantyService, warranty_service);
    init_service!(shipments::ShipmentService, shipment_service);
    init_service!(work_orders::WorkOrderService, work_order_service);
    init_service!(billofmaterials::BillOfMaterialsService, bill_of_materials_service);
    init_service!(suppliers::SupplierService, suppliers_service);
    init_service!(customers::CustomerService, customers_service);
    init_service!(procurement::ProcurementService, procurement_service);
    init_service!(packing_lists::PackingListService, packing_lists_service);
    init_service!(packing_list_items::PackingListItemService, packing_list_items_service);
    init_service!(sourcing::SourcingService, sourcing_service);
    init_service!(demand_planning::DemandPlanningService, demand_planning_service);
    init_service!(distribution::DistributionService, distribution_service);
    init_service!(logistics::LogisticsService, logistics_service);
    init_service!(warehousing::WarehousingService, warehousing_service);
    init_service!(invoicing::InvoicingService, invoicing_service);
    init_service!(payments::PaymentService, payments_service);
    init_service!(accounting::AccountingService, accounting_service);
    init_service!(budgeting::BudgetingService, budgeting_service);
    init_service!(financial_reporting::FinancialReportingService, financial_reporting_service);
    init_service!(business_intelligence::BusinessIntelligenceService, business_intelligence_service);
    init_service!(forecasting::ForecastingService, forecasting_service);
    init_service!(trend_analysis::TrendAnalysisService, trend_analysis_service);
    init_service!(kpi_tracking::KPITrackingService, kpi_tracking_service);
    init_service!(leads::LeadsService, leads_service);
    init_service!(accounts::AccountService, accounts_service);
    init_service!(cases::CaseService, cases_service);
    init_service!(vendors::VendorService, vendors_service);
    init_service!(contacts::ContactService, contacts_service);
    init_service!(projects::ProjectService, projects_service);
    init_service!(assets::AssetService, assets_service);
    init_service!(maintenance::MaintenanceService, maintenance_service);
    init_service!(tasks::TaskService, tasks_service);
    init_service!(timesheets::TimesheetService, timesheets_service);
    init_service!(quality::QualityService, quality_service);
    init_service!(inspections::InspectionService, inspections_service);
    init_service!(non_conformance::NonConformanceService, non_conformance_service);
    init_service!(settings::SettingService, settings_service);
    init_service!(configurations::ConfigurationService, configurations_service);
    init_service!(notifications::NotificationService, notifications_service);
    init_service!(logs::LogService, logs_service);
    init_service!(reports::ReportService, reports_service);
    init_service!(exports::ExportService, exports_service);
    init_service!(imports::ImportService, imports_service);
    init_service!(alerts::AlertService, alerts_service);
    init_service!(oauth::OAuthService, oauth_service);
    init_service!(notes::NoteService, notes_service);
    init_service!(comments::CommentService, comments_service);
    init_service!(tags::TagService, tags_service);
    init_service!(events::EventService, events_service);

    // Construct the Services struct
    Ok(Services {
        orders: order_service,
        inventory: inventory_service,
        returns: return_service,
        warranties: warranty_service,
        shipments: shipment_service,
        work_orders: work_order_service,
        bill_of_materials: bill_of_materials_service,
        suppliers: suppliers_service,
        customers: customers_service,
        procurement: procurement_service,
        packing_lists: packing_lists_service,
        packing_list_items: packing_list_items_service,
        sourcing: sourcing_service,
        demand_planning: demand_planning_service,
        distribution: distribution_service,
        logistics: logistics_service,
        warehousing: warehousing_service,
        invoicing: invoicing_service,
        payments: payments_service,
        accounting: accounting_service,
        budgeting: budgeting_service,
        financial_reporting: financial_reporting_service,
        business_intelligence: business_intelligence_service,
        forecasting: forecasting_service,
        trend_analysis: trend_analysis_service,
        kpi_tracking: kpi_tracking_service,
        leads: leads_service,
        accounts: accounts_service,
        cases: cases_service,
        vendors: vendors_service,
        contacts: contacts_service,
        projects: projects_service,
        assets: assets_service,
        maintenance: maintenance_service,
        tasks: tasks_service,
        timesheets: timesheets_service,
        quality: quality_service,
        inspections: inspections_service,
        non_conformance: non_conformance_service,
        settings: settings_service,
        configurations: configurations_service,
        notifications: notifications_service,
        logs: logs_service,
        reports: reports_service,
        exports: exports_service,
        imports: imports_service,
        alerts: alerts_service,
        oauth: oauth_service,
        notes: notes_service,
        comments: comments_service,
        tags: tags_service,
        events: events_service,
    })
}

/// Handles incoming protobuf requests
async fn handle_proto_request(
    Extension(state): Extension<AppState>,
    payload: axum::body::Bytes,
) -> Result<axum::response::Response<axum::body::Full<axum::body::Bytes>>, axum::http::StatusCode> {
    // Deserialize the incoming protobuf message
    let request = proto::SomeRequest::decode(payload.as_ref())
        .map_err(|_| axum::http::StatusCode::BAD_REQUEST)?;

    // Process the request (add your business logic here)
    let response = proto::SomeResponse {
        // Populate response fields based on your logic
        message: format!("Processed request with id: {}", request.id),
    };

    // Serialize the response back to protobuf
    let mut buf = Vec::new();
    response
        .encode(&mut buf)
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(axum::response::Response::builder()
        .status(axum::http::StatusCode::OK)
        .body(axum::body::Full::from(buf))
        .unwrap())
}

/// Sets up OpenTelemetry telemetry
fn setup_telemetry(config: &AppConfig) -> Result<(), AppError> {
    let tracer = opentelemetry_jaeger::new_pipeline()
        .with_service_name("stateset-api")
        .with_endpoint(&config.jaeger_endpoint)
        .install_simple()?;
    global::set_tracer_provider(tracer);
    Ok(())
}
