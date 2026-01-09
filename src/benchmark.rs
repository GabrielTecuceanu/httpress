use std::collections::HashMap;
use std::time::Duration;

use crate::client::HttpClient;
use crate::config::{BenchConfig, HttpMethod, RequestConfig, RequestSource, StopCondition};
use crate::error::{Error, Result};
use crate::executor::Executor;
use crate::metrics::BenchmarkResults;

/// Builder for configuring and running benchmarks
pub struct BenchmarkBuilder {
    url: Option<String>,
    method: HttpMethod,
    concurrency: usize,
    stop_condition: StopCondition,
    headers: HashMap<String, String>,
    body: Option<String>,
    timeout: Duration,
    rate: Option<u64>,
}

impl BenchmarkBuilder {
    /// Create a new builder with default settings
    pub fn new() -> Self {
        BenchmarkBuilder {
            url: None,
            method: HttpMethod::Get,
            concurrency: 10,
            stop_condition: StopCondition::Infinite,
            headers: HashMap::new(),
            body: None,
            timeout: Duration::from_secs(30),
            rate: None,
        }
    }

    /// Set the target URL (required)
    pub fn url(mut self, url: &str) -> Self {
        self.url = Some(url.to_string());
        self
    }

    /// Set the HTTP method (default: GET)
    pub fn method(mut self, method: HttpMethod) -> Self {
        self.method = method;
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

    /// Build the benchmark
    pub fn build(self) -> Result<Benchmark> {
        let url = self.url.ok_or(Error::MissingUrl)?;

        let request_config = RequestConfig {
            url,
            method: self.method,
            headers: self.headers,
            body: self.body,
        };

        let config = BenchConfig {
            request_source: RequestSource::Static(request_config),
            concurrency: self.concurrency,
            stop_condition: self.stop_condition,
            timeout: self.timeout,
            rate: self.rate,
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
