use std::collections::HashMap;
use std::time::Duration;

use crate::cli::{Args, Method};

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

/// HTTP method for requests
#[derive(Debug, Clone, Copy)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
    Patch,
    Head,
    Options,
}

impl From<Method> for HttpMethod {
    fn from(m: Method) -> Self {
        match m {
            Method::Get => HttpMethod::Get,
            Method::Post => HttpMethod::Post,
            Method::Put => HttpMethod::Put,
            Method::Delete => HttpMethod::Delete,
            Method::Patch => HttpMethod::Patch,
            Method::Head => HttpMethod::Head,
            Method::Options => HttpMethod::Options,
        }
    }
}

/// Benchmark configuration
#[derive(Debug, Clone)]
pub struct BenchConfig {
    pub url: String,
    pub method: HttpMethod,
    pub concurrency: usize,
    pub stop_condition: StopCondition,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
    pub timeout: Duration,
}

impl BenchConfig {
    /// Create config from CLI arguments
    pub fn from_args(args: Args) -> Result<Self, ConfigError> {
        let stop_condition = match (args.requests, args.duration) {
            (Some(n), None) => StopCondition::Requests(n),
            (None, Some(d)) => StopCondition::Duration(parse_duration(&d)?),
            (None, None) => StopCondition::Infinite,
            (Some(_), Some(_)) => unreachable!("clap prevents this"),
        };

        let headers = parse_headers(&args.headers)?;

        Ok(BenchConfig {
            url: args.url,
            method: args.method.into(),
            concurrency: args.concurrency,
            stop_condition,
            headers,
            body: args.body,
            timeout: Duration::from_secs(args.timeout),
        })
    }
}

/// Parse duration string
fn parse_duration(s: &str) -> Result<Duration, ConfigError> {
    let s = s.trim();

    if let Some(ms) = s.strip_suffix("ms") {
        let ms: u64 = ms.parse().map_err(|_| ConfigError::InvalidDuration(s.to_string()))?;
        return Ok(Duration::from_millis(ms));
    }

    if let Some(secs) = s.strip_suffix('s') {
        let secs: u64 = secs.parse().map_err(|_| ConfigError::InvalidDuration(s.to_string()))?;
        return Ok(Duration::from_secs(secs));
    }

    if let Some(mins) = s.strip_suffix('m') {
        let mins: u64 = mins.parse().map_err(|_| ConfigError::InvalidDuration(s.to_string()))?;
        return Ok(Duration::from_secs(mins * 60));
    }

    // Try parsing as plain seconds
    if let Ok(secs) = s.parse::<u64>() {
        return Ok(Duration::from_secs(secs));
    }

    Err(ConfigError::InvalidDuration(s.to_string()))
}

/// Parse header strings
fn parse_headers(headers: &[String]) -> Result<HashMap<String, String>, ConfigError> {
    let mut map = HashMap::new();

    for h in headers {
        let (key, value) = h
            .split_once(':')
            .ok_or_else(|| ConfigError::InvalidHeader(h.clone()))?;

        map.insert(key.trim().to_string(), value.trim().to_string());
    }

    Ok(map)
}

#[derive(Debug)]
pub enum ConfigError {
    InvalidDuration(String),
    InvalidHeader(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::InvalidDuration(s) => write!(f, "Invalid duration: '{}'. Use format like 10s, 1m, 500ms", s),
            ConfigError::InvalidHeader(s) => write!(f, "Invalid header: '{}'. Use format 'Key: Value'", s),
        }
    }
}

impl std::error::Error for ConfigError {}
