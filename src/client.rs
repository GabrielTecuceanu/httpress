use std::time::Duration;

use reqwest::{Client, Response};

use crate::config::{BenchConfig, HttpMethod};
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
        let mut request = match config.method {
            HttpMethod::Get => self.client.get(&config.url),
            HttpMethod::Post => self.client.post(&config.url),
            HttpMethod::Put => self.client.put(&config.url),
            HttpMethod::Delete => self.client.delete(&config.url),
            HttpMethod::Patch => self.client.patch(&config.url),
            HttpMethod::Head => self.client.head(&config.url),
            HttpMethod::Options => self.client.request(reqwest::Method::OPTIONS, &config.url),
        };

        for (key, value) in &config.headers {
            request = request.header(key, value);
        }

        if let Some(ref body) = config.body {
            request = request.body(body.clone());
        }

        let response = request.send().await?;
        
        Ok(response)
    }
}
