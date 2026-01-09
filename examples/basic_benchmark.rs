use httpress::{Benchmark, Result};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
    // Run a simple benchmark
    let results = Benchmark::builder()
        .url("http://localhost:3000")
        .concurrency(50)
        .duration(Duration::from_secs(10))
        .build()?
        .run()
        .await?;

    // Print results using the built-in formatter
    results.print();

    // Or access individual metrics programmatically
    println!("\n--- Programmatic Access ---");
    println!("Total requests: {}", results.total_requests);
    println!("Success rate: {:.2}%",
        (results.successful_requests as f64 / results.total_requests as f64) * 100.0
    );
    println!("Throughput: {:.2} req/s", results.throughput);
    println!("p99 latency: {:?}", results.latency_p99);

    Ok(())
}
