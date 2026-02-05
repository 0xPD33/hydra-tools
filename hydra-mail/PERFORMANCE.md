# Hydra Mail Performance

Benchmark results for hydra-mail's in-memory pub/sub system.

**TL;DR**: Sub-millisecond latency, millions of messages per second on typical hardware.

## Environment

- **CPU**: AMD Ryzen 9 9950X3D (32 cores) @ 5.7GHz
- **OS**: Linux (NixOS)
- **Rust**: 1.93.0-nightly (2025-11-11)
- **Framework**: Criterion.rs

> Results are hardware-dependent. Run `cargo bench` on your system.

## Summary

| Operation | Result |
|-----------|--------|
| Emit latency | 108 ns |
| Subscribe latency | 43 µs |
| Roundtrip | 2.4 µs |
| Peak throughput | 10.2M msgs/sec |

## Detailed Results

### Core Operations

```
emit_and_store:    108 ns    (publish + buffer update)
subscribe:         43 µs     (setup + history retrieval)
roundtrip:         2.4 µs    (emit → receive)
```

### Throughput (batch emit)

| Messages | Throughput | Total Time |
|----------|------------|------------|
| 100 | 10.0M/sec | 10 µs |
| 1,000 | 10.2M/sec | 98 µs |
| 10,000 | 10.2M/sec | 985 µs |

### Concurrency

**Parallel tasks (same channel):**
- 2 tasks: 3.2 µs
- 4 tasks: 3.7 µs
- 8 tasks: 5.1 µs
- 16 tasks: 9.6 µs

**Multi-project (isolated channels):**
- 5 projects: 4.0 µs
- 10 projects: 7.5 µs
- 20 projects: 11.5 µs

**Multiple subscribers:**
- Performance independent of subscriber count (Tokio broadcast uses atomics)
- 1 subscriber: 119 ns
- 50 subscribers: 117 ns

### Message Size

| Size | Latency | Throughput |
|------|---------|------------|
| 32B | 105 ns | 289 MB/s |
| 256B | 107 ns | 2.2 GB/s |
| 1KB | 155 ns | 6.2 GB/s |
| 4KB | 172 ns | 22.2 GB/s |

Message size has minimal impact - bottleneck is synchronization, not data.

### Replay Buffer

| Buffer Size | Retrieval Time |
|-------------|----------------|
| 10 msgs | 210 ns |
| 50 msgs | 1.3 µs |
| 100 msgs | 2.4 µs |

### Encoding (JSON baseline)

```
JSON encode:  99 ns   (to string)
JSON decode:  205 ns  (from string)

Message size: 171 bytes (compact)
```

### Real-World Scenarios

```
Slow consumer catchup (100 msgs):   17.1 µs
Realistic workflow (4 msg burst):   406 ns
Channel churn (create/destroy):     49.5 µs
List channels (100 channels):       4.9 ms
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
- Latency: Claimed <5ms, measured 108ns (46,000x better)
- Throughput: Measured 10.2M/sec

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

**Version**: v0.1.0 • **Updated**: 2026-02-04
