use std::time::Duration;

use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::Request;
use hyper_tls::HttpsConnector;
use hyper_util::client::legacy::Client;
use hyper_util::client::legacy::Error as HyperError;
use hyper_util::rt::TokioExecutor;

use crate::config::{BenchConfig, HttpMethod, RequestConfig, RequestContext, RequestSource};
use crate::error::{Error, Result};
use std::error::Error as StdError;

/// HTTP client wrapper for benchmark requests
pub struct HttpClient {
    client: Client<HttpsConnector<hyper_util::client::legacy::connect::HttpConnector>, Full<Bytes>>,
    timeout: Duration,
}

impl HttpClient {
    /// Create a new HTTP client with the given timeout and connection pool settings
    pub fn new(timeout: Duration, concurrency: usize, insecure: bool) -> Result<Self> {
        let mut connector = hyper_util::client::legacy::connect::HttpConnector::new();
        connector.enforce_http(false);
        connector.set_nodelay(true);
        connector.set_keepalive(Some(Duration::from_secs(60)));

        let https = if insecure {
            let mut tls_builder = native_tls::TlsConnector::builder();
            tls_builder.danger_accept_invalid_certs(true);
            tls_builder.danger_accept_invalid_hostnames(true);
            let tls = tls_builder
                .build()
                .map_err(|e| crate::error::Error::Http(e.into()))?;
            HttpsConnector::from((connector, tls.into()))
        } else {
            HttpsConnector::new_with_connector(connector)
        };

        let client = Client::builder(TokioExecutor::new())
            .pool_idle_timeout(Duration::from_secs(90))
            .pool_max_idle_per_host(concurrency)
            .pool_timer(hyper_util::rt::TokioTimer::new())
            .build(https);

        Ok(HttpClient { client, timeout })
    }

    /// Execute a request, dispatching based on the request source (static or dynamic).
    /// Returns (status_code, bytes_received).
    pub async fn execute_for_worker(
        &self,
        config: &BenchConfig,
        worker_id: usize,
        request_number: usize,
    ) -> Result<(Option<u16>, usize)> {
        match &config.request_source {
            RequestSource::Static(req) => self.execute_request(req).await,
            RequestSource::Dynamic(generator) => {
                let ctx = RequestContext {
                    worker_id,
                    request_number,
                };
                self.execute_request(&generator(ctx)).await
            }
        }
    }

    /// Execute a single HTTP request from RequestConfig.
    /// Returns (status_code, bytes_received).
    pub async fn execute_request(&self, req: &RequestConfig) -> Result<(Option<u16>, usize)> {
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
            .map_err(|_| Error::Timeout)?
            .map_err(|e| self.classify_error(e))?;

        let status = response.status().as_u16();

        // Consume body to allow connection reuse (HEAD has no body)
        let bytes = if req.method != HttpMethod::Head {
            response
                .into_body()
                .collect()
                .await
                .map(|b| b.to_bytes().len())
                .unwrap_or(0)
        } else {
            0
        };

        Ok((Some(status), bytes))
    }
    fn classify_error(&self, err: HyperError) -> Error {
        let mut source = err.source();
        while let Some(err) = source {
            // TLS Errors Check (native_tls)
            if let Some(tls_err) = err.downcast_ref::<native_tls::Error>() {
                return Error::TlsError(tls_err.to_string());
            }
            // IO Errors Check
            if let Some(io_err) = err.downcast_ref::<std::io::Error>() {
                let io_err_msg = io_err.to_string().to_lowercase();
                // DNS Error - Check
                const DNS_PATTERNS: &[&str] = &[
                    "failed to lookup address information",
                    "name or service not known",
                    "dns",
                    "no such host",
                    "temporary failure in name resolution",
                ];
                if DNS_PATTERNS
                    .iter()
                    .any(|pattern| io_err_msg.contains(pattern))
                {
                    return Error::DnsError;
                }
                return match io_err.kind() {
                    std::io::ErrorKind::ConnectionRefused => Error::ConnectionRefused,
                    std::io::ErrorKind::ConnectionReset => Error::ConnectionReset,
                    std::io::ErrorKind::TimedOut => Error::Timeout,
                    _ => Error::Other(io_err.to_string()),
                };
            }
            source = err.source();
        }
        let err_msg = err.to_string().to_lowercase();
        // Hyper Error - Common Errors Check
        if err_msg.contains("connection reset") {
            return Error::ConnectionReset;
        }
        if err_msg.contains("connection refused") {
            return Error::ConnectionRefused;
        }
        // Generic HTTP Error
        Error::Http(Box::new(err))
    }
}
