use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use crate::client::HttpClient;
use crate::config::{
    AfterRequestHook, BeforeRequestHook, BenchConfig, HttpMethod, RateContext, RateFunction,
    RequestConfig, RequestContext, RequestGenerator, RequestSource, StopCondition,
};
use crate::error::{Error, Result};
use crate::executor::Executor;
use crate::metrics::BenchmarkResults;

/// Builder for configuring and running benchmarks
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
}

impl BenchmarkBuilder {
    /// Create a new builder with default settings
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
        }
    }

    /// Set the target URL (required)
    pub fn url(mut self, url: &str) -> Self {
        self.url = Some(url.to_string());
        self
    }

    /// Set the HTTP method (default: GET)
    pub fn method(mut self, method: HttpMethod) -> Self {
        self.method = Some(method);
        self
    }

    /// Set the number of concurrent connections (default: 10)
    pub fn concurrency(mut self, n: usize) -> Self {
        self.concurrency = n;
        self
    }

    /// Set the test duration
    pub fn duration(mut self, d: Duration) -> Self {
        self.stop_condition = StopCondition::Duration(d);
        self
    }

    /// Set the total number of requests
    pub fn requests(mut self, n: usize) -> Self {
        self.stop_condition = StopCondition::Requests(n);
        self
    }

    /// Set the target requests per second
    pub fn rate(mut self, rps: u64) -> Self {
        self.rate = Some(rps);
        self
    }

    /// Set a dynamic rate function (mutually exclusive with rate())
    pub fn rate_fn<F>(mut self, f: F) -> Self
    where
        F: Fn(RateContext) -> f64 + Send + Sync + 'static,
    {
        self.rate_fn = Some(Arc::new(f));
        self
    }

    /// Add a header
    pub fn header(mut self, key: &str, value: &str) -> Self {
        self.headers.insert(key.to_string(), value.to_string());
        self
    }

    /// Set the request body
    pub fn body(mut self, body: &str) -> Self {
        self.body = Some(body.to_string());
        self
    }

    /// Set the request timeout (default: 30s)
    pub fn timeout(mut self, d: Duration) -> Self {
        self.timeout = d;
        self
    }

    /// Set a custom request generator function (mutually exclusive with url())
    pub fn request_fn<F>(mut self, f: F) -> Self
    where
        F: Fn(RequestContext) -> RequestConfig + Send + Sync + 'static,
    {
        self.request_fn = Some(Arc::new(f));
        self
    }

    /// Add a before_request hook (can be called multiple times for chaining)
    pub fn before_request<F>(mut self, f: F) -> Self
    where
        F: Fn(crate::config::BeforeRequestContext) -> crate::config::HookAction + Send + Sync + 'static,
    {
        self.before_request_hooks.push(Arc::new(f));
        self
    }

    /// Add an after_request hook (can be called multiple times for chaining)
    pub fn after_request<F>(mut self, f: F) -> Self
    where
        F: Fn(crate::config::AfterRequestContext) -> crate::config::HookAction + Send + Sync + 'static,
    {
        self.after_request_hooks.push(Arc::new(f));
        self
    }

    /// Set maximum number of retries when hooks return Retry (default: 3)
    pub fn max_retries(mut self, n: usize) -> Self {
        self.max_retries = n;
        self
    }

    /// Build the benchmark
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
                ))
            }
            (None, None) => {
                return Err(Error::InvalidConfig(
                    "Must provide either url() or request_fn()".to_string(),
                ))
            }
            (Some(url), None) => {
                let request_config = RequestConfig {
                    url,
                    method: self.method.unwrap_or(HttpMethod::Get),
                    headers: self.headers,
                    body: self.body,
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
        };

        Ok(Benchmark { config })
    }
}

impl Default for BenchmarkBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// A configured benchmark ready to run
pub struct Benchmark {
    config: BenchConfig,
}

impl Benchmark {
    /// Create a new benchmark builder
    pub fn builder() -> BenchmarkBuilder {
        BenchmarkBuilder::new()
    }

    /// Run the benchmark and return results
    pub async fn run(self) -> Result<BenchmarkResults> {
        let client = HttpClient::new(self.config.timeout)?;
        let executor = Executor::new(client, self.config);
        executor.run().await
    }
}
