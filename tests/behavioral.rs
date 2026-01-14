mod common;

use std::time::Duration;

use common::TestServer;
use httpress::Benchmark;

#[tokio::test]
async fn test_stops_at_request_count() {
    let server = TestServer::start().await;

    let results = Benchmark::builder()
        .url(&format!("{}/ok", server.base_url))
        .requests(100)
        .concurrency(4)
        .build()
        .unwrap()
        .run()
        .await
        .unwrap();

    assert_eq!(results.total_requests, 100);
    assert_eq!(results.successful_requests, 100);
    assert_eq!(results.failed_requests, 0);
}

#[tokio::test]
async fn test_stops_at_duration() {
    let server = TestServer::start().await;

    let results = Benchmark::builder()
        .url(&format!("{}/ok", server.base_url))
        .duration(Duration::from_secs(1))
        .rate(100)
        .concurrency(2)
        .build()
        .unwrap()
        .run()
        .await
        .unwrap();

    // Duration should be approximately 1 second
    assert!(
        results.duration >= Duration::from_millis(900),
        "Duration too short: {:?}",
        results.duration
    );
    assert!(
        results.duration <= Duration::from_millis(1500),
        "Duration too long: {:?}",
        results.duration
    );
}

#[tokio::test]
async fn test_rate_limiting() {
    let server = TestServer::start().await;

    let results = Benchmark::builder()
        .url(&format!("{}/ok", server.base_url))
        .requests(100)
        .rate(100) // 100 req/s = ~1 second
        .concurrency(2)
        .build()
        .unwrap()
        .run()
        .await
        .unwrap();

    // Should take approximately 1 second, not instant
    assert!(
        results.duration >= Duration::from_millis(800),
        "Rate limiting not working - completed too fast: {:?}",
        results.duration
    );

    // Throughput should be near target rate (with some tolerance)
    assert!(
        results.throughput >= 80.0 && results.throughput <= 120.0,
        "Throughput out of range: {}",
        results.throughput
    );
}

#[tokio::test]
async fn test_dynamic_rate() {
    let server = TestServer::start().await;

    let results = Benchmark::builder()
        .url(&format!("{}/ok", server.base_url))
        .requests(50)
        .concurrency(2)
        .rate_fn(|ctx| {
            // Start slow, speed up
            if ctx.elapsed < Duration::from_millis(300) {
                50.0
            } else {
                200.0
            }
        })
        .build()
        .unwrap()
        .run()
        .await
        .unwrap();

    assert_eq!(results.total_requests, 50);
    assert_eq!(results.successful_requests, 50);
}

#[tokio::test]
async fn test_concurrency_multiple_workers() {
    let server = TestServer::start().await;

    let results = Benchmark::builder()
        .url(&format!("{}/ok", server.base_url))
        .requests(100)
        .concurrency(10)
        .build()
        .unwrap()
        .run()
        .await
        .unwrap();

    assert_eq!(results.total_requests, 100);
    assert_eq!(results.successful_requests, 100);
}

#[tokio::test]
async fn test_single_request() {
    let server = TestServer::start().await;

    let results = Benchmark::builder()
        .url(&format!("{}/ok", server.base_url))
        .requests(1)
        .concurrency(1)
        .build()
        .unwrap()
        .run()
        .await
        .unwrap();

    assert_eq!(results.total_requests, 1);
    assert_eq!(results.successful_requests, 1);
    assert_eq!(results.failed_requests, 0);
}

#[tokio::test]
async fn test_latency_metrics_populated() {
    let server = TestServer::start().await;

    let results = Benchmark::builder()
        .url(&format!("{}/delay/10", server.base_url)) // 10ms delay
        .requests(20)
        .concurrency(4)
        .build()
        .unwrap()
        .run()
        .await
        .unwrap();

    assert_eq!(results.total_requests, 20);

    // Latency should be at least 10ms (the delay)
    assert!(
        results.latency_min >= Duration::from_millis(5),
        "Latency min too low: {:?}",
        results.latency_min
    );
    assert!(
        results.latency_mean >= Duration::from_millis(10),
        "Latency mean too low: {:?}",
        results.latency_mean
    );

    // Sanity checks
    assert!(results.latency_min <= results.latency_mean);
    assert!(results.latency_mean <= results.latency_max);
    assert!(results.latency_p50 <= results.latency_p99);
}
