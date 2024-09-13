use axum::{
    routing::{get, post},
    Router, Extension,
};
use std::sync::Arc;
use tokio::sync::broadcast;
use slog::{info, o, Drain, Logger};
use dotenv::dotenv;
use opentelemetry::global;
use tower_http::compression::CompressionLayer;
use tower_http::trace::TraceLayer;

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

#[derive(Clone)]
struct AppState {
    config: Arc<AppConfig>,
    db_pool: Arc<db::DbPool>,
    redis_client: Arc<redis::Client>,
    event_sender: broadcast::Sender<events::Event>,
    logger: Logger,
    services: Services,
}

#[derive(Clone)]
struct Services {
    order_service: Arc<services::orders::OrderService>,
    inventory_service: Arc<services::inventory::InventoryService>,
    return_service: Arc<services::returns::ReturnService>,
    warranty_service: Arc<services::warranties::WarrantyService>,
    shipment_service: Arc<services::shipments::ShipmentService>,
    work_order_service: Arc<services::work_orders::WorkOrderService>,
    billofmaterials_service: Arc<services::billofmaterials::BillOfMaterialsService>,
    suppliers_service: Arc<services::suppliers::SupplierService>,
    customers_service: Arc<services::customers::CustomerService>,
    procurement_service: Arc<services::procurement::ProcurementService>,
    packing_lists_service: Arc<services::packing_lists::PackingListService>,
    packing_list_items_service: Arc<services::packing_list_items::PackingListItemService>,
    sourcing_service: Arc<services::sourcing::SourcingService>,
    demand_planning_service: Arc<services::demand_planning::DemandPlanningService>,
    distribution_service: Arc<services::distribution::DistributionService>,
    logistics_service: Arc<services::logistics::LogisticsService>,
    warehousing_service: Arc<services::warehousing::WarehousingService>,
    invoicing_service: Arc<services::invoicing::InvoicingService>,
    payments_service: Arc<services::payments::PaymentService>,
    accounting_service: Arc<services::accounting::AccountingService>,
    budgeting_service: Arc<services::budgeting::BudgetingService>,
    financial_reporting_service: Arc<services::financial_reporting::FinancialReportingService>,
    business_intelligence_service: Arc<services::business_intelligence::BusinessIntelligenceService>,
    forecasting_service: Arc<services::forecasting::ForecastingService>,
    trend_analysis_service: Arc<services::trend_analysis::TrendAnalysisService>,
    kpi_tracking_service: Arc<services::kpi_tracking::KPITrackingService>,
    leads_service: Arc<services::leads::LeadsService>,
    accounts_service: Arc<services::accounts::AccountService>,
    cases_service: Arc<services::cases::CaseService>,
    vendors_service: Arc<services::vendors::VendorService>,
    contacts_service: Arc<services::contacts::ContactService>,
    projects_service: Arc<services::projects::ProjectService>,
    assets_service: Arc<services::assets::AssetService>,
    maintenance_service: Arc<services::maintenance::MaintenanceService>,
    tasks_service: Arc<services::tasks::TaskService>,
    timesheets_service: Arc<services::timesheets::TimesheetService>,
    quality_service: Arc<services::quality::QualityService>,
    inspections_service: Arc<services::inspections::InspectionService>,
    non_conformance_service: Arc<services::non_conformance::NonConformanceService>,
    settings_service: Arc<services::settings::SettingService>,
    configurations_service: Arc<services::configurations::ConfigurationService>,
    notifications_service: Arc<services::notifications::NotificationService>,
    logs_service: Arc<services::logs::LogService>,
    reports_service: Arc<services::reports::ReportService>,
    exports_service: Arc<services::exports::ExportService>,
    
}

// StateSet API Web Services
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
        app_state.services.order_service.clone(),
        app_state.services.inventory_service.clone(),
        app_state.services.return_service.clone(),
        app_state.services.warranty_service.clone(),
        app_state.services.shipment_service.clone(),
        app_state.services.work_order_service.clone(),
        app_state.services.billofmaterials_service.clone(),
        app_state.services.manufacturing_service.clone(),
        app_state.services.suppliers_service.clone(),
        app_state.services.customers_service.clone(),
        app_state.services.procurement_service.clone(),
        app_state.services.packing_lists_service.clone(),
        app_state.services.packing_list_items_service.clone(),
        app_state.services.sourcing_service.clone(),
        app_state.services.demand_planning_service.clone(),
        app_state.services.distribution_service.clone(),
        app_state.services.logistics_service.clone(),
        app_state.services.warehousing_service.clone(),
        app_state.services.invoicing_service.clone(),
        app_state.services.payments_service.clone(),
        app_state.services.accounting_service.clone(),
        app_state.services.budgeting_service.clone(),
        app_state.services.financial_reporting_service.clone(),
        app_state.services.business_intelligence_service.clone(),
        app_state.services.forecasting_service.clone(),
        app_state.services.trend_analysis_service.clone(),
        app_state.services.kpi_tracking_service.clone(),
        app_state.services.leads_service.clone(),
        app_state.services.accounts_service.clone(),
        app_state.services.cases_service.clone(),
        app_state.services.vendors_service.clone(),
        app_state.services.contacts_service.clone(),
        app_state.services.projects_service.clone(),
        app_state.services.assets_service.clone(),
        app_state.services.maintenance_service.clone(),
        app_state.services.tasks_service.clone(),
        app_state.services.timesheets_service.clone(),
        app_state.services.quality_service.clone(),
        app_state.services.inspections_service.clone(),
        app_state.services.non_conformance_service.clone(),
        app_state.services.settings_service.clone(),
        app_state.services.configurations_service.clone(),
        app_state.services.notifications_service.clone(),
        app_state.services.logs_service.clone(),
        app_state.services.reports_service.clone(),
        app_state.services.exports_service.clone(),
        app_state.services.imports_service.clone(),
        app_state.services.oauth_service.clone(),
        
        
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

    // Build our application with a route
    let app = Router::new()
        .route("/health", get(health::health_check))
        .nest("/orders", handlers::orders::routes())
        .nest("/inventory", handlers::inventory::routes())
        .nest("/return", handlers::returns::routes())
        .nest("/warranties", handlers::warranties::routes())
        .nest("/shipments", handlers::shipments::routes())
        .nest("/work_orders", handlers::work_orders::routes())
        .nest("/work_order_line_items", handlers::work_order_line_items::routes())
        .nest("/billofmaterials", handlers::billofmaterials::routes())
        .nest("/bill_of_materials_line_items", handlers::bill_of_materials_line_items::routes())
        .nest("/manufacturing", handlers::manufacturing::routes())
        .nest("/manufacture_orders", handlers::manufacture_orders::routes())
        .nest("/manufacture_order_line_items", handlers::manufacture_order_line_items::routes())
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
        .nest("/suppliers", handlers::suppliers::routes())
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
        .nest("/notifications", handlers::notifications::routes())
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

    // Run our app with hyper
    let addr = format!("{}:{}", config.host, config.port);
    info!(log, "StateSet API server running"; "address" => &addr);
    axum::Server::bind(&addr.parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();

    info!(log, "Shutting down");
    Ok(())
}

fn setup_logger(config: &AppConfig) -> Logger {
    let decorator = slog_term::TermDecorator::new().build();
    let drain = slog_term::FullFormat::new(decorator).build().fuse();
    let drain = slog_async::Async::new(drain).build().fuse();
    let drain = slog::LevelFilter::new(drain, config.log_level.parse().unwrap()).fuse();
    slog::Logger::root(drain, o!())
}

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
    ).await?;

    Ok(AppState {
        config: config.clone(),
        db_pool,
        redis_client,
        event_sender,
        logger: log.clone(),
        services,
    })
}

async fn initialize_services(
    db_pool: Arc<db::DbPool>,
    redis_client: Arc<redis::Client>,
    rabbit_conn: lapin::Connection,
    event_sender: broadcast::Sender<events::Event>,
    log: Logger,
) -> Result<Services, AppError> {
    let rate_limiter = Arc::new(rate_limiter::RateLimiter::new(redis_client.clone(), "global", 1000, 60));
    let message_queue = Arc::new(message_queue::RabbitMQ::new(rabbit_conn));
    let circuit_breaker = Arc::new(circuit_breaker::CircuitBreaker::new(5, std::time::Duration::from_secs(60)));

    let inventory_service = Arc::new(services::inventory::InventoryService::new(
        db_pool.clone(),
        event_sender.clone(),
        redis_client.clone(),
        message_queue.clone(),
        circuit_breaker.clone(),
        log.clone(),
    ));

    let order_service = Arc::new(services::orders::OrderService::new(
        db_pool.clone(),
        inventory_service.clone(),
        event_sender.clone(),
        redis_client.clone(),
        message_queue.clone(),
        circuit_breaker.clone(),
        log.clone(),
    ));

    let return_service = Arc::new(services::returns::ReturnService::new(
        db_pool.clone(),
        inventory_service.clone(),
        event_sender.clone(),
        redis_client.clone(),
        message_queue.clone(),
        circuit_breaker.clone(),
        log.clone(),
    ));

    let warranty_service = Arc::new(services::warranties::WarrantyService::new(
        db_pool.clone(),
        event_sender.clone(),
        redis_client.clone(),
        message_queue.clone(),
        circuit_breaker.clone(),
        log.clone(),
    ));

    let shipment_service = Arc::new(services::shipments::ShipmentService::new(
        db_pool.clone(),
        event_sender.clone(),
        redis_client.clone(),
        message_queue.clone(),
        circuit_breaker.clone(),
        log.clone(),
    ));

    let work_order_service = Arc::new(services::work_orders::WorkOrderService::new(
        db_pool.clone(),
        inventory_service.clone(),
        event_sender.clone(),
        redis_client.clone(),
        message_queue.clone(),
        circuit_breaker.clone(),
        log.clone(),
    ));

    let billofmaterials_service = Arc::new(services::billofmaterials::BillOfMaterialsService::new(
        db_pool.clone(),
        event_sender.clone(),
        redis_client.clone(),
        message_queue.clone(),
        circuit_breaker.clone(),
        log.clone(),
    ));

    let suppliers_service = Arc::new(services::suppliers::SupplierService::new(
        db_pool.clone(),
        event_sender.clone(),
        redis_client.clone(),
        message_queue.clone(),
        circuit_breaker.clone(),
        log.clone(),
    ));

    let customers_service = Arc::new(services::customers::CustomerService::new(
        db_pool.clone(),
        event_sender.clone(),
        redis_client.clone(),
        message_queue.clone(),
        circuit_breaker.clone(),
        log.clone(),
    ));

    let procurement_service = Arc::new(services::procurement::ProcurementService::new(
        db_pool.clone(),
        event_sender.clone(),
        redis_client.clone(),
        message_queue.clone(),
        circuit_breaker.clone(),
        log.clone(),
    ));

    let packing_lists_service = Arc::new(services::packing_lists::PackingListService::new(
        db_pool.clone(),
        event_sender.clone(),
        redis_client.clone(),
        message_queue.clone(),
        circuit_breaker.clone(),
        log.clone(),
    ));

    let packing_list_items_service = Arc::new(services::packing_list_items::PackingListItemService::new(
        db_pool.clone(),
        event_sender.clone(),
        redis_client.clone(),
        message_queue.clone(),
        circuit_breaker.clone(),
        log.clone(),
    ));

    let sourcing_service = Arc::new(services::sourcing::SourcingService::new(
        db_pool.clone(),
        event_sender.clone(),
        redis_client.clone(),
        message_queue.clone(),
        circuit_breaker.clone(),
        log.clone(),
    ));

    let demand_planning_service = Arc::new(services::demand_planning::DemandPlanningService::new(
        db_pool.clone(),
        event_sender.clone(),
        redis_client.clone(),
        message_queue.clone(),
        circuit_breaker.clone(),
        log.clone(),
    ));

    let distribution_service = Arc::new(services::distribution::DistributionService::new(
        db_pool.clone(),
        event_sender.clone(),
        redis_client.clone(),
        message_queue.clone(),
        circuit_breaker.clone(),
        log.clone(),
    ));

    let logistics_service = Arc::new(services::logistics::LogisticsService::new(
        db_pool.clone(),
        event_sender.clone(),
        redis_client.clone(),
        message_queue.clone(),
        circuit_breaker.clone(),
        log.clone(),
    ));

    let warehousing_service = Arc::new(services::warehousing::WarehousingService::new(
        db_pool.clone(),
        event_sender.clone(),
        redis_client.clone(),
        message_queue.clone(),
        circuit_breaker.clone(),
        log.clone(),
    ));

    let invoicing_service = Arc::new(services::invoicing::InvoicingService::new(
        db_pool.clone(),
        event_sender.clone(),
        redis_client.clone(),
        message_queue.clone(),
        circuit_breaker.clone(),
        log.clone(),
    ));

    let payments_service = Arc::new(services::payments::PaymentService::new(
        db_pool.clone(),
        event_sender.clone(),
        redis_client.clone(),
        message_queue.clone(),
        circuit_breaker.clone(),
        log.clone(),
    ));

    let accounting_service = Arc::new(services::accounting::AccountingService::new(
        db_pool.clone(),
        event_sender.clone(),
        redis_client.clone(),
        message_queue.clone(),
        circuit_breaker.clone(),
        log.clone(),
    ));

    let budgeting_service = Arc::new(services::budgeting::BudgetingService::new(
        db_pool.clone(),
        event_sender.clone(),
        redis_client.clone(),
        message_queue.clone(),
        circuit_breaker.clone(),
        log.clone(),
    ));

    let financial_reporting_service = Arc::new(services::financial_reporting::FinancialReportingService::new(
        db_pool.clone(),
        event_sender.clone(),
        redis_client.clone(),
        message_queue.clone(),
        circuit_breaker.clone(),
        log.clone(),
    ));

    let business_intelligence_service = Arc::new(services::business_intelligence::BusinessIntelligenceService::new(
        db_pool.clone(),
        event_sender.clone(),
        redis_client.clone(),
        message_queue.clone(),
        circuit_breaker.clone(),
        log.clone(),
    ));

    let forecasting_service = Arc::new(services::forecasting::ForecastingService::new(
        db_pool.clone(),
        event_sender.clone(),
        redis_client.clone(),
        message_queue.clone(),
        circuit_breaker.clone(),
        log.clone(),
    ));

    let trend_analysis_service = Arc::new(services::trend_analysis::TrendAnalysisService::new(
        db_pool.clone(),
        event_sender.clone(),
        redis_client.clone(),
        message_queue.clone(),
        circuit_breaker.clone(),
        log.clone(),
    ));

    let kpi_tracking_service = Arc::new(services::kpi_tracking::KPITrackingService::new(
        db_pool.clone(),
        event_sender.clone(),
        redis_client.clone(),
        message_queue.clone(),
        circuit_breaker.clone(),
        log.clone(),
    ));

    let leads_service = Arc::new(services::leads::LeadsService::new(
        db_pool.clone(),
        event_sender.clone(),
        redis_client.clone(),
        message_queue.clone(),
        circuit_breaker.clone(),
        log.clone(),
    ));

    let accounts_service = Arc::new(services::accounts::AccountService::new(
        db_pool.clone(),
        event_sender.clone(),
        redis_client.clone(),
        message_queue.clone(),
        circuit_breaker.clone(),
        log.clone(),
    ));

    let cases_service = Arc::new(services::cases::CaseService::new(
        db_pool.clone(),
        event_sender.clone(),
        redis_client.clone(),
        message_queue.clone(),
        circuit_breaker.clone(),
        log.clone(),
    ));

    let vendors_service = Arc::new(services::vendors::VendorService::new(
        db_pool.clone(),
        event_sender.clone(),
        redis_client.clone(),
        message_queue.clone(),
        circuit_breaker.clone(),
        log.clone(),
    ));

    let contacts_service = Arc::new(services::contacts::ContactService::new(
        db_pool.clone(),
        event_sender.clone(),
        redis_client.clone(),
        message_queue.clone(),
        circuit_breaker.clone(),
        log.clone(),
    ));

    let projects_service = Arc::new(services::projects::ProjectService::new(
        db_pool.clone(),
        event_sender.clone(),
        redis_client.clone(),
        message_queue.clone(),
        circuit_breaker.clone(),
        log.clone(),
    ));

    let assets_service = Arc::new(services::assets::AssetService::new(
        db_pool.clone(),
        event_sender.clone(),
        redis_client.clone(),
        message_queue.clone(),
        circuit_breaker.clone(),
        log.clone(),
    ));

    let maintenance_service = Arc::new(services::maintenance::MaintenanceService::new(
        db_pool.clone(),
        event_sender.clone(),
        redis_client.clone(),
        message_queue.clone(),
        circuit_breaker.clone(),
        log.clone(),
    ));

    let tasks_service = Arc::new(services::tasks::TaskService::new(
        db_pool.clone(),
        event_sender.clone(),
        redis_client.clone(),
        message_queue.clone(),
        circuit_breaker.clone(),
        log.clone(),
    ));

    let timesheets_service = Arc::new(services::timesheets::TimesheetService::new(
        db_pool.clone(),
        event_sender.clone(),
        redis_client.clone(),
        message_queue.clone(),
        circuit_breaker.clone(),
        log.clone(),
    ));

    let quality_service = Arc::new(services::quality::QualityService::new(
        db_pool.clone(),
        event_sender.clone(),
        redis_client.clone(),
        message_queue.clone(),
        circuit_breaker.clone(),
        log.clone(),
    ));

    let inspections_service = Arc::new(services::inspections::InspectionService::new(
        db_pool.clone(),
        event_sender.clone(),
        redis_client.clone(),
        message_queue.clone(),
        circuit_breaker.clone(),
        log.clone(),
    ));

    let non_conformance_service = Arc::new(services::non_conformance::NonConformanceService::new(
        db_pool.clone(),
        event_sender.clone(),
        redis_client.clone(),
        message_queue.clone(),
        circuit_breaker.clone(),
        log.clone(),
    ));

    let settings_service = Arc::new(services::settings::SettingsService::new(
        db_pool.clone(),
        event_sender.clone(),
        redis_client.clone(),
        message_queue.clone(),
        circuit_breaker.clone(),
        log.clone(),
    ));

    let configurations_service = Arc::new(services::configurations::ConfigurationService::new(
        db_pool.clone(),
        event_sender.clone(),
        redis_client.clone(),
        message_queue.clone(),
        circuit_breaker.clone(),
        log.clone(),
    ));

    let notifications_service = Arc::new(services::notifications::NotificationService::new(
        db_pool.clone(),
        event_sender.clone(),
        redis_client.clone(),
        message_queue.clone(),
        circuit_breaker.clone(),
        log.clone(),
    ));

    let logs_service = Arc::new(services::logs::LogService::new(
        db_pool.clone(),
        event_sender.clone(),
        redis_client.clone(),
        message_queue.clone(),
        circuit_breaker.clone(),
        log.clone(),
    ));

    let reports_service = Arc::new(services::reports::ReportService::new(
        db_pool.clone(),
        event_sender.clone(),
        redis_client.clone(),
        message_queue.clone(),
        circuit_breaker.clone(),
        log.clone(),
    ));

    let exports_service = Arc::new(services::exports::ExportService::new(
        db_pool.clone(),
        event_sender.clone(),
        redis_client.clone(),
        message_queue.clone(),
        circuit_breaker.clone(),
        log.clone(),
    ));

    let imports_service = Arc::new(services::imports::ImportService::new(
        db_pool.clone(),
        event_sender.clone(),
        redis_client.clone(),
        message_queue.clone(),
        circuit_breaker.clone(),
        log.clone(),
    ));

    let alerts_service = Arc::new(services::alerts::AlertService::new(
        db_pool.clone(),
        event_sender.clone(),
        redis_client.clone(),
        message_queue.clone(),
        circuit_breaker.clone(),
        log.clone(),
    ));

    let oauth_service = Arc::new(services::oauth::OAuthService::new(
        db_pool.clone(),
        event_sender.clone(),
        redis_client.clone(),
        message_queue.clone(),
        circuit_breaker.clone(),
        log.clone(),
    ));

    let notes_service = Arc::new(services::notes::NoteService::new(
        db_pool.clone(),
        event_sender.clone(),
        redis_client.clone(),
        message_queue.clone(),
        circuit_breaker.clone(),
        log.clone(),
    ));

    let comments_service = Arc::new(services::comments::CommentService::new(
        db_pool.clone(),
        event_sender.clone(),
        redis_client.clone(),
        message_queue.clone(),
        circuit_breaker.clone(),
        log.clone(),
    ));

    let tags_service = Arc::new(services::tags::TagService::new(
        db_pool.clone(),
        event_sender.clone(),
        redis_client.clone(),
        message_queue.clone(),
        circuit_breaker.clone(),
        log.clone(),
    ));

    let events_service = Arc::new(services::events::EventService::new(
        db_pool.clone(),
        event_sender.clone(),
        redis_client.clone(),
        message_queue.clone(),
        circuit_breaker.clone(),
        log.clone(),
    ));

    let notifications_service = Arc::new(services::notifications::NotificationService::new(
        db_pool.clone(),
        event_sender.clone(),
        redis_client.clone(),
        message_queue.clone(),
        circuit_breaker.clone(),
        log.clone(),
    ));

    let logs_service = Arc::new(services::logs::LogService::new(
        db_pool.clone(),
        event_sender.clone(),
        redis_client.clone(),
        message_queue.clone(),
        circuit_breaker.clone(),
        log.clone(),
    
    Services {
        order_service,
        inventory_service,
        return_service,
        warranty_service,
        shipment_service,
        work_order_service,
        billofmaterials_service,
        suppliers_service,
        customers_service,
        procurement_service,
        packing_lists_service,
        packing_list_items_service,
        sourcing_service,
        demand_planning_service,
        distribution_service,
        logistics_service,
        warehousing_service,
        invoicing_service,
        payments_service,
        accounting_service,
        budgeting_service,
        financial_reporting_service,
        business_intelligence_service,
        forecasting_service,
        trend_analysis_service,
        kpi_tracking_service,
        leads_service,
        accounts_service,
        cases_service,
        vendors_service,
        contacts_service,
        projects_service,
        assets_service,
        maintenance_service,
        tasks_service,
        timesheets_service,
        quality_service,
        inspections_service,
        non_conformance_service,
        settings_service,
        configurations_service,
        notifications_service,
        logs_service,
        reports_service,
        exports_service,
        imports_service,
        alerts_service,
        oauth_service,
        notes_service,
        comments_service,
        tags_service,
        events_service,
        notifications_service,
        logs_service,

    }
}


async fn handle_proto_request(
    Extension(state): Extension<AppState>,
    payload: axum::body::Bytes
) -> Result<axum::response::Response<axum::body::Full<axum::body::Bytes>>, axum::http::StatusCode> {
    // Deserialize the incoming protobuf message
    let request = proto::SomeRequest::decode(payload.as_ref())
        .map_err(|_| axum::http::StatusCode::BAD_REQUEST)?;

    // Process the request (this is where you'd add your business logic)
    let response = proto::SomeResponse {
        // Fill in the response fields based on your logic
        message: format!("Processed request with id: {}", request.id),
    };

    // Serialize the response back to protobuf
    let mut buf = Vec::new();
    response.encode(&mut buf)
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(axum::response::Response::builder()
        .status(axum::http::StatusCode::OK)
        .body(axum::body::Full::from(buf))
        .unwrap())
}

fn setup_telemetry(config: &AppConfig) -> Result<(), AppError> {
    let tracer = opentelemetry_jaeger::new_pipeline()
        .with_service_name("stateset-api")
        .with_endpoint(&config.jaeger_endpoint)
        .install_simple()?;
    global::set_tracer_provider(tracer);
    Ok(())
}