use crate::config::{HttpMethod, OutputFormat};
use clap::{Parser, Subcommand};
use clap_complete::Shell;


/// An API benchmark tool built with rust
#[derive(Parser)]
#[command(name = "httpress")]
#[command(version, about = "An API benchmark tool built with rust")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Target URL to bench
    pub url: Option<String>,

    /// HTTP method
    #[arg(short, long, value_enum, default_value = "get")]
    pub method: HttpMethod,

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

    /// Skip TLS certificate verification
    #[arg(short = 'k', long)]
    pub insecure: bool,

    /// Output serialized into provided format
    #[arg(short = 'o', long, value_enum, default_value_t = OutputFormat::Text)]
    pub output: OutputFormat,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Generate shell completion scripts
    Completions {
        /// The shell to generate completions for
        #[arg(value_enum)]
        shell: Shell,
    },
}
