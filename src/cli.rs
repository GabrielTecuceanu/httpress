use clap::{Parser, ValueEnum};

#[derive(Parser)]
#[command(name = "httpress")]
#[command(version, about = "An API benchmark tool built with rust")]
pub struct Args {
    /// Target URL to bench
    pub url: String,

    /// HTTP method
    #[arg(short, long, value_enum, default_value = "get")]
    pub method: Method,

    /// Number of concurrent connections
    #[arg(short, long, default_value_t = 10)]
    pub concurrency: usize,

    /// Total number of requests
    #[arg(short = 'n', long, conflicts_with = "duration")]
    pub requests: Option<usize>,

    /// Test duration (e.g. 10s, 1m)
    #[arg(short, long, conflicts_with = "requests")]
    pub duration: Option<String>,

    /// HTTP header (repeatable)
    #[arg(short = 'H', long = "header")]
    pub headers: Vec<String>,

    /// Request body
    #[arg(short, long)]
    pub body: Option<String>,

    /// Request timeout in seconds
    #[arg(short, long, default_value_t = 30)]
    pub timeout: u64,

    /// Target requests per second (rate limit)
    #[arg(short = 'r', long)]
    pub rate: Option<u64>,
}

#[derive(Clone, Debug, ValueEnum)]
pub enum Method {
    Get, Post, Put, Delete, Patch, Head, Options,
}
