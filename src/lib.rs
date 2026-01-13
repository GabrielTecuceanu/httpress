//! # httpress
//!
//! A fast HTTP benchmarking library built in Rust.
//!
//! `httpress` provides a simple yet powerful API for load testing HTTP services. It supports
//! concurrent requests, rate limiting, custom request generation, and hooks for metrics collection.
//!
//! ## Quick Start
//!
//! ```no_run
//! use httpress::{Benchmark, Result};
//! use std::time::Duration;
//!
//! #[tokio::main]
//! async fn main() -> Result<()> {
//!     let results = Benchmark::builder()
//!         .url("http://localhost:3000")
//!         .concurrency(50)
//!         .duration(Duration::from_secs(10))
//!         .build()?
//!         .run()
//!         .await?;
//!
//!     results.print();
//!     Ok(())
//! }
//! ```
//!
//! ## Features
//!
//! - **Simple API**: Builder pattern for easy configuration
//! - **Flexible Rate Control**: Fixed rates or dynamic rate functions
//! - **Custom Request Generation**: Generate requests dynamically per-worker
//! - **Hook System**: Inject custom logic before/after requests
//! - **Detailed Metrics**: Latency percentiles, throughput, status codes

pub mod benchmark;
pub mod cli;
pub mod client;
pub mod config;
pub mod error;
pub mod executor;
pub mod metrics;

// Re-export main types for library users
pub use benchmark::{Benchmark, BenchmarkBuilder};
pub use config::{
    AfterRequestContext, AfterRequestHook, BeforeRequestContext, BeforeRequestHook, HookAction,
    HttpMethod, RateContext, RateFunction, RequestConfig, RequestContext, RequestGenerator,
    RequestSource,
};
pub use error::{Error, Result};
pub use metrics::BenchmarkResults;