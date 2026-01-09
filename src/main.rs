use clap::Parser;

use httpress::cli::Args;
use httpress::client::HttpClient;
use httpress::config::BenchConfig;
use httpress::executor::Executor;

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
    
    if let Some(rate) = &config.rate {
        println!("Rate limit: {} req/s", rate);
    }

    // Create HTTP client
    let client = match HttpClient::new(config.timeout) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to create HTTP client: {}", e);
            std::process::exit(1);
        }
    };

    // Run the benchmark
    let executor = Executor::new(client, config);
    if let Err(e) = executor.run().await {
        eprintln!("Benchmark failed: {}", e);
        std::process::exit(1);
    }

   
}
