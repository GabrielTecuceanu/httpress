use httpress::{Benchmark, RateContext};
use std::time::Duration;

#[tokio::main]
async fn main() -> httpress::Result<()> {
    println!("Dynamic Rate Ramping Example");
    println!("============================\n");
    println!("This benchmark demonstrates dynamic rate control by ramping");
    println!("from 100 req/s to 1000 req/s over 10 seconds.\n");

    let results = Benchmark::builder()
        .url("http://localhost:3000")
        .concurrency(50)
        .duration(Duration::from_secs(10))
        .rate_fn(|ctx: RateContext| {
            // Linear ramp from 100 to 1000 req/s over 10 seconds
            let target_duration = 10.0;
            let elapsed_secs = ctx.elapsed.as_secs_f64();
            let progress = (elapsed_secs / target_duration).min(1.0);

            let start_rate = 100.0;
            let end_rate = 1000.0;
            let current = start_rate + (end_rate - start_rate) * progress;

            current
        })
        .build()?
        .run()
        .await?;

    println!("\n");
    results.print();

    Ok(())
}
