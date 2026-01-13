//! Benchmark results and metrics.
//!
//! This module contains types for representing benchmark results.
//! The main type is [`BenchmarkResults`], which contains detailed metrics
//! from a completed benchmark including latency statistics, throughput,
//! and status code distribution.
//!
//! # Examples
//!
//! ```no_run
//! use httpress::Benchmark;
//! use std::time::Duration;
//!
//! # #[tokio::main]
//! # async fn main() -> httpress::Result<()> {
//! let results = Benchmark::builder()
//!     .url("http://localhost:3000")
//!     .requests(1000)
//!     .build()?
//!     .run()
//!     .await?;
//!
//! // Print formatted results
//! results.print();
//!
//! // Or access individual metrics
//! println!("Throughput: {:.2} req/s", results.throughput);
//! println!("p99 latency: {:?}", results.latency_p99);
//! println!("Success rate: {:.2}%",
//!     (results.successful_requests as f64 / results.total_requests as f64) * 100.0
//! );
//! # Ok(())
//! # }
//! ```

use std::collections::HashMap;
use std::time::Duration;

/// Result of a single HTTP request
pub struct RequestResult {
    pub latency: Duration,
    pub status: Option<u16>,
}

/// Computed benchmark results with detailed metrics.
///
/// This struct contains all the metrics collected during a benchmark run, including
/// request counts, latency statistics, and throughput measurements.
///
/// # Examples
///
/// ```no_run
/// # use httpress::{Benchmark, Result};
/// # use std::time::Duration;
/// # #[tokio::main]
/// # async fn main() -> Result<()> {
/// let results = Benchmark::builder()
///     .url("http://localhost:3000")
///     .requests(100)
///     .build()?
///     .run()
///     .await?;
///
/// // Print formatted results
/// results.print();
///
/// // Or access individual metrics
/// println!("Total requests: {}", results.total_requests);
/// println!("Success rate: {:.2}%",
///     (results.successful_requests as f64 / results.total_requests as f64) * 100.0
/// );
/// println!("p99 latency: {:?}", results.latency_p99);
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct BenchmarkResults {
    /// Total number of requests executed.
    pub total_requests: usize,

    /// Number of successful requests (HTTP status 2xx).
    pub successful_requests: usize,

    /// Number of failed requests (non-2xx status or connection errors).
    pub failed_requests: usize,

    /// Actual duration of the benchmark.
    pub duration: Duration,

    /// Throughput in requests per second (total_requests / duration).
    pub throughput: f64,

    /// Minimum request latency observed.
    pub latency_min: Duration,

    /// Maximum request latency observed.
    pub latency_max: Duration,

    /// Mean (average) request latency.
    pub latency_mean: Duration,

    /// 50th percentile (median) request latency.
    pub latency_p50: Duration,

    /// 90th percentile request latency.
    pub latency_p90: Duration,

    /// 95th percentile request latency.
    pub latency_p95: Duration,

    /// 99th percentile request latency.
    pub latency_p99: Duration,

    /// Distribution of HTTP status codes and their counts.
    pub status_codes: HashMap<u16, usize>,
}

impl BenchmarkResults {
    /// Print formatted results to stdout.
    ///
    /// Displays a human-readable summary including:
    /// - Request counts (total, success, errors)
    /// - Duration and throughput
    /// - Latency statistics (min, max, mean, percentiles)
    /// - Status code distribution
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use httpress::{Benchmark, Result};
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    /// let results = Benchmark::builder()
    ///     .url("http://localhost:3000")
    ///     .requests(100)
    ///     .build()?
    ///     .run()
    ///     .await?;
    ///
    /// results.print();
    /// // Output:
    /// // --- Benchmark Complete ---
    /// // Requests:     100 total, 98 success, 2 errors
    /// // Duration:     2.45s
    /// // Throughput:   40.82 req/s
    /// //
    /// // Latency:
    /// //   Min:    12.3ms
    /// //   Max:    156.7ms
    /// //   Mean:   45.2ms
    /// //   p50:    42.1ms
    /// //   p90:    78.3ms
    /// //   p95:    95.4ms
    /// //   p99:    145.2ms
    /// //
    /// // Status codes:
    /// //   200: 98
    /// //   500: 2
    /// # Ok(())
    /// # }
    /// ```
    pub fn print(&self) {
        println!("\n--- Benchmark Complete ---");
        println!(
            "Requests:     {} total, {} success, {} errors",
            self.total_requests, self.successful_requests, self.failed_requests
        );
        println!("Duration:     {:.2}s", self.duration.as_secs_f64());
        println!("Throughput:   {:.2} req/s", self.throughput);

        println!("\nLatency:");
        println!("  Min:    {}", format_duration(self.latency_min));
        println!("  Max:    {}", format_duration(self.latency_max));
        println!("  Mean:   {}", format_duration(self.latency_mean));
        println!("  p50:    {}", format_duration(self.latency_p50));
        println!("  p90:    {}", format_duration(self.latency_p90));
        println!("  p95:    {}", format_duration(self.latency_p95));
        println!("  p99:    {}", format_duration(self.latency_p99));

        if !self.status_codes.is_empty() {
            println!("\nStatus codes:");
            let mut codes: Vec<_> = self.status_codes.iter().collect();
            codes.sort_by_key(|(k, _)| *k);
            for (code, count) in codes {
                println!("  {}: {}", code, count);
            }
        }
    }
}

/// Aggregated metrics from all requests
pub struct Metrics {
    pub total: usize,
    pub success: usize,
    pub latencies: Vec<Duration>,
    pub status_codes: HashMap<u16, usize>,
}

impl Metrics {
    pub fn new() -> Self {
        Metrics {
            total: 0,
            success: 0,
            latencies: Vec::new(),
            status_codes: HashMap::new(),
        }
    }

    pub fn record(&mut self, result: RequestResult) {
        self.total += 1;
        if let Some(status) = result.status {
            *self.status_codes.entry(status).or_insert(0) += 1;
            if (200..300).contains(&status) {
                self.success += 1;
            }
        }
        self.latencies.push(result.latency);
    }

    pub fn report(&self, elapsed: Duration) {
        let errors = self.total - self.success;
        let throughput = self.total as f64 / elapsed.as_secs_f64();

        println!("\n--- Benchmark Complete ---");
        println!(
            "Requests:     {} total, {} success, {} errors",
            self.total, self.success, errors
        );
        println!("Duration:     {:.2}s", elapsed.as_secs_f64());
        println!("Throughput:   {:.2} req/s", throughput);

        if !self.latencies.is_empty() {
            let mut sorted = self.latencies.clone();
            sorted.sort();

            let min = sorted.first().unwrap();
            let max = sorted.last().unwrap();
            let sum: Duration = sorted.iter().sum();
            let mean = sum / sorted.len() as u32;

            let p50 = percentile(&sorted, 50);
            let p90 = percentile(&sorted, 90);
            let p95 = percentile(&sorted, 95);
            let p99 = percentile(&sorted, 99);

            println!("\nLatency:");
            println!("  Min:    {}", format_duration(*min));
            println!("  Max:    {}", format_duration(*max));
            println!("  Mean:   {}", format_duration(mean));
            println!("  p50:    {}", format_duration(p50));
            println!("  p90:    {}", format_duration(p90));
            println!("  p95:    {}", format_duration(p95));
            println!("  p99:    {}", format_duration(p99));
        }

        if !self.status_codes.is_empty() {
            println!("\nStatus codes:");
            let mut codes: Vec<_> = self.status_codes.iter().collect();
            codes.sort_by_key(|(k, _)| *k);
            for (code, count) in codes {
                println!("  {}: {}", code, count);
            }
        }
    }

    /// Convert raw metrics into computed results
    pub fn into_results(mut self, elapsed: Duration) -> BenchmarkResults {
        self.latencies.sort();
        let sorted = &self.latencies;

        let (latency_min, latency_max, latency_mean, latency_p50, latency_p90, latency_p95, latency_p99) =
            if sorted.is_empty() {
                (
                    Duration::ZERO,
                    Duration::ZERO,
                    Duration::ZERO,
                    Duration::ZERO,
                    Duration::ZERO,
                    Duration::ZERO,
                    Duration::ZERO,
                )
            } else {
                let min = *sorted.first().unwrap();
                let max = *sorted.last().unwrap();
                let sum: Duration = sorted.iter().sum();
                let mean = sum / sorted.len() as u32;

                (
                    min,
                    max,
                    mean,
                    percentile(sorted, 50),
                    percentile(sorted, 90),
                    percentile(sorted, 95),
                    percentile(sorted, 99),
                )
            };

        BenchmarkResults {
            total_requests: self.total,
            successful_requests: self.success,
            failed_requests: self.total - self.success,
            duration: elapsed,
            throughput: self.total as f64 / elapsed.as_secs_f64(),
            latency_min,
            latency_max,
            latency_mean,
            latency_p50,
            latency_p90,
            latency_p95,
            latency_p99,
            status_codes: self.status_codes,
        }
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}

fn percentile(sorted: &[Duration], p: usize) -> Duration {
    let idx = (sorted.len() * p / 100).saturating_sub(1).max(0);
    sorted[idx]
}

fn format_duration(d: Duration) -> String {
    let micros = d.as_micros();
    if micros < 1000 {
        format!("{}us", micros)
    } else if micros < 1_000_000 {
        format!("{:.2}ms", micros as f64 / 1000.0)
    } else {
        format!("{:.2}s", d.as_secs_f64())
    }
}
