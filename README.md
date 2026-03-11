<div align="center">

# httpress

[![Crates.io](https://img.shields.io/crates/v/httpress.svg)](https://crates.io/crates/httpress)
[![Documentation](https://docs.rs/httpress/badge.svg)](https://docs.rs/httpress)
[![License](https://img.shields.io/crates/l/httpress.svg)](https://github.com/TecuceanuGabriel/httpress)

A fast, flexible HTTP benchmarking library and CLI tool built in Rust.

[Features](#features) • [Installation](#installation) • [Quick Start](#quick-start) • [Library Usage](#library-usage) • [CLI Usage](#cli-usage) • [Examples](#examples)

</div>

## Features

- **Simple Builder API** - Fluent, type-safe configuration
- **High Performance** - Async Rust with minimal overhead
- **Flexible Rate Control** - Fixed rates or dynamic rate functions
- **Custom Request Generation** - Generate requests dynamically per-worker
- **Hook System** - Inject custom logic before/after requests
- **Detailed Metrics** - Latency percentiles, throughput, status codes
- **Concurrent Workers** - Configurable parallelism
- **Adaptive Testing** - Duration-based or request-count-based
- **Retry Logic** - Smart retry with hook-based control
- **Library + CLI** - Use as Rust library or standalone tool

## Installation

### As a Library

Add to your `Cargo.toml`:

```toml
[dependencies]
httpress = "0.5"
tokio = { version = "1", features = ["full"] }
```

### As a CLI Tool

```bash
cargo install httpress
```

## Quick Start

```rust
use httpress::Benchmark;
use std::time::Duration;

let results = Benchmark::builder()
    .url("http://localhost:3000")
    .concurrency(50)
    .duration(Duration::from_secs(10))
    .show_progress(true)
    .build()?
    .run()
    .await?;

results.print();
```

## Library Usage

### Basic Benchmark

```rust
let results = Benchmark::builder()
    .url("http://localhost:3000")
    .concurrency(50)
    .requests(1000)
    .show_progress(true)
    .build()?
    .run()
    .await?;
```

### Custom Request Generation

```rust
.request_fn(|ctx: RequestContext| {
    let user_id = ctx.request_number % 100;
    RequestConfig {
        url: format!("http://localhost:3000/user/{}", user_id),
        method: HttpMethod::Get,
        headers: HashMap::new(),
        body: None,
    }
})
```

### Dynamic Rate Control

```rust
.rate_fn(|ctx: RateContext| {
    let progress = (ctx.elapsed.as_secs_f64() / 10.0).min(1.0);
    100.0 + (900.0 * progress)  // Ramp from 100 to 1000 req/s
})
```

### Hook System

```rust
.after_request(|ctx: AfterRequestContext| {
    if let Some(status) = ctx.status {
        if status >= 500 {
            return HookAction::Retry;
        }
    }
    HookAction::Continue
})
```

For complete API documentation, see [docs.rs/httpress](https://docs.rs/httpress).

## CLI Usage

### Basic Examples

```bash
# Run benchmark with 100 concurrent connections for 30 seconds
httpress http://example.com -c 100 -d 30s

# Fixed number of requests with rate limiting
httpress http://example.com -n 10000 -r 1000

# POST request with headers and body
httpress http://example.com/api -m POST \
  -H "Content-Type: application/json" \
  -b '{"key": "value"}'
```

### Options

| Flag                | Description                  | Default |
| ------------------- | ---------------------------- | ------- |
| `-n, --requests`    | Total number of requests     | -       |
| `-d, --duration`    | Test duration (e.g. 10s, 1m) | -       |
| `-c, --concurrency` | Concurrent connections       | 10      |
| `-r, --rate`        | Rate limit (req/s)           | -       |
| `-m, --method`      | HTTP method                  | GET     |
| `-H, --header`      | HTTP header (repeatable)     | -       |
| `-b, --body`        | Request body                 | -       |
| `-t, --timeout`     | Request timeout in seconds   | 30      |

### Example Output

```
--- Benchmark Complete ---
Requests:     1000 total, 1000 success, 0 errors
Duration:     0.06s
Throughput:   16185.07 req/s

Latency:
  Min:    245us
  Max:    2.41ms
  Mean:   612us
  p50:    544us
  p90:    955us
  p95:    1.08ms
  p99:    1.61ms

Status codes:
  200: 1000
```

## Examples

The `examples/` directory contains:

- **basic_benchmark.rs** - Simple benchmark example
- **custom_requests.rs** - Dynamic request generation with request_fn
- **rate_ramping.rs** - Rate control with rate_fn
- **hooks_metrics.rs** - Custom metrics collection using hooks

Run examples with:

```bash
cargo run --example basic_benchmark
```
