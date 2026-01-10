# httpress

A fast HTTP benchmarking tool built in Rust.

## Installation

```bash
cargo install httpress
```

Or from source:

```bash
cargo install --path .
```

## CLI Usage

```bash
# Basic benchmark (10 concurrent connections, infinite loop)
httpress http://example.com

# Fixed number of requests
httpress http://example.com -n 1000 -c 50

# Fixed total duration
httpress http://example.com -d 30s -c 100

# Rate limiting
httpress http://example.com -r 1000 -d 10s

# POST with body and headers
httpress http://example.com/add -m post \
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
-> httpress http://127.0.0.1:3000 -n 1000 -c 10

Target: http://127.0.0.1:3000 Get
Concurrency: 10
Stop condition: Requests(1000)

Starting benchmark with 10 workers...

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

### Using the test server

```bash
# Run the test server
cargo run --example test_server

# In another terminal, run benchmarks
cargo run -- http://127.0.0.1:3000 -n 100 -c 10
```

## Library Usage

Add httpress as a dependency in your `Cargo.toml`:

```toml
[dependencies]
httpress = "0.3"
tokio = { version = "1", features = ["full"] }
```

See [examples/basic_benchmark.rs](examples/basic_benchmark.rs) for a basic example of using the api.

See [examples/custom_requests.rs](examples/custom_requests.rs) for an example of using the request_fn() method to generate custom requests.
