use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::mpsc;
use tokio::time::interval;

use crate::client::HttpClient;
use crate::config::{BenchConfig, StopCondition};
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
                run_worker(worker_id, client, config, state, tx, rate_per_worker).await
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
    _worker_id: usize,
    client: Arc<HttpClient>,
    config: Arc<BenchConfig>,
    state: Arc<ExecutorState>,
    tx: mpsc::UnboundedSender<RequestResult>,
    rate_per_worker: Option<u64>,
) {
    let mut rate_interval = rate_per_worker.map(|r| {
        interval(Duration::from_micros(1_000_000 / r))
    });

    while state.increment_and_check() {
        if let Some(ref mut interval) = rate_interval {
            interval.tick().await;
        }

        let start = Instant::now();
        let status = match client.execute(&config).await {
            Ok(response) => Some(response.status().as_u16()),
            Err(_) => None,
        };
        let latency = start.elapsed();

        let _ = tx.send(RequestResult { latency, status });
    }
}
