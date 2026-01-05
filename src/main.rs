use clap::Parser;

use httpress::cli::Args;
use httpress::client::HttpClient;
use httpress::config::BenchConfig;

#[tokio::main]
async fn main() {
    let args = Args::parse();

    // Build config from CLI args
    let config = match BenchConfig::from_args(args) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    println!("Target: {} {:?}", config.url, config.method);
    println!("Concurrency: {}", config.concurrency);
    println!("Stop condition: {:?}", config.stop_condition);

    // Create HTTP client
    let client = match HttpClient::new(config.timeout) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to create HTTP client: {}", e);
            std::process::exit(1);
        }
    };

    // Execute a single test request
    println!("\nSending test request...");
    match client.execute(&config).await {
        Ok(response) => {
            println!("Status: {}", response.status());
            println!("Response size: {} bytes", response.content_length().unwrap_or(0));
        }
        Err(e) => {
            eprintln!("Request failed: {}", e);
            std::process::exit(1);
        }
    }
}
