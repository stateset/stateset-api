use std::sync::Arc;
use std::net::SocketAddr;
use stateset_api::{config, db, events, services::AppServices, grpc};
use dotenv::dotenv;
use tracing::{info, error};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    let config = config::load_config()?;
    let db = db::establish_connection(&config.db_url).await.map_err(|e| {
        error!("DB connection failed: {}", e);
        e
    })?;
    let db = Arc::new(db);

    let (tx, rx) = tokio::sync::mpsc::channel(100);
    let sender = Arc::new(events::EventSender::new(tx));
    tokio::spawn(async move { events::process_events(rx).await; });

    let services = AppServices::new(db.clone(), sender.clone());

    let addr: SocketAddr = ([0, 0, 0, 0], config.port + 1).into();
    info!("gRPC server listening on {}", addr);

    grpc::serve(services, addr).await
}
