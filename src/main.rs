use clap::{CommandFactory, Parser};
use clap_complete::generate;
use httpress::cli::{Cli, Commands};
use httpress::client::HttpClient;
use httpress::config::{BenchConfig, RequestSource};
use httpress::executor::Executor;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // Handle the `completions` subcommand before any benchmarking logic.
    if let Some(Commands::Completions { shell }) = cli.command {
        let mut cmd = Cli::command();
        let name = cmd.get_name().to_string();
        generate(shell, &mut cmd, name, &mut std::io::stdout());
        std::process::exit(0);
    }

    let config = match BenchConfig::from_args(cli) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    let RequestSource::Static(req) = &config.request_source else {
        unreachable!("CLI only creates Static requests")
    };

    println!("Target: {} {:?}", req.url, req.method);
    println!("Concurrency: {}", config.concurrency);
    println!("Stop condition: {:?}", config.stop_condition);

    if let Some(rate) = &config.rate {
        println!("Rate limit: {} req/s", rate);
    }

    let client = match HttpClient::new(config.timeout, config.concurrency, config.insecure) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to create HTTP client: {}", e);
            std::process::exit(1);
        }
    };

    println!(
        "\nStarting benchmark with {} workers...",
        config.concurrency
    );

    let (config, pb) = config.with_progress();

    let executor = Executor::new(client, config);
    match executor.run().await {
        Ok(results) => {
            pb.finish_and_clear();
            results.print();
        }
        Err(e) => {
            pb.finish_and_clear();
            eprintln!("Benchmark failed: {}", e);
            std::process::exit(1);
        }
    }
}
