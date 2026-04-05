<div align="center">

# httpress

[![Crates.io](https://img.shields.io/crates/v/httpress.svg)](https://crates.io/crates/httpress)
[![Downloads](https://img.shields.io/crates/d/httpress.svg)](https://crates.io/crates/httpress)
[![Documentation](https://docs.rs/httpress/badge.svg)](https://docs.rs/httpress)
[![CI](https://github.com/GabrielTecuceanu/httpress/actions/workflows/ci.yml/badge.svg)](https://github.com/GabrielTecuceanu/httpress/actions/workflows/ci.yml)
[![License](https://img.shields.io/crates/l/httpress.svg)](https://github.com/GabrielTecuceanu/httpress)

a fast HTTP benchmarking library and CLI tool

![demo](/assets/demo.gif)

</div>

## Contents

- [Features](#features)
- [Performance](#performance)
- [Installation](#installation)
- [Quick Start](#quick-start)
- [Library Usage](#library-usage)
  - [Basic Benchmark](#basic-benchmark)
  - [Custom Request Generation](#custom-request-generation)
  - [Dynamic Rate Control](#dynamic-rate-control)
  - [Hook System](#hook-system)
- [CLI Usage](#cli-usage)
  - [Basic Examples](#basic-examples)
  - [Options](#options)
- [Examples](#examples)
- [Roadmap](#roadmap)

## Features

- **High Performance** - async rust with minimal overhead
- **Detailed Metrics** - latency percentiles, throughput, status code breakdown
- **Flexible Stop Conditions** - duration, request-count or infinite
- **Concurrent Workers** - configure the number of concurrent connections
- **Library + CLI** - use as a rust library or standalone tool
- **Flexible Rate Control** - use fixed rates or dynamic rate functions
- **Custom Request Generation** - generate requests dynamically per-worker
- **Hook System** - inject custom logic before/after each request
- **Simple Builder API** - easy to use, type-safe configuration

## Performance

- Benchmarks run against a local [hyper](https://hyper.rs/) echo server.
- Environment: Intel Core i5-8350U, Arch Linux x86_64.

### Scaling

| Concurrency | Throughput   | p50     | p99     |
| ----------- | ------------ | ------- | ------- |
| 5           | 17,315 req/s | 0.25 ms | 0.57 ms |
| 10          | 20,909 req/s | 0.39 ms | 1.10 ms |
| 25          | 29,071 req/s | 0.70 ms | 1.64 ms |
| 50          | 32,000 req/s | 1.25 ms | 2.90 ms |
| 100         | 32,502 req/s | 2.42 ms | 6.03 ms |

### Comparison to other similar tools (concurrency=100, 10 s)

| Tool     | Throughput   | p50     | p99     |
| -------- | ------------ | ------- | ------- |
| httpress | 32,502 req/s | 2.42 ms | 6.03 ms |
| wrk      | 39,135 req/s | 2.47 ms | 5.42 ms |
| hey      | 27,182 req/s | 3.50 ms | 9.40 ms |

### Sample output

```
Target: http://localhost:3000 Get
Concurrency: 100
Stop condition: Duration(10s)

Starting benchmark with 100 workers...

--- Benchmark Complete ---
Requests:     325107 total, 325107 success, 0 errors
Duration:     10.00s
Throughput:   32502.23 req/s
Transferred:  0.62 MB

Latency:
  Min:    175us
  Max:    45.12ms
  Mean:   2.52ms
  p50:    2.42ms
  p90:    3.91ms
  p95:    4.43ms
  p99:    6.03ms

Status codes:
  200: 325107
```

### JSON output

```bash
httpress http://localhost:3000 -c 100 -d 10s -o json
```

```json
{
  "total_requests": 325107,
  "successful_requests": 325107,
  "failed_requests": 0,
  "duration": "10.00s",
  "throughput": 32502.23,
  "latency_min": "175us",
  "latency_max": "45.12ms",
  "latency_mean": "2.52ms",
  "latency_p50": "2.42ms",
  "latency_p90": "3.91ms",
  "latency_p95": "4.43ms",
  "latency_p99": "6.03ms",
  "status_codes": {
    "200": 325107
  },
  "total_bytes": 650214
}
```

## Installation

### As a CLI Tool

```bash
cargo install httpress
```

#### Shell Completions

`httpress` can generate shell completion scripts for your shell of choice.
Once loaded, pressing `Tab` will autocomplete subcommands, flags, and option values.

```bash
# Bash: Add to your .bashrc
source <(httpress completions bash)

# Zsh: Add to your .zshrc
source <(httpress completions zsh)

# Fish: Add to your ~/.config/fish/config.fish
httpress completions fish | source

# PowerShell: Add to your profile ($PROFILE)
Invoke-Expression (& httpress completions powershell)

# Elvish: Add to your ~/.config/elvish/rc.elv
eval (httpress completions elvish | slurp)
```

### As a Library

Add to your `Cargo.toml`:

```toml
[dependencies]
httpress = "0.6"
tokio = { version = "1", features = ["full"] }
```

## Quick Start

```rust
use httpress::{Benchmark, Result};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
    let results = Benchmark::builder()
        .url("http://localhost:3000")
        .concurrency(50)
        .duration(Duration::from_secs(10))
        .show_progress(true)
        .build()?
        .run()
        .await?;

    results.print();
    Ok(())
}
```

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

# Run until interrupted (Ctrl+C)
httpress http://example.com -c 50

# Output results as JSON
httpress http://example.com -c 100 -d 10s -o json
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
| `-o, --output`      | Output format (text, json)   | text    |
| `-t, --timeout`     | Request timeout in seconds   | 30      |
| `-k, --insecure`    | Skip TLS verification        | false   |


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
    100.0 + (900.0 * progress)  // ramp from 100 to 1000 req/s
})
```

### Hook System

#### Before-request

You can use these for circuit breakers or conditional execution:

```rust
.before_request(|ctx: BeforeRequestContext| {
    let failure_rate = ctx.failed_requests as f64 / ctx.total_requests.max(1) as f64;
    if failure_rate > 0.5 && ctx.total_requests > 100 {
        HookAction::Abort
    } else {
        HookAction::Continue
    }
})
```

#### After-request

You can use them to collect custom metrics or write retry logic:

```rust
.after_request(|ctx: AfterRequestContext| {
    if let Some(status) = ctx.status {
        if status >= 500 {
            return HookAction::Retry;
        }
    }
    HookAction::Continue
})
.max_retries(3)
```

For complete API documentation, see [docs.rs/httpress](https://docs.rs/httpress).

## Examples

The `examples/` directory contains:

- [basic_benchmark.rs](examples/basic_benchmark.rs) - basic benchmark example
- [custom_requests.rs](examples/custom_requests.rs) - dynamic request generation using `request_fn`
- [rate_ramping.rs](examples/rate_ramping.rs) - advanced rate control using `rate_fn`
- [hooks_metrics.rs](examples/hooks_metrics.rs) - custom metrics collection using hooks
- [test_server.rs](examples/test_server.rs) - local axum test server used by the other examples

Run examples with:

```bash
cargo run --example basic_benchmark
```

## Real-World Example

[httpress-example](https://github.com/GabrielTecuceanu/httpress-example) - an
axum key-value store server that uses httpress in its test suite for regression
testing, with a fully configured CI pipeline.

## Roadmap

- [ ] Coordinated omission correction
- [ ] HDR histogram
- [ ] HTTP/2 support
- [ ] Latency breakdown (DNS, TCP connect, TLS, TTFB)
- [ ] Warm-up period
- [x] Structured output (JSON)
- [ ] Multi-step scenarios
- [ ] HTTP/3 support
