use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use tokio::sync::{mpsc, watch};
use tokio::time::{MissedTickBehavior, interval};

use crate::client::HttpClient;
use crate::config::{
    AfterRequestContext, BeforeRequestContext, BenchConfig, HookAction, RateContext, StopCondition,
};
use crate::error::Result;
use crate::metrics::{BenchmarkResults, Metrics, RequestResult};
use crate::progress::ProgressSnapshot;

/// Common context shared by all worker tasks
struct WorkerContext {
    worker_id: usize,
    client: Arc<HttpClient>,
    config: Arc<BenchConfig>,
    state: Arc<ExecutorState>,
    tx: mpsc::UnboundedSender<RequestResult>,
    start_time: Instant,
}

impl WorkerContext {
    /// Execute a single request with hooks and retry logic, then send the result.
    async fn execute_and_send(&self, request_number: usize) {
        let result = self.execute_with_hooks(request_number).await;
        let _ = self.tx.send(result);
    }

    /// Execute a single request with hooks and retry logic.
    async fn execute_with_hooks(&self, request_number: usize) -> RequestResult {
        let max_retries = self.config.max_retries;
        let mut retry_count = 0;

        loop {
            // Execute before_request hooks
            if !self.config.before_request_hooks.is_empty() {
                let ctx = self.before_context(request_number);
                match execute_hooks(&self.config.before_request_hooks, ctx) {
                    HookAction::Continue => {}
                    HookAction::Abort => {
                        self.state.record_failure();
                        return RequestResult {
                            latency: Duration::ZERO,
                            status: None,
                            bytes: 0,
                        };
                    }
                    HookAction::Retry => {
                        if retry_count < max_retries {
                            retry_count += 1;
                            continue;
                        } else {
                            self.state.record_failure();
                            return RequestResult {
                                latency: Duration::ZERO,
                                status: None,
                                bytes: 0,
                            };
                        }
                    }
                }
            }

            let start = Instant::now();
            let (status, bytes) = self
                .client
                .execute_for_worker(&self.config, self.worker_id, request_number)
                .await
                .unwrap_or_default();
            let latency = start.elapsed();

            // Execute after_request hooks
            let hook_action = if !self.config.after_request_hooks.is_empty() {
                let ctx = self.after_context(request_number, latency, status);
                execute_hooks(&self.config.after_request_hooks, ctx)
            } else {
                HookAction::Continue
            };

            match hook_action {
                HookAction::Continue => {
                    self.state.record_status(status);
                    return RequestResult {
                        latency,
                        status,
                        bytes,
                    };
                }
                HookAction::Abort => {
                    self.state.record_failure();
                    return RequestResult {
                        latency,
                        status: None,
                        bytes: 0,
                    };
                }
                HookAction::Retry => {
                    if retry_count < max_retries {
                        retry_count += 1;
                        continue;
                    } else {
                        self.state.record_status(status);
                        return RequestResult {
                            latency,
                            status,
                            bytes,
                        };
                    }
                }
            }
        }
    }

    fn before_context(&self, request_number: usize) -> BeforeRequestContext {
        let (total, success, failed) = self.state.get_counts();
        BeforeRequestContext {
            worker_id: self.worker_id,
            request_number,
            elapsed: self.start_time.elapsed(),
            total_requests: total,
            successful_requests: success,
            failed_requests: failed,
        }
    }

    fn after_context(
        &self,
        request_number: usize,
        latency: Duration,
        status: Option<u16>,
    ) -> AfterRequestContext {
        let (total, success, failed) = self.state.get_counts();
        AfterRequestContext {
            worker_id: self.worker_id,
            request_number,
            elapsed: self.start_time.elapsed(),
            total_requests: total,
            successful_requests: success,
            failed_requests: failed,
            latency,
            status,
        }
    }
}

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
            && slot >= target
        {
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

    /// Get current counts for hook contexts
    fn get_counts(&self) -> (usize, usize, usize) {
        (
            self.request_count.load(Ordering::Relaxed),
            self.successful_count.load(Ordering::Relaxed),
            self.failed_count.load(Ordering::Relaxed),
        )
    }

    /// Record result based on HTTP status code
    fn record_status(&self, status: Option<u16>) {
        match status {
            Some(s) if (200..300).contains(&s) => {
                self.successful_count.fetch_add(1, Ordering::Relaxed);
            }
            _ => {
                self.failed_count.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// Record a failed request
    fn record_failure(&self) {
        self.failed_count.fetch_add(1, Ordering::Relaxed);
    }
}

/// Maximum number of results to drain from the channel per recv_many call.
const RECV_BATCH_LIMIT: usize = 256;

/// Execute hooks in order, returning the first non-Continue action.
fn execute_hooks<T, F>(hooks: &[Arc<F>], ctx: T) -> HookAction
where
    T: Copy,
    F: Fn(T) -> HookAction + Send + Sync + ?Sized,
{
    for hook in hooks {
        match hook(ctx) {
            HookAction::Continue => continue,
            action => return action,
        }
    }
    HookAction::Continue
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
            let per_worker = r as f64 / self.config.concurrency as f64;
            Duration::from_secs_f64(1.0 / per_worker.max(0.1))
        });

        // Spawn a single rate coordinator for dynamic rate, shared across all workers
        let rate_rx = self.spawn_rate_coordinator(&state, start_time);
        self.spawn_progress_coordinator(&state, start_time);

        for worker_id in 0..self.config.concurrency {
            let ctx = WorkerContext {
                worker_id,
                client: Arc::clone(&self.client),
                config: Arc::clone(&self.config),
                state: Arc::clone(&state),
                tx: tx.clone(),
                start_time,
            };
            let rate_rx = rate_rx.clone();

            let handle =
                tokio::spawn(async move { run_worker(ctx, rate_per_worker, rate_rx).await });

            handles.push(handle);
        }

        drop(tx);

        let capacity = match self.config.stop_condition {
            StopCondition::Requests(n) => n,
            StopCondition::Duration(d) => {
                let secs = d.as_secs_f64();
                match self.config.rate {
                    Some(rate) => (rate as f64 * secs) as usize,
                    None => self.config.concurrency * 1_000 * secs as usize,
                }
            }
            StopCondition::Infinite => 10_000,
        };
        let mut metrics = Metrics::with_capacity(capacity);
        let mut buf = Vec::with_capacity(RECV_BATCH_LIMIT);
        while rx.recv_many(&mut buf, RECV_BATCH_LIMIT).await > 0 {
            for result in buf.drain(..) {
                metrics.record(result);
            }
        }

        for handle in handles {
            let _ = handle.await;
        }

        let elapsed = start_time.elapsed();

        Ok(metrics.into_results(elapsed))
    }

    /// Spawn a rate coordinator task if dynamic rate is configured.
    fn spawn_rate_coordinator(
        &self,
        state: &Arc<ExecutorState>,
        start_time: Instant,
    ) -> Option<watch::Receiver<f64>> {
        let rate_fn = self.config.rate_fn.as_ref()?;

        let (total, success, failed) = state.get_counts();
        let initial_rate = validate_rate(rate_fn(RateContext {
            elapsed: Duration::ZERO,
            total_requests: total,
            successful_requests: success,
            failed_requests: failed,
            current_rate: 0.0,
        }));
        let (rate_tx, rate_rx) = watch::channel(initial_rate);

        let rate_fn = rate_fn.clone();
        let state = Arc::clone(state);
        tokio::spawn(async move {
            const RATE_UPDATE_INTERVAL_MS: u64 = 100;
            let mut update_interval = interval(Duration::from_millis(RATE_UPDATE_INTERVAL_MS));
            update_interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
            let mut current_rate = initial_rate;

            loop {
                update_interval.tick().await;
                if state.stop.load(Ordering::Relaxed) {
                    break;
                }
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
                    let _ = rate_tx.send(current_rate);
                }
            }
        });

        Some(rate_rx)
    }

    /// Spawn a progress reporter task if a progress function is configured.
    fn spawn_progress_coordinator(&self, state: &Arc<ExecutorState>, start_time: Instant) {
        let Some(progress_fn) = &self.config.progress_fn else {
            return;
        };

        let progress_fn = progress_fn.clone();
        let state = Arc::clone(state);
        let target_requests = match self.config.stop_condition {
            StopCondition::Requests(n) => Some(n),
            _ => None,
        };
        let target_duration = match self.config.stop_condition {
            StopCondition::Duration(d) => Some(d),
            _ => None,
        };

        tokio::spawn(async move {
            const INTERVAL_MS: u64 = 250;
            let mut tick_interval = interval(Duration::from_millis(INTERVAL_MS));
            tick_interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
            let mut prev_count = 0usize;

            loop {
                tick_interval.tick().await;

                let (total, success, failed) = state.get_counts();
                let delta = total.saturating_sub(prev_count);
                prev_count = total;
                let current_rps = delta as f64 * (1000.0 / INTERVAL_MS as f64);

                progress_fn(ProgressSnapshot {
                    total_requests: total,
                    successful_requests: success,
                    failed_requests: failed,
                    elapsed: start_time.elapsed(),
                    current_rps,
                    target_requests,
                    target_duration,
                });

                if state.stop.load(Ordering::Relaxed) {
                    break;
                }
            }
        });
    }
}

/// Worker loop that dispatches to static or dynamic rate mode
async fn run_worker(
    ctx: WorkerContext,
    rate_per_worker: Option<Duration>,
    rate_rx: Option<watch::Receiver<f64>>,
) {
    match rate_rx {
        None => run_worker_static(ctx, rate_per_worker).await,
        Some(rate_rx) => run_worker_dynamic(ctx, rate_rx).await,
    }
}

/// Worker with static rate limiting
async fn run_worker_static(ctx: WorkerContext, rate_period: Option<Duration>) {
    let mut rate_interval = rate_period.map(|p| interval(p));

    let mut request_number = 0;

    while ctx.state.increment_and_check() {
        if let Some(ref mut interval) = rate_interval {
            interval.tick().await;
        }

        ctx.execute_and_send(request_number).await;
        request_number += 1;

        // Yield to the runtime between requests when there's no rate limit.
        // Without this, fast responses (e.g. localhost) turn the loop into a
        // CPU-saturating spin that can starve the OS on high-core-count systems.
        if rate_interval.is_none() {
            tokio::task::yield_now().await;
        }
    }
}

/// Worker with dynamic rate control
async fn run_worker_dynamic(ctx: WorkerContext, mut rate_rx: watch::Receiver<f64>) {
    let mut current_rate = *rate_rx.borrow();
    let mut rate_interval = create_rate_interval(current_rate, ctx.config.concurrency);
    let mut rate_active = true;

    let mut request_number = 0;

    loop {
        tokio::select! {
            result = rate_rx.changed(), if rate_active => {
                match result {
                    Ok(()) => {
                        let new_rate = *rate_rx.borrow_and_update();
                        if (new_rate - current_rate).abs() > 0.01 {
                            current_rate = new_rate;
                            rate_interval = create_rate_interval(current_rate, ctx.config.concurrency);
                        }
                    }
                    Err(_) => {
                        rate_active = false;
                    }
                }
            }
            _ = rate_interval.tick() => {
                if !ctx.state.increment_and_check() {
                    break;
                }

                ctx.execute_and_send(request_number).await;
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
