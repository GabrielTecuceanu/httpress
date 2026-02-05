use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::mpsc;
use tokio::time::{interval, MissedTickBehavior};

use crate::client::HttpClient;
use crate::config::{
    AfterRequestContext, BeforeRequestContext, BenchConfig,
    HookAction, RateContext, RequestContext, RequestSource, StopCondition,
};
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

        if let Some(target) = self.target_requests
            && slot >= target {
                self.stop.store(true, Ordering::Relaxed);
                self.request_count.fetch_sub(1, Ordering::Relaxed);
                return false;
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

/// Execute hooks with panic safety
fn execute_hooks<T, F>(hooks: &[Arc<F>], ctx: T, hook_type: &str) -> HookAction
where
    T: Copy,
    F: Fn(T) -> HookAction + Send + Sync + ?Sized,
{
    use std::panic::{catch_unwind, AssertUnwindSafe};

    for (idx, hook) in hooks.iter().enumerate() {
        let ctx_clone = ctx;
        match catch_unwind(AssertUnwindSafe(|| hook(ctx_clone))) {
            Ok(HookAction::Continue) => continue,
            Ok(action @ (HookAction::Abort | HookAction::Retry)) => return action,
            Err(_) => {
                eprintln!("Warning: {} hook {} panicked, continuing", hook_type, idx);
                continue;
            }
        }
    }
    HookAction::Continue
}

/// Build context for before_request hooks
fn build_before_context(
    worker_id: usize,
    request_number: usize,
    state: &ExecutorState,
    start_time: Instant,
) -> BeforeRequestContext {
    let (total, success, failed) = state.get_counts();
    BeforeRequestContext {
        worker_id,
        request_number,
        elapsed: start_time.elapsed(),
        total_requests: total,
        successful_requests: success,
        failed_requests: failed,
    }
}

/// Build context for after_request hooks
fn build_after_context(
    worker_id: usize,
    request_number: usize,
    state: &ExecutorState,
    start_time: Instant,
    latency: Duration,
    status: Option<u16>,
) -> AfterRequestContext {
    let (total, success, failed) = state.get_counts();
    AfterRequestContext {
        worker_id,
        request_number,
        elapsed: start_time.elapsed(),
        total_requests: total,
        successful_requests: success,
        failed_requests: failed,
        latency,
        status,
    }
}

/// Execute the actual HTTP request
async fn perform_http_request(
    worker_id: usize,
    request_number: usize,
    client: &HttpClient,
    config: &BenchConfig,
) -> (Duration, Option<u16>) {
    let start = Instant::now();
    let status = match &config.request_source {
        RequestSource::Static(_) => match client.execute(config).await {
            Ok(response) => Some(response.status().as_u16()),
            Err(_) => None,
        },
        RequestSource::Dynamic(generator) => {
            let ctx = RequestContext {
                worker_id,
                request_number,
            };
            let request_config = generator(ctx);

            match client.execute_request(&request_config).await {
                Ok(response) => Some(response.status().as_u16()),
                Err(_) => None,
            }
        }
    };
    let latency = start.elapsed();
    (latency, status)
}

/// Record request result to state based on status code
fn record_result(state: &ExecutorState, status: Option<u16>) {
    if let Some(s) = status {
        if (200..300).contains(&s) {
            state.record_success();
        } else {
            state.record_failure();
        }
    } else {
        state.record_failure();
    }
}

/// Execute a single request with hooks and retry logic
async fn execute_request_with_hooks(
    worker_id: usize,
    request_number: usize,
    client: &HttpClient,
    config: &BenchConfig,
    state: &ExecutorState,
    start_time: Instant,
) -> RequestResult {
    let max_retries = config.max_retries;
    let mut retry_count = 0;

    loop {
        // Execute before_request hooks
        if !config.before_request_hooks.is_empty() {
            let ctx = build_before_context(worker_id, request_number, state, start_time);
            match execute_hooks(&config.before_request_hooks, ctx, "before_request") {
                HookAction::Continue => {}
                HookAction::Abort => {
                    state.record_failure();
                    return RequestResult {
                        latency: Duration::ZERO,
                        status: None,
                    };
                }
                HookAction::Retry => {
                    if retry_count < max_retries {
                        retry_count += 1;
                        continue;
                    } else {
                        state.record_failure();
                        return RequestResult {
                            latency: Duration::ZERO,
                            status: None,
                        };
                    }
                }
            }
        }

        let (latency, status) = perform_http_request(worker_id, request_number, client, config).await;

        // Execute after_request hooks
        let hook_action = if !config.after_request_hooks.is_empty() {
            let ctx = build_after_context(worker_id, request_number, state, start_time, latency, status);
            execute_hooks(&config.after_request_hooks, ctx, "after_request")
        } else {
            HookAction::Continue
        };

        match hook_action {
            HookAction::Continue => {
                record_result(state, status);
                return RequestResult { latency, status };
            }
            HookAction::Abort => {
                state.record_failure();
                return RequestResult {
                    latency,
                    status: None,
                };
            }
            HookAction::Retry => {
                if retry_count < max_retries {
                    retry_count += 1;
                    continue;
                } else {
                    // Max retries exceeded, record the result anyway
                    record_result(state, status);
                    return RequestResult { latency, status };
                }
            }
        }
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
            run_worker_static(worker_id, client, Arc::clone(&config), state, tx, rate_per_worker, start_time).await
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
    start_time: Instant,
) {
    let mut rate_interval = rate_per_worker.map(|r| interval(Duration::from_micros(1_000_000 / r)));

    let mut request_number = 0;

    while state.increment_and_check() {
        if let Some(ref mut interval) = rate_interval {
            interval.tick().await;
        }

        let result = execute_request_with_hooks(
            worker_id,
            request_number,
            &client,
            &config,
            &state,
            start_time,
        )
        .await;

        let _ = tx.send(result);
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

                let result = execute_request_with_hooks(
                    worker_id,
                    request_number,
                    &client,
                    &config,
                    &state,
                    start_time,
                )
                .await;

                let _ = tx.send(result);
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
