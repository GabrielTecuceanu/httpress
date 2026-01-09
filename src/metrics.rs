use std::collections::HashMap;
use std::time::Duration;

/// Result of a single HTTP request
pub struct RequestResult {
    pub latency: Duration,
    pub status: Option<u16>,
}

/// Computed benchmark results
#[derive(Debug, Clone)]
pub struct BenchmarkResults {
    pub total_requests: usize,
    pub successful_requests: usize,
    pub failed_requests: usize,
    pub duration: Duration,
    pub throughput: f64,
    pub latency_min: Duration,
    pub latency_max: Duration,
    pub latency_mean: Duration,
    pub latency_p50: Duration,
    pub latency_p90: Duration,
    pub latency_p95: Duration,
    pub latency_p99: Duration,
    pub status_codes: HashMap<u16, usize>,
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
