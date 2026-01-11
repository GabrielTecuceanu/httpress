use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::mpsc;
use tokio::time::{interval, MissedTickBehavior};

use crate::client::HttpClient;
use crate::config::{BenchConfig, RateContext, RequestContext, RequestSource, StopCondition};
use crate::error::Result;
use crate::metrics::{BenchmarkResults, Metrics, RequestResult};

/// Shared state for coordinating workers
struct ExecutorState {
    /// Signal to stop all workers
    stop: AtomicBool,
    /// Counter for completed requests
    request_count: AtomicUsize,
    /// Target request count (if applicable)
    target_requests: Option<usize>,
    /// Counter for successful requests (2xx status codes)
    successful_count: AtomicUsize,
    /// Counter for failed requests (non-2xx or errors)
    failed_count: AtomicUsize,
}

impl ExecutorState {
    fn new(stop_condition: &StopCondition) -> Self {
        let target_requests = match stop_condition {
            StopCondition::Requests(n) => Some(*n),
            _ => None,
        };

        ExecutorState {
            stop: AtomicBool::new(false),
            request_count: AtomicUsize::new(0),
            target_requests,
            successful_count: AtomicUsize::new(0),
            failed_count: AtomicUsize::new(0),
        }
    }

    /// Try to claim a slot and check if worker should continue.
    /// Returns true if a slot was claimed and work should proceed.
    fn increment_and_check(&self) -> bool {
        if self.stop.load(Ordering::Relaxed) {
            return false;
        }

        let slot = self.request_count.fetch_add(1, Ordering::Relaxed);

        if let Some(target) = self.target_requests {
            if slot >= target {
                self.stop.store(true, Ordering::Relaxed);
                self.request_count.fetch_sub(1, Ordering::Relaxed);
                return false;
            }
        }

        true
    }

    /// Signal all workers to stop
    fn signal_stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }

    /// Get current counts for RateContext
    fn get_counts(&self) -> (usize, usize, usize) {
        (
            self.request_count.load(Ordering::Relaxed),
            self.successful_count.load(Ordering::Relaxed),
            self.failed_count.load(Ordering::Relaxed),
        )
    }

    /// Record successful request (2xx status code)
    fn record_success(&self) {
        self.successful_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Record failed request (non-2xx or error)
    fn record_failure(&self) {
        self.failed_count.fetch_add(1, Ordering::Relaxed);
    }
}

/// Async HTTP executor with fixed concurrency
pub struct Executor {
    client: Arc<HttpClient>,
    config: Arc<BenchConfig>,
}

impl Executor {
    /// Create a new executor
    pub fn new(client: HttpClient, config: BenchConfig) -> Self {
        Executor {
            client: Arc::new(client),
            config: Arc::new(config),
        }
    }

    /// Run the benchmark and return results
    pub async fn run(&self) -> Result<BenchmarkResults> {
        let state = Arc::new(ExecutorState::new(&self.config.stop_condition));
        let start_time = Instant::now();

        let (tx, mut rx) = mpsc::unbounded_channel::<RequestResult>();

        if let StopCondition::Duration(duration) = self.config.stop_condition {
            let state_clone = Arc::clone(&state);
            tokio::spawn(async move {
                tokio::time::sleep(duration).await;
                state_clone.signal_stop();
            });
        }

        let state_for_ctrlc = Arc::clone(&state);
        tokio::spawn(async move {
            if tokio::signal::ctrl_c().await.is_ok() {
                println!("\nReceived Ctrl+C, shutting down...");
                state_for_ctrlc.signal_stop();
            }
        });

        let mut handles = Vec::with_capacity(self.config.concurrency);

        let rate_per_worker = self.config.rate.map(|r| {
            (r as f64 / self.config.concurrency as f64).max(1.0) as u64
        });

        for worker_id in 0..self.config.concurrency {
            let client = Arc::clone(&self.client);
            let config = Arc::clone(&self.config);
            let state = Arc::clone(&state);
            let tx = tx.clone();

            let handle = tokio::spawn(async move {
                run_worker(worker_id, client, config, state, tx, rate_per_worker, start_time).await
            });

            handles.push(handle);
        }

        drop(tx);

        let mut metrics = Metrics::new();
        while let Some(result) = rx.recv().await {
            metrics.record(result);
        }

        for handle in handles {
            let _ = handle.await;
        }

        let elapsed = start_time.elapsed();

        Ok(metrics.into_results(elapsed))
    }
}

/// Worker loop that executes requests
async fn run_worker(
    worker_id: usize,
    client: Arc<HttpClient>,
    config: Arc<BenchConfig>,
    state: Arc<ExecutorState>,
    tx: mpsc::UnboundedSender<RequestResult>,
    rate_per_worker: Option<u64>,
    start_time: Instant,
) {
    match &config.rate_fn {
        None => {
            run_worker_static(worker_id, client, Arc::clone(&config), state, tx, rate_per_worker).await
        }
        Some(rate_fn) => {
            run_worker_dynamic(worker_id, client, Arc::clone(&config), state, tx, rate_fn.clone(), start_time).await
        }
    }
}

/// Worker with static rate limiting
async fn run_worker_static(
    worker_id: usize,
    client: Arc<HttpClient>,
    config: Arc<BenchConfig>,
    state: Arc<ExecutorState>,
    tx: mpsc::UnboundedSender<RequestResult>,
    rate_per_worker: Option<u64>,
) {
    let mut rate_interval = rate_per_worker.map(|r| interval(Duration::from_micros(1_000_000 / r)));

    let mut request_number = 0;

    while state.increment_and_check() {
        if let Some(ref mut interval) = rate_interval {
            interval.tick().await;
        }

        let start = Instant::now();
        let status = match &config.request_source {
            RequestSource::Static(_) => match client.execute(&config).await {
                Ok(response) => Some(response.status().as_u16()),
                Err(_) => None,
            },
            RequestSource::Dynamic(generator) => {
                let ctx = RequestContext { worker_id, request_number };
                let request_config = generator(ctx);

                match client.execute_request(&request_config).await {
                    Ok(response) => Some(response.status().as_u16()),
                    Err(_) => None,
                }
            }
        };
        let latency = start.elapsed();

        if let Some(s) = status {
            if (200..300).contains(&s) {
                state.record_success();
            } else {
                state.record_failure();
            }
        } else {
            state.record_failure();
        }

        let _ = tx.send(RequestResult { latency, status });
        request_number += 1;
    }
}

/// Worker with dynamic rate control
async fn run_worker_dynamic(
    worker_id: usize,
    client: Arc<HttpClient>,
    config: Arc<BenchConfig>,
    state: Arc<ExecutorState>,
    tx: mpsc::UnboundedSender<RequestResult>,
    rate_fn: Arc<dyn Fn(RateContext) -> f64 + Send + Sync>,
    start_time: Instant,
) {
    const RATE_UPDATE_INTERVAL_MS: u64 = 100;

    let mut rate_update_interval = interval(Duration::from_millis(RATE_UPDATE_INTERVAL_MS));
    rate_update_interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

    let (total, success, failed) = state.get_counts();
    let initial_context = RateContext {
        elapsed: start_time.elapsed(),
        total_requests: total,
        successful_requests: success,
        failed_requests: failed,
        current_rate: 0.0,
    };
    let mut current_rate = validate_rate(rate_fn(initial_context));
    let mut rate_interval = create_rate_interval(current_rate, config.concurrency);

    let mut request_number = 0;

    loop {
        tokio::select! {
            _ = rate_update_interval.tick() => {
                let (total, success, failed) = state.get_counts();
                let ctx = RateContext {
                    elapsed: start_time.elapsed(),
                    total_requests: total,
                    successful_requests: success,
                    failed_requests: failed,
                    current_rate,
                };
                let new_rate = validate_rate(rate_fn(ctx));

                if (new_rate - current_rate).abs() > 0.01 {
                    current_rate = new_rate;
                    rate_interval = create_rate_interval(current_rate, config.concurrency);
                }
            }
            _ = rate_interval.tick() => {
                if !state.increment_and_check() {
                    break;
                }

                let start = Instant::now();
                let status = match &config.request_source {
                    RequestSource::Static(_) => match client.execute(&config).await {
                        Ok(response) => Some(response.status().as_u16()),
                        Err(_) => None,
                    },
                    RequestSource::Dynamic(generator) => {
                        let ctx = RequestContext { worker_id, request_number };
                        let request_config = generator(ctx);

                        match client.execute_request(&request_config).await {
                            Ok(response) => Some(response.status().as_u16()),
                            Err(_) => None,
                        }
                    }
                };
                let latency = start.elapsed();

                if let Some(s) = status {
                    if (200..300).contains(&s) {
                        state.record_success();
                    } else {
                        state.record_failure();
                    }
                } else {
                    state.record_failure();
                }

                let _ = tx.send(RequestResult { latency, status });
                request_number += 1;
            }
        }
    }
}

/// Create rate interval for a given rate per second
fn create_rate_interval(rate_per_second: f64, worker_count: usize) -> tokio::time::Interval {
    let rate_per_worker = (rate_per_second / worker_count as f64).max(0.1);
    let period_micros = (1_000_000.0 / rate_per_worker) as u64;
    let mut interval = interval(Duration::from_micros(period_micros));
    interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
    interval
}

/// Validate and clamp rate to safe range
fn validate_rate(rate: f64) -> f64 {
    if rate.is_nan() || rate.is_infinite() || rate < 0.1 {
        0.1
    } else {
        rate
    }
}
