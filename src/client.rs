use std::time::Duration;

use reqwest::{Client, Response};

use crate::config::{BenchConfig, HttpMethod, RequestConfig, RequestSource};
use crate::error::Result;

/// HTTP client wrapper for benchmark requests
pub struct HttpClient {
    client: Client,
}

impl HttpClient {
    /// Create a new HTTP client with the given timeout
    pub fn new(timeout: Duration) -> Result<Self> {
        let client = Client::builder()
            .timeout(timeout)
            .pool_max_idle_per_host(0) // Disable connection pooling for accurate benchmarks
            .build()?;

        Ok(HttpClient { client })
    }

    /// Execute a single HTTP request based on config
    pub async fn execute(&self, config: &BenchConfig) -> Result<Response> {
        match &config.request_source {
            RequestSource::Static(req) => self.execute_request(req).await,
            RequestSource::Dynamic(_) => {
                unreachable!("execute() should not be called with Dynamic request source")
            }
        }
    }

    /// Execute a single HTTP request from RequestConfig
    pub async fn execute_request(&self, req: &RequestConfig) -> Result<Response> {
        let mut request = match req.method {
            HttpMethod::Get => self.client.get(&req.url),
            HttpMethod::Post => self.client.post(&req.url),
            HttpMethod::Put => self.client.put(&req.url),
            HttpMethod::Delete => self.client.delete(&req.url),
            HttpMethod::Patch => self.client.patch(&req.url),
            HttpMethod::Head => self.client.head(&req.url),
            HttpMethod::Options => self.client.request(reqwest::Method::OPTIONS, &req.url),
        };

        for (key, value) in &req.headers {
            request = request.header(key, value);
        }

        if let Some(ref body) = req.body {
            request = request.body(body.clone());
        }

        let response = request.send().await?;

        Ok(response)
    }
}
