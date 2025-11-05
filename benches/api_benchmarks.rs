use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::time::Duration;

// Benchmark for order creation performance
fn order_creation_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("order_creation");

    for size in [1, 5, 10, 20].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter(|| {
                // Simulate order creation with multiple items
                let mut total = 0;
                for i in 0..size {
                    total += black_box(i * 2);
                }
                total
            });
        });
    }

    group.finish();
}

// Benchmark for inventory allocation
fn inventory_allocation_benchmark(c: &mut Criterion) {
    c.bench_function("inventory_allocation", |b| {
        b.iter(|| {
            // Simulate inventory allocation logic
            let quantity = black_box(100);
            let reserved = black_box(25);
            let available = quantity - reserved;
            black_box(available)
        });
    });
}

// Benchmark for JSON serialization/deserialization
fn json_serialization_benchmark(c: &mut Criterion) {
    use serde_json::json;

    let data = json!({
        "order_id": "550e8400-e29b-41d4-a716-446655440000",
        "customer_id": "123e4567-e89b-12d3-a456-426614174000",
        "status": "pending",
        "total": 199.99,
        "items": [
            {
                "product_id": "prod_001",
                "quantity": 2,
                "price": 49.99
            },
            {
                "product_id": "prod_002",
                "quantity": 1,
                "price": 99.99
            }
        ]
    });

    c.bench_function("json_serialize", |b| {
        b.iter(|| {
            let serialized = serde_json::to_string(&data).unwrap();
            black_box(serialized)
        });
    });

    c.bench_function("json_deserialize", |b| {
        let serialized = serde_json::to_string(&data).unwrap();
        b.iter(|| {
            let deserialized: serde_json::Value = serde_json::from_str(&serialized).unwrap();
            black_box(deserialized)
        });
    });
}

// Benchmark for UUID generation
fn uuid_generation_benchmark(c: &mut Criterion) {
    use uuid::Uuid;

    c.bench_function("uuid_v4_generation", |b| {
        b.iter(|| {
            let id = Uuid::new_v4();
            black_box(id)
        });
    });
}

// Benchmark for string operations (common in API processing)
fn string_operations_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("string_operations");

    let test_string = "ORDER-2024-11-03-12345";

    group.bench_function("string_clone", |b| {
        b.iter(|| {
            let cloned = test_string.to_string();
            black_box(cloned)
        });
    });

    group.bench_function("string_format", |b| {
        b.iter(|| {
            let formatted = format!("ORDER-{}-{}", "2024-11-03", "12345");
            black_box(formatted)
        });
    });

    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .measurement_time(Duration::from_secs(10))
        .sample_size(100);
    targets =
        order_creation_benchmark,
        inventory_allocation_benchmark,
        json_serialization_benchmark,
        uuid_generation_benchmark,
        string_operations_benchmark
}

criterion_main!(benches);
