//! Benchmarks for hydra-mail channel operations
//!
//! Run with: cargo bench
//! Results validate the claimed performance characteristics:
//! - <5ms latency for message delivery
//! - High throughput for broadcast operations

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput, BenchmarkId};
use hydra_mail::channels::{emit_and_store, subscribe_broadcast, get_or_create_broadcast_tx};
use uuid::Uuid;
use tokio::runtime::Runtime;

/// Benchmark emit_and_store latency (single message)
fn bench_emit_latency(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let uuid = Uuid::new_v4();
    let topic = "bench:emit";
    let message = r#"{"action":"updated","target":"file.rs","impact":"refactored module"}"#.to_string();

    c.bench_function("emit_and_store_latency", |b| {
        b.to_async(&rt).iter(|| async {
            emit_and_store(black_box(uuid), black_box(topic), black_box(message.clone())).await
        })
    });
}

/// Benchmark subscribe_broadcast latency (new subscriber)
fn bench_subscribe_latency(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("subscribe_broadcast_latency", |b| {
        b.to_async(&rt).iter(|| async {
            // Use unique UUID each time to avoid accumulating channels
            let uuid = Uuid::new_v4();
            let topic = "bench:subscribe";
            subscribe_broadcast(black_box(uuid), black_box(topic)).await
        })
    });
}

/// Benchmark emit throughput (messages per second)
fn bench_emit_throughput(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let uuid = Uuid::new_v4();
    let topic = "bench:throughput";
    let message = r#"{"action":"updated","target":"file.rs"}"#.to_string();

    let mut group = c.benchmark_group("emit_throughput");

    for batch_size in [100, 1000, 10000].iter() {
        group.throughput(Throughput::Elements(*batch_size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            batch_size,
            |b, &size| {
                b.to_async(&rt).iter(|| async {
                    for _ in 0..size {
                        emit_and_store(uuid, topic, message.clone()).await;
                    }
                })
            },
        );
    }
    group.finish();
}

/// Benchmark round-trip latency (emit + receive)
fn bench_roundtrip_latency(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let uuid = Uuid::new_v4();
    let topic = "bench:roundtrip";
    let message = r#"{"action":"test"}"#.to_string();

    // Pre-create channel and subscriber
    rt.block_on(async {
        let _ = get_or_create_broadcast_tx(uuid, topic).await;
    });

    c.bench_function("roundtrip_latency", |b| {
        b.to_async(&rt).iter(|| async {
            // Subscribe and emit
            let (mut rx, _history) = subscribe_broadcast(uuid, topic).await;
            emit_and_store(uuid, topic, message.clone()).await;
            let _ = rx.recv().await;
        })
    });
}

/// Benchmark with active subscribers (realistic scenario)
fn bench_emit_with_subscribers(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let uuid = Uuid::new_v4();
    let topic = "bench:with_subs";
    let message = r#"{"action":"updated","target":"file.rs"}"#.to_string();

    let mut group = c.benchmark_group("emit_with_subscribers");

    for num_subscribers in [1, 10, 50].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_subscribers),
            num_subscribers,
            |b, &n| {
                // Create subscribers before benchmark
                let _subscribers: Vec<_> = rt.block_on(async {
                    let mut subs = Vec::new();
                    for _ in 0..n {
                        let (rx, _) = subscribe_broadcast(uuid, topic).await;
                        subs.push(rx);
                    }
                    subs
                });

                b.to_async(&rt).iter(|| async {
                    emit_and_store(uuid, topic, message.clone()).await
                })
            },
        );
    }
    group.finish();
}

/// Benchmark replay buffer retrieval
fn bench_replay_buffer(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("replay_buffer");

    for buffer_fill in [10, 50, 100].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(buffer_fill),
            buffer_fill,
            |b, &fill| {
                // Use unique channel per benchmark to avoid interference
                let uuid = Uuid::new_v4();
                let topic = format!("bench:replay:{}", fill);
                let message = r#"{"data":"test"}"#.to_string();

                // Pre-fill buffer
                rt.block_on(async {
                    for _ in 0..fill {
                        emit_and_store(uuid, &topic, message.clone()).await;
                    }
                });

                b.to_async(&rt).iter(|| async {
                    let (_rx, history) = subscribe_broadcast(uuid, &topic).await;
                    black_box(history)
                })
            },
        );
    }
    group.finish();
}

/// Benchmark message size impact
fn bench_message_sizes(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let uuid = Uuid::new_v4();

    let mut group = c.benchmark_group("message_sizes");

    // Different message sizes in bytes
    let sizes = [
        ("tiny_32b", 32),
        ("small_256b", 256),
        ("medium_1kb", 1024),
        ("large_4kb", 4096),
    ];

    for (name, size) in sizes.iter() {
        let topic = format!("bench:size:{}", name);
        // Create message of approximately the target size
        let message = format!(r#"{{"data":"{}"}}"#, "x".repeat(*size));

        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(name),
            &message,
            |b, msg| {
                b.to_async(&rt).iter(|| async {
                    emit_and_store(uuid, &topic, msg.clone()).await
                })
            },
        );
    }
    group.finish();
}

/// Compare JSON serialization overhead (baseline for TOON comparison)
fn bench_json_serialization(c: &mut Criterion) {
    use serde::{Serialize, Deserialize};

    #[derive(Serialize, Deserialize)]
    struct TestMessage {
        action: String,
        target: String,
        impact: String,
        timestamp: u64,
    }

    let msg = TestMessage {
        action: "updated".to_string(),
        target: "src/main.rs".to_string(),
        impact: "refactored error handling".to_string(),
        timestamp: 1234567890,
    };

    let mut group = c.benchmark_group("serialization");

    group.bench_function("json_serialize", |b| {
        b.iter(|| serde_json::to_string(black_box(&msg)).unwrap())
    });

    let json_str = serde_json::to_string(&msg).unwrap();
    group.bench_function("json_deserialize", |b| {
        b.iter(|| serde_json::from_str::<TestMessage>(black_box(&json_str)).unwrap())
    });

    // Report sizes for reference
    println!("\nJSON message size: {} bytes", json_str.len());

    group.finish();
}

/// Benchmark concurrent emits from multiple tasks
fn bench_concurrent_emits(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let uuid = Uuid::new_v4();
    let topic = "bench:concurrent";
    let message = r#"{"action":"test"}"#.to_string();

    let mut group = c.benchmark_group("concurrent_emits");

    for num_tasks in [2, 4, 8, 16].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_tasks),
            num_tasks,
            |b, &n| {
                b.to_async(&rt).iter(|| async {
                    let mut handles = Vec::new();
                    for _ in 0..n {
                        let msg = message.clone();
                        handles.push(tokio::spawn(async move {
                            emit_and_store(uuid, topic, msg).await
                        }));
                    }
                    for handle in handles {
                        let _ = handle.await;
                    }
                })
            },
        );
    }
    group.finish();
}

/// Benchmark multi-project isolation overhead
fn bench_multi_project(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let message = r#"{"action":"test"}"#.to_string();

    let mut group = c.benchmark_group("multi_project_isolation");

    for num_projects in [1, 5, 10, 20].iter() {
        let projects: Vec<Uuid> = (0..*num_projects).map(|_| Uuid::new_v4()).collect();

        group.bench_with_input(
            BenchmarkId::from_parameter(num_projects),
            &projects,
            |b, projs| {
                b.to_async(&rt).iter(|| async {
                    // Emit to all projects simultaneously
                    let mut handles = Vec::new();
                    for &uuid in projs.iter() {
                        let msg = message.clone();
                        handles.push(tokio::spawn(async move {
                            emit_and_store(uuid, "test", msg).await
                        }));
                    }
                    for handle in handles {
                        let _ = handle.await;
                    }
                })
            },
        );
    }
    group.finish();
}

/// Benchmark subscriber catching up (slow consumer scenario)
fn bench_slow_consumer(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let uuid = Uuid::new_v4();
    let topic = "bench:slow";
    let message = r#"{"action":"test"}"#.to_string();

    c.bench_function("slow_consumer_catchup", |b| {
        b.to_async(&rt).iter(|| async {
            // Create subscriber
            let (mut rx, _) = subscribe_broadcast(uuid, topic).await;

            // Emit 100 messages rapidly
            for _ in 0..100 {
                emit_and_store(uuid, topic, message.clone()).await;
            }

            // Consumer catches up
            let mut count = 0;
            while let Ok(_) = rx.try_recv() {
                count += 1;
            }
            black_box(count)
        })
    });
}

/// Benchmark burst throughput (rapid fire emissions)
fn bench_burst_throughput(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let uuid = Uuid::new_v4();
    let message = r#"{"data":"x"}"#.to_string();

    let mut group = c.benchmark_group("burst_throughput");

    for burst_size in [100, 1000, 5000].iter() {
        let topic = format!("bench:burst:{}", burst_size);
        group.throughput(Throughput::Elements(*burst_size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(burst_size),
            burst_size,
            |b, &size| {
                b.to_async(&rt).iter(|| async {
                    // Single subscriber ready to receive
                    let (_rx, _) = subscribe_broadcast(uuid, &topic).await;

                    // Burst emit
                    for _ in 0..size {
                        emit_and_store(uuid, &topic, message.clone()).await;
                    }
                })
            },
        );
    }
    group.finish();
}

/// Benchmark channel list operation
fn bench_list_channels(c: &mut Criterion) {
    use hydra_mail::channels::list_channels;

    let rt = Runtime::new().unwrap();
    let uuid = Uuid::new_v4();

    let mut group = c.benchmark_group("list_channels");

    for num_channels in [10, 50, 100].iter() {
        // Pre-create channels
        rt.block_on(async {
            for i in 0..*num_channels {
                let topic = format!("chan:{}", i);
                emit_and_store(uuid, &topic, "init".to_string()).await;
            }
        });

        group.bench_with_input(
            BenchmarkId::from_parameter(num_channels),
            num_channels,
            |b, _| {
                b.to_async(&rt).iter(|| async {
                    list_channels(uuid).await
                })
            },
        );
    }
    group.finish();
}

/// Benchmark JSON encoding performance (baseline for format comparison)
fn bench_encoding_formats(c: &mut Criterion) {
    use serde::{Serialize, Deserialize};
    use std::collections::HashMap;

    #[derive(Serialize, Deserialize, Clone)]
    struct RealisticMessage {
        action: String,
        target: String,
        impact: String,
        timestamp: u64,
        metadata: HashMap<String, String>,
    }

    let mut metadata = HashMap::new();
    metadata.insert("author".to_string(), "developer".to_string());
    metadata.insert("branch".to_string(), "feature/new-api".to_string());

    let msg = RealisticMessage {
        action: "updated".to_string(),
        target: "src/main.rs".to_string(),
        impact: "refactored error handling module".to_string(),
        timestamp: 1234567890,
        metadata,
    };

    let mut group = c.benchmark_group("encoding_formats");

    // JSON encoding
    group.bench_function("json_encode", |b| {
        b.iter(|| {
            serde_json::to_string(black_box(&msg)).unwrap()
        })
    });

    let json_encoded = serde_json::to_string(&msg).unwrap();

    // JSON decoding
    group.bench_function("json_decode", |b| {
        b.iter(|| {
            serde_json::from_str::<RealisticMessage>(black_box(&json_encoded)).unwrap()
        })
    });

    // Compact JSON (no whitespace)
    group.bench_function("json_compact", |b| {
        b.iter(|| {
            serde_json::to_vec(black_box(&msg)).unwrap()
        })
    });

    println!("\n--- Message Size Comparison ---");
    println!("JSON (pretty): {} bytes", serde_json::to_string_pretty(&msg).unwrap().len());
    println!("JSON (compact): {} bytes", json_encoded.len());
    println!("JSON (binary): {} bytes", serde_json::to_vec(&msg).unwrap().len());

    group.finish();
}

/// Benchmark with realistic message patterns
fn bench_realistic_patterns(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let uuid = Uuid::new_v4();

    // Simulate realistic development workflow messages
    let messages = vec![
        r#"{"action":"file_changed","path":"src/main.rs","bytes":1234}"#,
        r#"{"action":"test_passed","suite":"integration","duration_ms":450}"#,
        r#"{"action":"build_complete","target":"debug","artifacts":["bin/hydra-mail"]}"#,
        r#"{"action":"lint_warning","file":"src/config.rs","line":42,"message":"unused import"}"#,
    ];

    c.bench_function("realistic_workflow", |b| {
        b.to_async(&rt).iter(|| async {
            // Simulate a burst of activity
            for msg in &messages {
                emit_and_store(uuid, "repo:delta", msg.to_string()).await;
            }
        })
    });
}

/// Benchmark cleanup overhead when channels are removed
fn bench_channel_churn(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let message = r#"{"data":"test"}"#.to_string();

    c.bench_function("channel_create_destroy_churn", |b| {
        b.to_async(&rt).iter(|| async {
            // Create unique channel each iteration
            let uuid = Uuid::new_v4();
            let topic = "temp";

            // Create, use, and implicitly drop
            let (mut rx, _) = subscribe_broadcast(uuid, topic).await;
            emit_and_store(uuid, topic, message.clone()).await;
            let _ = rx.recv().await;
            // rx dropped here, channel reference count decreases
        })
    });
}

criterion_group!(
    benches,
    bench_emit_latency,
    bench_subscribe_latency,
    bench_emit_throughput,
    bench_roundtrip_latency,
    bench_emit_with_subscribers,
    bench_replay_buffer,
    bench_message_sizes,
    bench_json_serialization,
    bench_concurrent_emits,
    bench_multi_project,
    bench_slow_consumer,
    bench_burst_throughput,
    bench_list_channels,
    bench_encoding_formats,
    bench_realistic_patterns,
    bench_channel_churn,
);

criterion_main!(benches);
