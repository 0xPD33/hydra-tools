# Hydra Mail Performance

Benchmark results for hydra-mail's in-memory pub/sub system.

## Environment

- **CPU**: AMD Ryzen 9 9950X3D (32 cores) @ 5.7GHz
- **OS**: Linux (NixOS)
- **Rust**: 1.93.0-nightly (2025-11-11)
- **Framework**: Criterion.rs

> Results are hardware-dependent. Run `cargo bench` on your system.

## Summary

| Operation | Result |
|-----------|--------|
| Emit latency | 100 ns |
| Subscribe latency | 63 µs |
| Roundtrip | 2.7 µs |
| Peak throughput | 10.6M msgs/sec |

## Detailed Results

### Core Operations

```
emit_and_store:    99.7 ns   (publish + buffer update)
subscribe:         63.1 µs   (setup + history retrieval)
roundtrip:         2.7 µs    (emit → receive)
```

### Throughput (batch emit)

| Messages | Throughput | Total Time |
|----------|------------|------------|
| 100 | 10.2M/sec | 9.9 µs |
| 1,000 | 10.4M/sec | 96 µs |
| 10,000 | 10.6M/sec | 946 µs |

### Concurrency

**Parallel tasks (same channel):**
- 2 tasks: 3.2 µs
- 4 tasks: 3.6 µs
- 8 tasks: 5.6 µs
- 16 tasks: 10.9 µs

**Multi-project (isolated channels):**
- 5 projects: 4.4 µs
- 10 projects: 8.1 µs
- 20 projects: 14.6 µs

**Multiple subscribers:**
- Performance independent of subscriber count (Tokio broadcast uses atomics)
- 1 subscriber: 120 ns
- 50 subscribers: 112 ns

### Message Size

| Size | Latency | Throughput |
|------|---------|------------|
| 32B | 98 ns | 309 MB/s |
| 256B | 102 ns | 2.3 GB/s |
| 1KB | 151 ns | 6.3 GB/s |
| 4KB | 167 ns | 22.8 GB/s |

Message size has minimal impact - bottleneck is synchronization, not data.

### Replay Buffer

| Buffer Size | Retrieval Time |
|-------------|----------------|
| 10 msgs | 221 ns |
| 50 msgs | 1.3 µs |
| 100 msgs | 2.5 µs |

### Encoding (JSON baseline)

```
JSON encode:  104 ns  (to string)
JSON decode:  214 ns  (from string)

Message size: 171 bytes (compact)
```

### Real-World Scenarios

```
Slow consumer catchup (100 msgs):   17.1 µs
Realistic workflow (4 msg burst):   443 ns
Channel churn (create/destroy):     45.8 µs
List channels (100 channels):       2.3 ms
```

## Observations

**Scaling:**
- Linear with concurrent tasks up to 8, slight contention at 16+
- Linear with multiple projects (HashMap lookup overhead)
- Message size has <70% impact on latency

**Bottlenecks:**
- Mutex contention at high concurrency
- HashMap lookups for channel routing
- String allocation for message cloning

**Comparison to claims:**
- Latency: Claimed <5ms, measured 100ns (50,000x better)
- Throughput: Claimed 1M+/sec, measured 10.6M/sec (10x better)

## Running Benchmarks

```bash
# Full suite (~5 minutes)
cargo bench

# Quick check
cargo bench -- --sample-size 10

# Specific benchmark
cargo bench emit_latency

# Save baseline for comparison
cargo bench --save-baseline main
```

## Methodology

- Statistical analysis with 100 samples per benchmark
- 3 second warm-up per test
- Outlier detection and removal
- Wall-clock time measurement

Typical variance: <5% standard deviation

---

**Version**: v0.1.0 • **Updated**: 2025-12-18
