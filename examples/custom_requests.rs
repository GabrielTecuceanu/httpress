use httpress::{Benchmark, HttpMethod, RequestConfig, RequestContext};
use std::collections::HashMap;

#[tokio::main]
async fn main() -> httpress::Result<()> {
    println!("Running benchmark with custom request generator...\n");

    // Example: Rotating URLs and dynamic headers
    let results = Benchmark::builder()
        .request_fn(|ctx: RequestContext| {
            // Rotate through different user IDs
            let user_id = ctx.request_number % 100;

            // Add custom headers based on worker and request number
            let mut headers = HashMap::new();
            headers.insert(
                "X-Worker-Id".to_string(),
                ctx.worker_id.to_string(),
            );
            headers.insert(
                "X-Request-Number".to_string(),
                ctx.request_number.to_string(),
            );

            // Vary request method based on request number
            let method = if ctx.request_number % 10 == 0 {
                HttpMethod::Post
            } else {
                HttpMethod::Get
            };

            RequestConfig {
                url: format!("http://localhost:3000/user/{}", user_id),
                method,
                headers,
                body: if method == HttpMethod::Post {
                    Some(format!(r#"{{"user_id": {}, "worker": {}}}"#, user_id, ctx.worker_id))
                } else {
                    None
                },
            }
        })
        .concurrency(10)
        .requests(100)
        .build()?
        .run()
        .await?;

    results.print();

    Ok(())
}
