//! Configuration types and contexts for benchmarks.
//!
//! This module contains types used to configure benchmarks and pass context
//! to hooks and generator functions:
//!
//! - **Configuration**: [`HttpMethod`], [`RequestConfig`], [`RequestSource`]
//! - **Generator Contexts**: [`RequestContext`], [`RateContext`]
//! - **Hook Contexts**: [`BeforeRequestContext`], [`AfterRequestContext`]
//! - **Hook Control**: [`HookAction`]
//! - **Function Types**: [`RequestGenerator`], [`RateFunction`], [`BeforeRequestHook`], [`AfterRequestHook`]
//!
//! # Examples
//!
//! Using request context for dynamic URLs:
//! ```no_run
//! use httpress::{Benchmark, RequestContext, RequestConfig, HttpMethod};
//! use std::collections::HashMap;
//!
//! # #[tokio::main]
//! # async fn main() -> httpress::Result<()> {
//! Benchmark::builder()
//!     .request_fn(|ctx: RequestContext| {
//!         RequestConfig {
//!             url: format!("http://localhost:3000/user/{}", ctx.request_number),
//!             method: HttpMethod::Get,
//!             headers: HashMap::new(),
//!             body: None,
//!         }
//!     })
//!     .requests(100)
//!     .build()?;
//! # Ok(())
//! # }
//! ```

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;

use indicatif::ProgressBar;

use crate::cli::Cli;
use crate::error::Error;
use crate::progress::{ProgressFn, create_progress_bar, update_progress_bar};

/// Defines when the benchmark should stop
#[derive(Debug, Clone)]
pub enum StopCondition {
    /// Stop after N requests
    Requests(usize),
    /// Stop after duration
    Duration(Duration),
    /// Run until interrupted (Ctrl+C)
    Infinite,
}

/// HTTP method for requests.
///
/// Specifies the HTTP method to use when making requests to the target server.
///
/// # Examples
///
/// ```
/// use httpress::HttpMethod;
///
/// let method = HttpMethod::Post;
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum HttpMethod {
    /// HTTP GET method.
    Get,
    /// HTTP POST method.
    Post,
    /// HTTP PUT method.
    Put,
    /// HTTP DELETE method.
    Delete,
    /// HTTP PATCH method.
    Patch,
    /// HTTP HEAD method.
    Head,
    /// HTTP OPTIONS method.
    Options,
}

/// Configuration for a single HTTP request.
///
/// Used by custom request generator functions to specify the details of each request.
/// When using [`BenchmarkBuilder::request_fn`](crate::BenchmarkBuilder::request_fn),
/// your function returns this struct to configure each individual request.
///
/// # Examples
///
/// ```
/// use httpress::{RequestConfig, HttpMethod};
/// use std::collections::HashMap;
/// use bytes::Bytes;
///
/// let config = RequestConfig {
///     url: "http://localhost:3000/api/users".to_string(),
///     method: HttpMethod::Post,
///     headers: HashMap::from([
///         ("Content-Type".to_string(), "application/json".to_string()),
///         ("Authorization".to_string(), "Bearer token123".to_string()),
///     ]),
///     body: Some(Bytes::from(r#"{"name": "John"}"#)),
/// };
/// ```
#[derive(Debug, Clone)]
pub struct RequestConfig {
    /// The target URL for this request.
    pub url: String,

    /// The HTTP method to use.
    pub method: HttpMethod,

    /// HTTP headers to include in the request.
    pub headers: HashMap<String, String>,

    /// Optional request body.
    pub body: Option<Bytes>,
}

/// Context passed to request generator functions.
///
/// Provides information about the current request context, allowing you to generate
/// different requests based on worker ID and request number.
///
/// # Examples
///
/// ```no_run
/// # use httpress::{Benchmark, RequestContext, RequestConfig, HttpMethod};
/// # use std::collections::HashMap;
/// # #[tokio::main]
/// # async fn main() -> httpress::Result<()> {
/// Benchmark::builder()
///     .request_fn(|ctx: RequestContext| {
///         // Rotate through 100 different user IDs
///         let user_id = ctx.request_number % 100;
///
///         // Add worker ID to headers for debugging
///         let mut headers = HashMap::new();
///         headers.insert("X-Worker-Id".to_string(), ctx.worker_id.to_string());
///
///         RequestConfig {
///             url: format!("http://localhost:3000/user/{}", user_id),
///             method: HttpMethod::Get,
///             headers,
///             body: None,
///         }
///     })
///     .concurrency(10)
///     .requests(1000)
///     .build()?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Copy)]
pub struct RequestContext {
    /// ID of the worker executing this request (0-based).
    ///
    /// Each concurrent worker has a unique ID from 0 to (concurrency - 1).
    pub worker_id: usize,

    /// Sequential request number for this worker (0-based).
    ///
    /// This increments for each request made by this specific worker.
    pub request_number: usize,
}

/// Context passed to rate generator functions.
///
/// Provides runtime information about the benchmark state, allowing you to dynamically
/// adjust the request rate based on elapsed time, request counts, or success rates.
///
/// # Examples
///
/// ```no_run
/// # use httpress::{Benchmark, RateContext};
/// # use std::time::Duration;
/// # #[tokio::main]
/// # async fn main() -> httpress::Result<()> {
/// Benchmark::builder()
///     .url("http://localhost:3000")
///     .rate_fn(|ctx: RateContext| {
///         let elapsed_secs = ctx.elapsed.as_secs_f64();
///
///         // Linear ramp from 100 to 1000 req/s over 10 seconds
///         if elapsed_secs < 10.0 {
///             let progress = elapsed_secs / 10.0;
///             100.0 + (900.0 * progress)
///         } else {
///             1000.0
///         }
///     })
///     .duration(Duration::from_secs(30))
///     .build()?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Copy)]
pub struct RateContext {
    /// Time elapsed since benchmark start.
    pub elapsed: Duration,

    /// Total requests completed so far (success + failure).
    pub total_requests: usize,

    /// Successful requests so far (HTTP status 2xx).
    pub successful_requests: usize,

    /// Failed requests so far (non-2xx status or connection errors).
    pub failed_requests: usize,

    /// Current configured rate in requests per second (for reference).
    ///
    /// This reflects the rate returned by the previous call to the rate function.
    pub current_rate: f64,
}

/// Type alias for request generator functions.
///
/// A request generator is a function that creates a [`RequestConfig`] for each request
/// based on the provided [`RequestContext`]. This allows you to dynamically generate
/// requests with different URLs, methods, headers, or bodies.
///
/// # Type Signature
///
/// ```text
/// Fn(RequestContext) -> RequestConfig + Send + Sync + 'static
/// ```
///
/// # Examples
///
/// See [`BenchmarkBuilder::request_fn`](crate::BenchmarkBuilder::request_fn) for usage examples.
pub type RequestGenerator = Arc<dyn Fn(RequestContext) -> RequestConfig + Send + Sync>;

/// Type alias for rate generator functions.
///
/// A rate function dynamically determines the request rate (requests per second) based
/// on the current benchmark state provided in [`RateContext`]. This enables advanced
/// patterns like rate ramping, adaptive rate control, or scheduled rate changes.
///
/// # Type Signature
///
/// ```text
/// Fn(RateContext) -> f64 + Send + Sync + 'static
/// ```
///
/// The returned `f64` value represents the desired requests per second.
///
/// # Examples
///
/// See [`BenchmarkBuilder::rate_fn`](crate::BenchmarkBuilder::rate_fn) for usage examples.
pub type RateFunction = Arc<dyn Fn(RateContext) -> f64 + Send + Sync>;

/// Context passed to before-request hook functions.
///
/// Provides information about the benchmark state before a request is sent.
/// Before-request hooks can use this to implement rate limiting, circuit breakers,
/// or conditional request execution.
///
/// # Examples
///
/// ```no_run
/// # use httpress::{Benchmark, BeforeRequestContext, HookAction};
/// # #[tokio::main]
/// # async fn main() -> httpress::Result<()> {
/// Benchmark::builder()
///     .url("http://localhost:3000")
///     .before_request(|ctx: BeforeRequestContext| {
///         // Circuit breaker: stop sending requests if too many failures
///         let failure_rate = ctx.failed_requests as f64 / ctx.total_requests.max(1) as f64;
///         if failure_rate > 0.5 && ctx.total_requests > 100 {
///             println!("Circuit breaker triggered at {}% failures", failure_rate * 100.0);
///             HookAction::Abort
///         } else {
///             HookAction::Continue
///         }
///     })
///     .requests(1000)
///     .build()?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Copy)]
pub struct BeforeRequestContext {
    /// ID of the worker executing this request (0-based).
    pub worker_id: usize,

    /// Sequential request number for this worker (0-based).
    pub request_number: usize,

    /// Time elapsed since benchmark start.
    pub elapsed: Duration,

    /// Total requests completed so far (success + failure).
    pub total_requests: usize,

    /// Successful requests so far (HTTP status 2xx).
    pub successful_requests: usize,

    /// Failed requests so far (non-2xx status or connection errors).
    pub failed_requests: usize,
}

/// Context passed to after-request hook functions.
///
/// Provides detailed information about a completed request, including latency and status code.
/// After-request hooks can use this for custom metrics collection, retry logic based on
/// response status, or conditional behavior based on performance.
///
/// # Examples
///
/// ```no_run
/// # use httpress::{Benchmark, AfterRequestContext, HookAction};
/// # use std::sync::{Arc, Mutex};
/// # use std::time::Duration;
/// # #[tokio::main]
/// # async fn main() -> httpress::Result<()> {
/// let slow_request_count = Arc::new(Mutex::new(0));
/// let slow_count_clone = slow_request_count.clone();
///
/// Benchmark::builder()
///     .url("http://localhost:3000")
///     .after_request(move |ctx: AfterRequestContext| {
///         // Track slow requests (> 100ms)
///         if ctx.latency > Duration::from_millis(100) {
///             let mut count = slow_count_clone.lock().unwrap();
///             *count += 1;
///         }
///
///         // Retry on 5xx errors
///         if let Some(status) = ctx.status {
///             if status >= 500 {
///                 return HookAction::Retry;
///             }
///         }
///
///         HookAction::Continue
///     })
///     .max_retries(3)
///     .requests(1000)
///     .build()?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Copy)]
pub struct AfterRequestContext {
    /// ID of the worker that executed this request (0-based).
    pub worker_id: usize,

    /// Sequential request number for this worker (0-based).
    pub request_number: usize,

    /// Time elapsed since benchmark start.
    pub elapsed: Duration,

    /// Total requests completed so far (success + failure).
    pub total_requests: usize,

    /// Successful requests so far (HTTP status 2xx).
    pub successful_requests: usize,

    /// Failed requests so far (non-2xx status or connection errors).
    pub failed_requests: usize,

    /// Time taken for this request (latency).
    pub latency: Duration,

    /// HTTP status code if the request succeeded, `None` if it failed.
    pub status: Option<u16>,
}

/// Action returned by hook functions to control request execution.
///
/// Hook functions (both before-request and after-request) return this enum to signal
/// what action the benchmark executor should take for the current request.
///
/// # Examples
///
/// ```no_run
/// # use httpress::{Benchmark, AfterRequestContext, HookAction};
/// # #[tokio::main]
/// # async fn main() -> httpress::Result<()> {
/// Benchmark::builder()
///     .url("http://localhost:3000")
///     .after_request(|ctx: AfterRequestContext| {
///         match ctx.status {
///             Some(status) if status >= 500 => {
///                 // Retry server errors
///                 HookAction::Retry
///             }
///             Some(status) if status == 429 => {
///                 // Abort on rate limiting
///                 HookAction::Abort
///             }
///             _ => {
///                 // Continue normally
///                 HookAction::Continue
///             }
///         }
///     })
///     .max_retries(3)
///     .requests(1000)
///     .build()?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookAction {
    /// Continue with normal execution.
    ///
    /// The request proceeds normally. This is the typical return value.
    Continue,

    /// Abort this request without retrying.
    ///
    /// The request is counted as failed, but the benchmark continues with other requests.
    /// Use this for requests that should be skipped (e.g., circuit breaker triggered).
    Abort,

    /// Retry this request.
    ///
    /// The request will be retried up to the configured `max_retries` limit.
    /// Use this for transient errors that might succeed on retry (e.g., 5xx errors).
    Retry,
}

/// Type alias for before-request hook functions.
///
/// Before-request hooks are called before sending each HTTP request. They receive
/// [`BeforeRequestContext`] and return [`HookAction`] to control execution flow.
///
/// # Type Signature
///
/// ```text
/// Fn(BeforeRequestContext) -> HookAction + Send + Sync + 'static
/// ```
///
/// # Examples
///
/// See [`BenchmarkBuilder::before_request`](crate::BenchmarkBuilder::before_request) for usage examples.
pub type BeforeRequestHook = Arc<dyn Fn(BeforeRequestContext) -> HookAction + Send + Sync>;

/// Type alias for after-request hook functions.
///
/// After-request hooks are called after each HTTP request completes (or fails).
/// They receive [`AfterRequestContext`] with request latency and status code,
/// and return [`HookAction`] to control execution flow.
///
/// # Type Signature
///
/// ```text
/// Fn(AfterRequestContext) -> HookAction + Send + Sync + 'static
/// ```
///
/// # Examples
///
/// See [`BenchmarkBuilder::after_request`](crate::BenchmarkBuilder::after_request) for usage examples.
pub type AfterRequestHook = Arc<dyn Fn(AfterRequestContext) -> HookAction + Send + Sync>;

/// Source of request configuration - either static or dynamically generated.
///
/// This enum represents how requests are configured in a benchmark. It is used
/// internally by the builder and executor, but is exposed publicly as part of
/// the configuration API.
///
/// You typically don't construct this directly; instead use
/// [`BenchmarkBuilder::url`](crate::BenchmarkBuilder::url) for static configuration or
/// [`BenchmarkBuilder::request_fn`](crate::BenchmarkBuilder::request_fn) for dynamic generation.
#[derive(Clone)]
pub enum RequestSource {
    /// Static configuration used for all requests.
    ///
    /// Created when using [`BenchmarkBuilder::url`](crate::BenchmarkBuilder::url).
    Static(RequestConfig),

    /// Dynamic generator function called for each request.
    ///
    /// Created when using [`BenchmarkBuilder::request_fn`](crate::BenchmarkBuilder::request_fn).
    Dynamic(RequestGenerator),
}

/// Output format for benchmark results.
///
/// Controls how results are rendered when using the `--output`/`-o` CLI flag.
/// Defaults to [`OutputFormat::Text`].
#[derive(Clone, Copy, clap::ValueEnum)]
pub enum OutputFormat {
    Text,
    Json,
}

/// Benchmark configuration
#[derive(Clone)]
pub struct BenchConfig {
    pub request_source: RequestSource,
    pub concurrency: usize,
    pub stop_condition: StopCondition,
    pub timeout: Duration,
    pub rate: Option<u64>,
    pub rate_fn: Option<RateFunction>,
    pub before_request_hooks: Vec<BeforeRequestHook>,
    pub after_request_hooks: Vec<AfterRequestHook>,
    pub max_retries: usize,
    pub progress_fn: Option<ProgressFn>,
    pub insecure: bool,
}

impl BenchConfig {
    /// Create config from CLI arguments
    pub fn from_args(args: Cli) -> Result<Self, Error> {
        let url = args.url.ok_or_else(|| Error::MissingUrl)?;

        let stop_condition = match (args.requests, args.duration) {
            (Some(n), None) => StopCondition::Requests(n),
            (None, Some(d)) => StopCondition::Duration(parse_duration(&d)?),
            (None, None) => StopCondition::Infinite,
            (Some(_), Some(_)) => unreachable!("clap prevents this"),
        };

        let headers = parse_headers(&args.headers)?;

        let request_config = RequestConfig {
            url,
            method: args.method,
            headers,
            body: args.body.map(Bytes::from),
        };

        Ok(BenchConfig {
            request_source: RequestSource::Static(request_config),
            concurrency: args.concurrency,
            stop_condition,
            timeout: Duration::from_secs(args.timeout),
            rate: args.rate,
            rate_fn: None,
            before_request_hooks: Vec::new(),
            after_request_hooks: Vec::new(),
            max_retries: 3,
            progress_fn: None,
            insecure: args.insecure,
        })
    }

    /// Attach a built-in terminal progress bar and return the updated config
    /// alongside the bar handle (used to call `finish_and_clear` after the run).
    pub fn with_progress(mut self) -> (Self, Arc<ProgressBar>) {
        let pb = Arc::new(create_progress_bar(&self.stop_condition));
        let pb_fn = Arc::clone(&pb);
        self.progress_fn = Some(Arc::new(move |snap| update_progress_bar(&pb_fn, &snap)));
        (self, pb)
    }
}

/// Parse duration string like "10s", "1m", "500ms"
fn parse_duration(s: &str) -> Result<Duration, Error> {
    let s = s.trim();

    if let Some(ms) = s.strip_suffix("ms") {
        let ms: u64 = ms
            .parse()
            .map_err(|_| Error::InvalidDuration(s.to_string()))?;
        return Ok(Duration::from_millis(ms));
    }

    if let Some(secs) = s.strip_suffix('s') {
        let secs: u64 = secs
            .parse()
            .map_err(|_| Error::InvalidDuration(s.to_string()))?;
        return Ok(Duration::from_secs(secs));
    }

    if let Some(mins) = s.strip_suffix('m') {
        let mins: u64 = mins
            .parse()
            .map_err(|_| Error::InvalidDuration(s.to_string()))?;
        return Ok(Duration::from_secs(mins * 60));
    }

    // Try parsing as plain seconds
    if let Ok(secs) = s.parse::<u64>() {
        return Ok(Duration::from_secs(secs));
    }

    Err(Error::InvalidDuration(s.to_string()))
}

/// Parse header strings like "Content-Type: application/json"
fn parse_headers(headers: &[String]) -> Result<HashMap<String, String>, Error> {
    let mut map = HashMap::new();

    for h in headers {
        let (key, value) = h
            .split_once(':')
            .ok_or_else(|| Error::InvalidHeader(h.clone()))?;

        map.insert(key.trim().to_string(), value.trim().to_string());
    }

    Ok(map)
}
