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