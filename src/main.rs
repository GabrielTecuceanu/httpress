use clap::{CommandFactory, Parser};
use clap_complete::generate;
use httpress::cli::{Cli, Commands};
use httpress::client::HttpClient;
use httpress::config::{BenchConfig, OutputFormat, RequestSource};
use httpress::executor::Executor;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let output_format = cli.output;

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

    // Print banner to stderr so stdout stays clean for piped output (e.g. --output json | jq)
    eprintln!("Target: {} {:?}", req.url, req.method);
    eprintln!("Concurrency: {}", config.concurrency);
    eprintln!("Stop condition: {:?}", config.stop_condition);

    if let Some(rate) = &config.rate {
        eprintln!("Rate limit: {} req/s", rate);
    }

    let client = match HttpClient::new(config.timeout, config.concurrency, config.insecure) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to create HTTP client: {}", e);
            std::process::exit(1);
        }
    };

    eprintln!(
        "\nStarting benchmark with {} workers...",
        config.concurrency
    );

    let (config, pb) = config.with_progress();

    let executor = Executor::new(client, config);
    match executor.run().await {
        Ok(results) => {
            pb.finish_and_clear();
            match output_format {
                OutputFormat::Text => {
                    results.print();
                    // Print latency histogram after benchmark results
                    results.print_histogram();
                }
                OutputFormat::Json => match serde_json::to_string_pretty(&results) {
                    Ok(json) => println!("{}", json),
                    Err(e) => {
                        eprintln!("{}", e);
                        std::process::exit(1);
                    }
                },
            }
        }
        Err(e) => {
            pb.finish_and_clear();
            eprintln!("Benchmark failed: {}", e);
            std::process::exit(1);
        }
    }
}
