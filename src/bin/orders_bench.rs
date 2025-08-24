use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rand::Rng;
use tokio::task::JoinSet;

use stateset_api::proto::order::{
    order_service_client::OrderServiceClient, CreateOrderRequest, Order, OrderItem, OrderStatus,
};
use tonic::transport::Endpoint;

#[derive(Clone, Debug)]
struct BenchConfig {
    mode: String,                 // "grpc" or "http"
    target: String,               // e.g., "http://127.0.0.1:8081" for gRPC
    duration_secs: u64,           // test duration in seconds
    concurrency: usize,           // number of concurrent workers
    warmup_secs: u64,             // warmup duration in seconds
}

impl BenchConfig {
    fn from_env_or_args() -> Self {
        // Very light CLI parsing without external deps
        let mut mode = std::env::var("MODE").unwrap_or_else(|_| "grpc".to_string());
        let mut target = std::env::var("TARGET").unwrap_or_else(|_| "http://127.0.0.1:8081".to_string());
        let mut duration_secs = std::env::var("DURATION_SECS").ok().and_then(|v| v.parse().ok()).unwrap_or(15);
        let mut concurrency = std::env::var("CONCURRENCY").ok().and_then(|v| v.parse().ok()).unwrap_or(64);
        let mut warmup_secs = std::env::var("WARMUP_SECS").ok().and_then(|v| v.parse().ok()).unwrap_or(3);

        let args: Vec<String> = std::env::args().collect();
        let mut i = 1;
        while i + 1 < args.len() {
            match args[i].as_str() {
                "--mode" => mode = args[i + 1].clone(),
                "--target" => target = args[i + 1].clone(),
                "--duration" => duration_secs = args[i + 1].parse().unwrap_or(duration_secs),
                "--concurrency" => concurrency = args[i + 1].parse().unwrap_or(concurrency),
                "--warmup" => warmup_secs = args[i + 1].parse().unwrap_or(warmup_secs),
                _ => {}
            }
            i += 2;
        }

        Self { mode, target, duration_secs, concurrency, warmup_secs }
    }
}

#[derive(Default, Clone)]
struct Metrics {
    successes: u64,
    failures: u64,
    // store some latencies (in microseconds) for percentile approximations
    latencies_us: Vec<u128>,
}

impl Metrics {
    fn merge_into(&self, agg: &mut Metrics) {
        agg.successes += self.successes;
        agg.failures += self.failures;
        // Limit latencies to avoid unbounded memory in extreme runs
        let mut latencies = self.latencies_us.clone();
        if latencies.len() > 50_000 {
            latencies.truncate(50_000);
        }
        agg.latencies_us.extend(latencies);
    }
}

fn percentile(latencies_us: &mut [u128], p: f64) -> Option<f64> {
    if latencies_us.is_empty() {
        return None;
    }
    latencies_us.sort_unstable();
    let rank = (p * (latencies_us.len() as f64 - 1.0)).round() as usize;
    Some(latencies_us[rank] as f64 / 1000.0) // return in milliseconds
}

fn mean_ms(latencies_us: &[u128]) -> Option<f64> {
    if latencies_us.is_empty() {
        return None;
    }
    let sum: u128 = latencies_us.iter().copied().sum();
    Some((sum as f64) / (latencies_us.len() as f64) / 1000.0)
}

fn build_random_order() -> Order {
    let mut rng = rand::thread_rng();
    let num_items = rng.gen_range(1..=3);
    let mut items = Vec::with_capacity(num_items);
    for _ in 0..num_items {
        items.push(OrderItem {
            product_id: format!("prod_{:04}", rng.gen_range(1..=5000)),
            quantity: rng.gen_range(1..=5),
            unit_price: None, // keep message light; server ignores it currently
        });
    }

    Order {
        id: String::new(),
        customer_id: format!("customer_{:06}", rng.gen_range(1..=1_000_000)),
        items,
        total_amount: None,
        status: OrderStatus::Pending as i32,
        created_at: None,
        shipping_address: None,
        billing_address: None,
        payment_method_id: String::new(),
        shipment_id: String::new(),
    }
}

async fn worker_grpc(target: String, end_time: Instant, metrics: Arc<Mutex<Metrics>>) {
    // Create a dedicated client per worker
    let channel = match Endpoint::from_shared(target.clone()) {
        Ok(ep) => match ep.connect().await {
            Ok(ch) => ch,
            Err(_) => {
                let mut m = metrics.lock().unwrap();
                m.failures += 1;
                return;
            }
        },
        Err(_) => {
            let mut m = metrics.lock().unwrap();
            m.failures += 1;
            return;
        }
    };

    let mut client = OrderServiceClient::new(channel);

    while Instant::now() < end_time {
        let order = build_random_order();
        let req = CreateOrderRequest { order: Some(order) };
        let start = Instant::now();
        let res = client.create_order(req).await;
        let elapsed = start.elapsed();
        let mut m = metrics.lock().unwrap();
        match res {
            Ok(_) => {
                m.successes += 1;
                m.latencies_us.push(elapsed.as_micros());
            }
            Err(_) => {
                m.failures += 1;
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let cfg = BenchConfig::from_env_or_args();
    println!(
        "Running orders throughput benchmark: mode={}, target={}, duration={}s, warmup={}s, concurrency={}",
        cfg.mode, cfg.target, cfg.duration_secs, cfg.warmup_secs, cfg.concurrency
    );

    // Warmup
    if cfg.warmup_secs > 0 {
        let warmup_end = Instant::now() + Duration::from_secs(cfg.warmup_secs);
        let warmup_metrics = Arc::new(Mutex::new(Metrics::default()));
        let mut warmup_tasks = JoinSet::new();
        for _ in 0..cfg.concurrency.min(8) { // keep warmup light
            let target = cfg.target.clone();
            let wm = warmup_metrics.clone();
            warmup_tasks.spawn(worker_grpc(target, warmup_end, wm));
        }
        while let Some(_) = warmup_tasks.join_next().await {}
    }

    // Run benchmark
    let end_time = Instant::now() + Duration::from_secs(cfg.duration_secs);
    let metrics = Arc::new(Mutex::new(Metrics::default()));
    let mut tasks = JoinSet::new();

    match cfg.mode.as_str() {
        "grpc" => {
            for _ in 0..cfg.concurrency {
                let target = cfg.target.clone();
                let m = metrics.clone();
                tasks.spawn(worker_grpc(target, end_time, m));
            }
        }
        other => {
            eprintln!("Unsupported mode: {} (only 'grpc' is implemented in this bench)", other);
            return;
        }
    }

    while let Some(_) = tasks.join_next().await {}

    // Aggregate and report
    let final_metrics = metrics.lock().unwrap().clone();

    let successes = final_metrics.successes;
    let failures = final_metrics.failures;
    let total = successes + failures;
    let rps = successes as f64 / (cfg.duration_secs as f64);

    let mut lats = final_metrics.latencies_us.clone();
    let avg_ms = mean_ms(&lats).unwrap_or(0.0);
    let p50_ms = percentile(&mut lats, 0.50).unwrap_or(0.0);
    let p95_ms = percentile(&mut lats, 0.95).unwrap_or(0.0);
    let p99_ms = percentile(&mut lats, 0.99).unwrap_or(0.0);

    println!("\nResults:");
    println!("  Total requests: {} (successes={} failures={})", total, successes, failures);
    println!("  Throughput: {:.2} orders/sec", rps);
    println!("  Latency (ms): avg={:.2} p50={:.2} p95={:.2} p99={:.2}", avg_ms, p50_ms, p95_ms, p99_ms);
}
#![cfg(feature = "demos")]
