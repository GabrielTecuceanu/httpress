use std::time::Duration;

use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::Request;
use hyper_tls::HttpsConnector;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;

use crate::config::{BenchConfig, HttpMethod, RequestConfig, RequestSource};
use crate::error::Result;

/// HTTP client wrapper for benchmark requests
pub struct HttpClient {
    client: Client<HttpsConnector<hyper_util::client::legacy::connect::HttpConnector>, Full<Bytes>>,
    timeout: Duration,
}

impl HttpClient {
    /// Create a new HTTP client with the given timeout and connection pool settings
    pub fn new(timeout: Duration, concurrency: usize) -> Result<Self> {
        let mut connector = hyper_util::client::legacy::connect::HttpConnector::new();
        connector.set_nodelay(true);
        connector.set_keepalive(Some(Duration::from_secs(60)));

        let https = HttpsConnector::new_with_connector(connector);

        let client = Client::builder(TokioExecutor::new())
            .pool_idle_timeout(Duration::from_secs(90))
            .pool_max_idle_per_host(concurrency)
            .pool_timer(hyper_util::rt::TokioTimer::new())
            .build(https);

        Ok(HttpClient { client, timeout })
    }

    /// Execute a single HTTP request based on config
    pub async fn execute(&self, config: &BenchConfig) -> Result<Option<u16>> {
        match &config.request_source {
            RequestSource::Static(req) => self.execute_request(req).await,
            RequestSource::Dynamic(_) => {
                unreachable!("execute() should not be called with Dynamic request source")
            }
        }
    }

    /// Execute a single HTTP request from RequestConfig
    pub async fn execute_request(&self, req: &RequestConfig) -> Result<Option<u16>> {
        let method = match req.method {
            HttpMethod::Get => hyper::Method::GET,
            HttpMethod::Post => hyper::Method::POST,
            HttpMethod::Put => hyper::Method::PUT,
            HttpMethod::Delete => hyper::Method::DELETE,
            HttpMethod::Patch => hyper::Method::PATCH,
            HttpMethod::Head => hyper::Method::HEAD,
            HttpMethod::Options => hyper::Method::OPTIONS,
        };

        let uri: hyper::Uri = req.url.parse().map_err(|e: hyper::http::uri::InvalidUri| {
            crate::error::Error::InvalidUrl(e.to_string())
        })?;

        let body = match &req.body {
            Some(b) => Full::new(b.clone()),
            None => Full::new(Bytes::new()),
        };

        let mut builder = Request::builder().method(method).uri(uri);

        for (key, value) in &req.headers {
            builder = builder.header(key.as_str(), value.as_str());
        }

        let request = builder
            .body(body)
            .map_err(|e| crate::error::Error::Http(e.into()))?;

        let response = tokio::time::timeout(self.timeout, self.client.request(request))
            .await
            .map_err(|_| crate::error::Error::Timeout)?
            .map_err(|e| crate::error::Error::Http(e.into()))?;

        let status = response.status().as_u16();

        // Consume body to allow connection reuse (HEAD has no body)
        if req.method != HttpMethod::Head {
            let _ = response.into_body().collect().await;
        }

        Ok(Some(status))
    }
}
