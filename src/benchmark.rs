//! Benchmark runner and builder.
//!
//! This module provides the main entry point for creating and running HTTP benchmarks.
//! Use [`Benchmark::builder()`] to configure a benchmark, then call [`Benchmark::run()`]
//! to execute it and get results.
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
//!     .concurrency(50)
//!     .duration(Duration::from_secs(10))
//!     .build()?
//!     .run()
//!     .await?;
//!
//! results.print();
//! # Ok(())
//! # }
//! ```

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;

use indicatif::ProgressBar;

use crate::client::HttpClient;
use crate::config::{
    AfterRequestHook, BeforeRequestHook, BenchConfig, HttpMethod, RateContext, RateFunction,
    RequestConfig, RequestContext, RequestGenerator, RequestSource, StopCondition,
};
use crate::error::{Error, Result};
use crate::executor::Executor;
use crate::metrics::BenchmarkResults;

/// Builder for configuring and running benchmarks.
///
/// `BenchmarkBuilder` provides a fluent API for configuring all aspects of an HTTP benchmark,
/// including target URL, concurrency, duration, rate limiting, custom request generation, and hooks.
///
/// # Examples
///
/// Basic benchmark:
/// ```no_run
/// # use httpress::Benchmark;
/// # use std::time::Duration;
/// # #[tokio::main]
/// # async fn main() -> httpress::Result<()> {
/// let results = Benchmark::builder()
///     .url("http://localhost:3000")
///     .concurrency(50)
///     .duration(Duration::from_secs(10))
///     .build()?
///     .run()
///     .await?;
/// # Ok(())
/// # }
/// ```
///
/// With rate limiting:
/// ```no_run
/// # use httpress::Benchmark;
/// # use std::time::Duration;
/// # #[tokio::main]
/// # async fn main() -> httpress::Result<()> {
/// let results = Benchmark::builder()
///     .url("http://localhost:3000")
///     .rate(1000)  // 1000 req/s
///     .duration(Duration::from_secs(30))
///     .build()?
///     .run()
///     .await?;
/// # Ok(())
/// # }
/// ```
pub struct BenchmarkBuilder {
    url: Option<String>,
    method: Option<HttpMethod>,
    concurrency: usize,
    stop_condition: StopCondition,
    headers: HashMap<String, String>,
    body: Option<String>,
    timeout: Duration,
    rate: Option<u64>,
    rate_fn: Option<RateFunction>,
    request_fn: Option<RequestGenerator>,
    before_request_hooks: Vec<BeforeRequestHook>,
    after_request_hooks: Vec<AfterRequestHook>,
    max_retries: usize,
    show_progress: bool,
    insecure: bool,
}

impl BenchmarkBuilder {
    /// Create a new builder with default settings.
    ///
    /// # Default Values
    ///
    /// - `concurrency`: 10
    /// - `stop_condition`: Infinite (runs until interrupted)
    /// - `timeout`: 30 seconds
    /// - `max_retries`: 3
    ///
    /// # Examples
    ///
    /// ```
    /// use httpress::BenchmarkBuilder;
    ///
    /// let builder = BenchmarkBuilder::new();
    /// ```
    pub fn new() -> Self {
        BenchmarkBuilder {
            url: None,
            method: None,
            concurrency: 10,
            stop_condition: StopCondition::Infinite,
            headers: HashMap::new(),
            body: None,
            timeout: Duration::from_secs(30),
            rate: None,
            rate_fn: None,
            request_fn: None,
            before_request_hooks: Vec::new(),
            after_request_hooks: Vec::new(),
            max_retries: 3,
            show_progress: false,
            insecure: false,
        }
    }

    /// Set the target URL (required unless using `request_fn`).
    ///
    /// The URL must be a valid HTTP or HTTPS URL.
    ///
    /// # Constraints
    ///
    /// - Cannot be used together with [`request_fn`](Self::request_fn)
    /// - Exactly one of `url()` or `request_fn()` must be specified
    ///
    /// # Examples
    ///
    /// ```
    /// # use httpress::Benchmark;
    /// let builder = Benchmark::builder()
    ///     .url("http://localhost:3000/api/endpoint");
    /// ```
    pub fn url(mut self, url: &str) -> Self {
        self.url = Some(url.to_string());
        self
    }

    /// Set the HTTP method (default: GET).
    ///
    /// Specifies which HTTP method to use for requests.
    ///
    /// # Constraints
    ///
    /// Cannot be used with [`request_fn`](Self::request_fn) (use `RequestConfig` instead).
    ///
    /// # Examples
    ///
    /// ```
    /// # use httpress::{Benchmark, HttpMethod};
    /// let builder = Benchmark::builder()
    ///     .url("http://localhost:3000")
    ///     .method(HttpMethod::Post);
    /// ```
    pub fn method(mut self, method: HttpMethod) -> Self {
        self.method = Some(method);
        self
    }

    /// Set the number of concurrent connections (default: 10).
    ///
    /// This determines how many workers will send requests in parallel.
    ///
    /// # Examples
    ///
    /// ```
    /// # use httpress::Benchmark;
    /// let builder = Benchmark::builder()
    ///     .url("http://localhost:3000")
    ///     .concurrency(100);  // 100 concurrent workers
    /// ```
    pub fn concurrency(mut self, n: usize) -> Self {
        self.concurrency = n;
        self
    }

    /// Set the test duration.
    ///
    /// The benchmark will run for the specified duration, then stop.
    ///
    /// # Constraints
    ///
    /// Cannot be used together with [`requests`](Self::requests).
    /// If neither `duration` nor `requests` is specified, the benchmark runs infinitely
    /// until interrupted (Ctrl+C).
    ///
    /// # Examples
    ///
    /// ```
    /// # use httpress::Benchmark;
    /// # use std::time::Duration;
    /// let builder = Benchmark::builder()
    ///     .url("http://localhost:3000")
    ///     .duration(Duration::from_secs(30));  // Run for 30 seconds
    /// ```
    pub fn duration(mut self, d: Duration) -> Self {
        self.stop_condition = StopCondition::Duration(d);
        self
    }

    /// Set the total number of requests.
    ///
    /// The benchmark will stop after this many requests have been sent (across all workers).
    ///
    /// # Constraints
    ///
    /// Cannot be used together with [`duration`](Self::duration).
    /// If neither `requests` nor `duration` is specified, the benchmark runs infinitely
    /// until interrupted (Ctrl+C).
    ///
    /// # Examples
    ///
    /// ```
    /// # use httpress::Benchmark;
    /// let builder = Benchmark::builder()
    ///     .url("http://localhost:3000")
    ///     .requests(10000);  // Send 10,000 requests total
    /// ```
    pub fn requests(mut self, n: usize) -> Self {
        self.stop_condition = StopCondition::Requests(n);
        self
    }

    /// Set a fixed target rate in requests per second.
    ///
    /// The benchmark will attempt to maintain this constant rate across all workers.
    ///
    /// # Constraints
    ///
    /// Cannot be used together with [`rate_fn`](Self::rate_fn).
    ///
    /// # Examples
    ///
    /// ```
    /// # use httpress::Benchmark;
    /// # use std::time::Duration;
    /// let builder = Benchmark::builder()
    ///     .url("http://localhost:3000")
    ///     .rate(1000)  // 1000 requests per second
    ///     .duration(Duration::from_secs(30));
    /// ```
    pub fn rate(mut self, rps: u64) -> Self {
        self.rate = Some(rps);
        self
    }

    /// Set a dynamic rate function.
    ///
    /// Provides a function that calculates the target rate dynamically based on runtime
    /// benchmark state (elapsed time, request counts, etc.). This enables rate ramping,
    /// adaptive rate control, or scheduled rate changes.
    ///
    /// # Constraints
    ///
    /// Cannot be used together with [`rate`](Self::rate).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use httpress::{Benchmark, RateContext};
    /// # use std::time::Duration;
    /// # #[tokio::main]
    /// # async fn main() -> httpress::Result<()> {
    /// let results = Benchmark::builder()
    ///     .url("http://localhost:3000")
    ///     .rate_fn(|ctx: RateContext| {
    ///         // Ramp from 100 to 1000 req/s over 10 seconds
    ///         let progress = (ctx.elapsed.as_secs_f64() / 10.0).min(1.0);
    ///         100.0 + (900.0 * progress)
    ///     })
    ///     .duration(Duration::from_secs(30))
    ///     .build()?
    ///     .run()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn rate_fn<F>(mut self, f: F) -> Self
    where
        F: Fn(RateContext) -> f64 + Send + Sync + 'static,
    {
        self.rate_fn = Some(Arc::new(f));
        self
    }

    /// Add an HTTP header.
    ///
    /// Can be called multiple times to add multiple headers.
    ///
    /// # Constraints
    ///
    /// Cannot be used with [`request_fn`](Self::request_fn) (use `RequestConfig` instead).
    ///
    /// # Examples
    ///
    /// ```
    /// # use httpress::Benchmark;
    /// let builder = Benchmark::builder()
    ///     .url("http://localhost:3000")
    ///     .header("Content-Type", "application/json")
    ///     .header("Authorization", "Bearer token123");
    /// ```
    pub fn header(mut self, key: &str, value: &str) -> Self {
        self.headers.insert(key.to_string(), value.to_string());
        self
    }

    /// Set the request body.
    ///
    /// # Constraints
    ///
    /// Cannot be used with [`request_fn`](Self::request_fn) (use `RequestConfig` instead).
    ///
    /// # Examples
    ///
    /// ```
    /// # use httpress::{Benchmark, HttpMethod};
    /// let builder = Benchmark::builder()
    ///     .url("http://localhost:3000/api/users")
    ///     .method(HttpMethod::Post)
    ///     .header("Content-Type", "application/json")
    ///     .body(r#"{"name": "John Doe"}"#);
    /// ```
    pub fn body(mut self, body: &str) -> Self {
        self.body = Some(body.to_string());
        self
    }

    /// Set the request timeout (default: 30s).
    ///
    /// Requests that take longer than this duration will be cancelled and counted as failed.
    ///
    /// # Examples
    ///
    /// ```
    /// # use httpress::Benchmark;
    /// # use std::time::Duration;
    /// let builder = Benchmark::builder()
    ///     .url("http://localhost:3000")
    ///     .timeout(Duration::from_secs(10));  // 10 second timeout
    /// ```
    pub fn timeout(mut self, d: Duration) -> Self {
        self.timeout = d;
        self
    }

    /// Set a custom request generator function.
    ///
    /// Provides a function that generates a unique [`RequestConfig`] for each request
    /// based on [`RequestContext`]. This enables dynamic request patterns like URL rotation,
    /// varying HTTP methods, or conditional headers/bodies.
    ///
    /// # Constraints
    ///
    /// - Cannot be used together with [`url`](Self::url)
    /// - Cannot be used with [`method`](Self::method), [`header`](Self::header), or [`body`](Self::body)
    /// - Exactly one of `url()` or `request_fn()` must be specified
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use httpress::{Benchmark, RequestConfig, RequestContext, HttpMethod};
    /// # use std::collections::HashMap;
    /// # #[tokio::main]
    /// # async fn main() -> httpress::Result<()> {
    /// let results = Benchmark::builder()
    ///     .request_fn(|ctx: RequestContext| {
    ///         // Rotate through 100 different user IDs
    ///         let user_id = ctx.request_number % 100;
    ///
    ///         RequestConfig {
    ///             url: format!("http://localhost:3000/user/{}", user_id),
    ///             method: HttpMethod::Get,
    ///             headers: HashMap::new(),
    ///             body: None,
    ///         }
    ///     })
    ///     .concurrency(10)
    ///     .requests(1000)
    ///     .build()?
    ///     .run()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn request_fn<F>(mut self, f: F) -> Self
    where
        F: Fn(RequestContext) -> RequestConfig + Send + Sync + 'static,
    {
        self.request_fn = Some(Arc::new(f));
        self
    }

    /// Add a before-request hook.
    ///
    /// Before-request hooks are called before sending each HTTP request. They can be used
    /// to implement circuit breakers, conditional request execution, or custom metrics.
    /// Multiple hooks can be added and will be executed in order.
    ///
    /// The hook receives [`BeforeRequestContext`](crate::BeforeRequestContext) and returns
    /// [`HookAction`](crate::HookAction) to control whether the request should proceed.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use httpress::{Benchmark, BeforeRequestContext, HookAction};
    /// # #[tokio::main]
    /// # async fn main() -> httpress::Result<()> {
    /// let results = Benchmark::builder()
    ///     .url("http://localhost:3000")
    ///     .before_request(|ctx: BeforeRequestContext| {
    ///         // Implement circuit breaker
    ///         let failure_rate = ctx.failed_requests as f64 / ctx.total_requests.max(1) as f64;
    ///         if failure_rate > 0.5 && ctx.total_requests > 100 {
    ///             HookAction::Abort
    ///         } else {
    ///             HookAction::Continue
    ///         }
    ///     })
    ///     .requests(1000)
    ///     .build()?
    ///     .run()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn before_request<F>(mut self, f: F) -> Self
    where
        F: Fn(crate::config::BeforeRequestContext) -> crate::config::HookAction
            + Send
            + Sync
            + 'static,
    {
        self.before_request_hooks.push(Arc::new(f));
        self
    }

    /// Add an after-request hook.
    ///
    /// After-request hooks are called after each HTTP request completes (or fails). They can be
    /// used for custom metrics collection, retry logic, or conditional behavior based on response.
    /// Multiple hooks can be added and will be executed in order.
    ///
    /// The hook receives [`AfterRequestContext`](crate::AfterRequestContext) with latency and
    /// status information, and returns [`HookAction`](crate::HookAction) to control retry behavior.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use httpress::{Benchmark, AfterRequestContext, HookAction};
    /// # use std::sync::{Arc, Mutex};
    /// # use std::time::Duration;
    /// # #[tokio::main]
    /// # async fn main() -> httpress::Result<()> {
    /// let slow_count = Arc::new(Mutex::new(0));
    /// let slow_count_clone = slow_count.clone();
    ///
    /// let results = Benchmark::builder()
    ///     .url("http://localhost:3000")
    ///     .after_request(move |ctx: AfterRequestContext| {
    ///         // Track slow requests
    ///         if ctx.latency > Duration::from_millis(100) {
    ///             *slow_count_clone.lock().unwrap() += 1;
    ///         }
    ///
    ///         // Retry on 5xx errors
    ///         if let Some(status) = ctx.status {
    ///             if status >= 500 {
    ///                 return HookAction::Retry;
    ///             }
    ///         }
    ///         HookAction::Continue
    ///     })
    ///     .max_retries(3)
    ///     .requests(1000)
    ///     .build()?
    ///     .run()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn after_request<F>(mut self, f: F) -> Self
    where
        F: Fn(crate::config::AfterRequestContext) -> crate::config::HookAction
            + Send
            + Sync
            + 'static,
    {
        self.after_request_hooks.push(Arc::new(f));
        self
    }

    /// Set maximum number of retries when hooks return `Retry` (default: 3).
    ///
    /// When a hook returns [`HookAction::Retry`](crate::HookAction::Retry), the request
    /// will be retried up to this many times. After exceeding this limit, the request
    /// is marked as failed.
    ///
    /// # Examples
    ///
    /// ```
    /// # use httpress::Benchmark;
    /// let builder = Benchmark::builder()
    ///     .url("http://localhost:3000")
    ///     .max_retries(5);  // Retry up to 5 times
    /// ```
    pub fn max_retries(mut self, n: usize) -> Self {
        self.max_retries = n;
        self
    }

    /// Enable or disable the built-in terminal progress bar (default: false).
    ///
    /// When enabled, a live progress bar is shown during the benchmark displaying
    /// completion progress and a rolling requests-per-second counter.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use httpress::Benchmark;
    /// # use std::time::Duration;
    /// # #[tokio::main]
    /// # async fn main() -> httpress::Result<()> {
    /// let results = Benchmark::builder()
    ///     .url("http://localhost:3000")
    ///     .duration(Duration::from_secs(30))
    ///     .show_progress(true)
    ///     .build()?
    ///     .run()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn show_progress(mut self, show: bool) -> Self {
        self.show_progress = show;
        self
    }

    /// Skip TLS certificate verification.
    pub fn insecure(mut self, insecure: bool) -> Self {
        self.insecure = insecure;
        self
    }

    /// Build the benchmark.
    ///
    /// Validates the configuration and constructs a [`Benchmark`] ready to run.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidConfig`] if:
    /// - Both `url()` and `request_fn()` are specified
    /// - Neither `url()` nor `request_fn()` is specified
    /// - Both `rate()` and `rate_fn()` are specified
    /// - `method()`, `header()`, or `body()` is used with `request_fn()`
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use httpress::Benchmark;
    /// # use std::time::Duration;
    /// # #[tokio::main]
    /// # async fn main() -> httpress::Result<()> {
    /// let benchmark = Benchmark::builder()
    ///     .url("http://localhost:3000")
    ///     .concurrency(50)
    ///     .duration(Duration::from_secs(10))
    ///     .build()?;  // Returns Result<Benchmark>
    ///
    /// let results = benchmark.run().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn build(self) -> Result<Benchmark> {
        if self.rate.is_some() && self.rate_fn.is_some() {
            return Err(Error::InvalidConfig(
                "Cannot use both rate() and rate_fn()".to_string(),
            ));
        }

        let request_source = match (self.url, self.request_fn) {
            (Some(_), Some(_)) => {
                return Err(Error::InvalidConfig(
                    "Cannot use both url() and request_fn()".to_string(),
                ));
            }
            (None, None) => {
                return Err(Error::InvalidConfig(
                    "Must provide either url() or request_fn()".to_string(),
                ));
            }
            (Some(url), None) => {
                let request_config = RequestConfig {
                    url,
                    method: self.method.unwrap_or(HttpMethod::Get),
                    headers: self.headers,
                    body: self.body.map(Bytes::from),
                };
                RequestSource::Static(request_config)
            }
            (None, Some(generator)) => {
                if self.method.is_some() {
                    return Err(Error::InvalidConfig(
                        "Cannot use method() with request_fn()".to_string(),
                    ));
                }
                if !self.headers.is_empty() {
                    return Err(Error::InvalidConfig(
                        "Cannot use header() with request_fn()".to_string(),
                    ));
                }
                if self.body.is_some() {
                    return Err(Error::InvalidConfig(
                        "Cannot use body() with request_fn()".to_string(),
                    ));
                }
                RequestSource::Dynamic(generator)
            }
        };

        let config = BenchConfig {
            request_source,
            concurrency: self.concurrency,
            stop_condition: self.stop_condition,
            timeout: self.timeout,
            rate: self.rate,
            rate_fn: self.rate_fn,
            before_request_hooks: self.before_request_hooks,
            after_request_hooks: self.after_request_hooks,
            max_retries: self.max_retries,
            progress_fn: None,
            insecure: self.insecure,
        };

        let (config, progress_bar) = if self.show_progress {
            let (c, pb) = config.with_progress();
            (c, Some(pb))
        } else {
            (config, None)
        };

        Ok(Benchmark {
            config,
            progress_bar,
        })
    }
}

impl Default for BenchmarkBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// A configured benchmark ready to run.
///
/// Create a `Benchmark` using [`Benchmark::builder()`] and then execute it with [`run()`](Self::run).
///
/// # Examples
///
/// ```no_run
/// # use httpress::Benchmark;
/// # use std::time::Duration;
/// # #[tokio::main]
/// # async fn main() -> httpress::Result<()> {
/// let results = Benchmark::builder()
///     .url("http://localhost:3000")
///     .concurrency(50)
///     .duration(Duration::from_secs(10))
///     .build()?
///     .run()
///     .await?;
///
/// results.print();
/// # Ok(())
/// # }
/// ```
pub struct Benchmark {
    config: BenchConfig,
    progress_bar: Option<Arc<ProgressBar>>,
}

impl Benchmark {
    /// Create a new benchmark builder.
    ///
    /// This is the entry point for configuring and running benchmarks.
    ///
    /// # Examples
    ///
    /// ```
    /// use httpress::Benchmark;
    ///
    /// let builder = Benchmark::builder();
    /// ```
    pub fn builder() -> BenchmarkBuilder {
        BenchmarkBuilder::new()
    }

    /// Run the benchmark and return results.
    ///
    /// Executes the configured benchmark, blocking until completion or interruption (Ctrl+C).
    /// Returns detailed metrics including latency statistics, throughput, and status codes.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The HTTP client cannot be initialized
    /// - Network errors occur during execution (wrapped in results, not returned as error)
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use httpress::Benchmark;
    /// # use std::time::Duration;
    /// # #[tokio::main]
    /// # async fn main() -> httpress::Result<()> {
    /// let benchmark = Benchmark::builder()
    ///     .url("http://localhost:3000")
    ///     .requests(1000)
    ///     .build()?;
    ///
    /// let results = benchmark.run().await?;
    ///
    /// println!("Throughput: {:.2} req/s", results.throughput);
    /// println!("p99 latency: {:?}", results.latency_p99);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn run(self) -> Result<BenchmarkResults> {
        let client = HttpClient::new(
            self.config.timeout,
            self.config.concurrency,
            self.config.insecure,
        )?;
        let executor = Executor::new(client, self.config);
        let results = executor.run().await?;
        if let Some(pb) = self.progress_bar {
            pb.finish_and_clear();
        }
        Ok(results)
    }
}
