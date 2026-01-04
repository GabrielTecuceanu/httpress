use clap::Parser;
use httpress::cli::Args;

fn main() {
    let args = Args::parse();

    println!("Target URL: {}", args.url);
    println!("Method: {:?}", args.method);
    println!("Concurrency: {}", args.concurrency);

    if let Some(n) = args.requests {
        println!("Requests: {}", n);
    }

    if let Some(ref d) = args.duration {
        println!("Duration: {}", d);
    }

    if !args.headers.is_empty() {
        println!("Headers: {:?}", args.headers);
    }

    if let Some(ref body) = args.body {
        println!("Body: {}", body);
    }

    println!("Timeout: {}s", args.timeout);
}
