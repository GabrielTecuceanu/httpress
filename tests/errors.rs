mod common;

use std::time::Duration;

use common::TestServer;
use httpress::Benchmark;

#[tokio::test]
async fn test_connection_refused() {
    // No server running on this port
    let results = Benchmark::builder()
        .url("http://127.0.0.1:59999/ok")
        .requests(5)
        .concurrency(1)
        .build()
        .unwrap()
        .run()
        .await
        .unwrap();

    assert_eq!(results.total_requests, 5);
    assert_eq!(results.failed_requests, 5);
    assert_eq!(results.successful_requests, 0);
}

#[tokio::test]
async fn test_request_timeout() {
    let server = TestServer::start().await;

    let results = Benchmark::builder()
        .url(&format!("{}/delay/5000", server.base_url)) // 5 second delay
        .requests(3)
        .concurrency(1)
        .timeout(Duration::from_millis(100)) // 100ms timeout
        .build()
        .unwrap()
        .run()
        .await
        .unwrap();

    assert_eq!(results.total_requests, 3);
    assert_eq!(results.failed_requests, 3);
    assert_eq!(results.successful_requests, 0);

    // Latency should be around timeout, not 5 seconds
    assert!(
        results.latency_max < Duration::from_millis(500),
        "Latency max {:?} should be less than 500ms",
        results.latency_max
    );
}

#[tokio::test]
async fn test_http_500_errors_recorded() {
    let server = TestServer::start().await;

    let results = Benchmark::builder()
        .url(&format!("{}/status/500", server.base_url))
        .requests(10)
        .concurrency(2)
        .build()
        .unwrap()
        .run()
        .await
        .unwrap();

    assert_eq!(results.total_requests, 10);
    assert_eq!(results.failed_requests, 10);
    assert_eq!(results.successful_requests, 0);

    // Status code should be recorded
    assert_eq!(
        *results.status_codes.get(&500).unwrap_or(&0),
        10,
        "Expected 10 requests with status 500"
    );
}

#[tokio::test]
async fn test_http_404_errors_recorded() {
    let server = TestServer::start().await;

    let results = Benchmark::builder()
        .url(&format!("{}/status/404", server.base_url))
        .requests(5)
        .concurrency(1)
        .build()
        .unwrap()
        .run()
        .await
        .unwrap();

    assert_eq!(results.total_requests, 5);
    assert_eq!(results.failed_requests, 5);
    assert_eq!(results.successful_requests, 0);

    assert_eq!(
        *results.status_codes.get(&404).unwrap_or(&0),
        5,
        "Expected 5 requests with status 404"
    );
}

#[tokio::test]
async fn test_mixed_status_codes() {
    let server = TestServer::start_rotating(vec![200, 400, 500]).await;

    let results = Benchmark::builder()
        .url(&format!("{}/rotating", server.base_url))
        .requests(30)
        .concurrency(1) // Single worker for predictable rotation
        .build()
        .unwrap()
        .run()
        .await
        .unwrap();

    assert_eq!(results.total_requests, 30);

    // Should have approximately 10 of each status code
    let count_200 = *results.status_codes.get(&200).unwrap_or(&0);
    let count_400 = *results.status_codes.get(&400).unwrap_or(&0);
    let count_500 = *results.status_codes.get(&500).unwrap_or(&0);

    assert_eq!(count_200, 10, "Expected 10 requests with status 200");
    assert_eq!(count_400, 10, "Expected 10 requests with status 400");
    assert_eq!(count_500, 10, "Expected 10 requests with status 500");

    // Only 200s should be successful
    assert_eq!(results.successful_requests, 10);
    assert_eq!(results.failed_requests, 20);
}

#[tokio::test]
async fn test_latency_recorded_for_errors() {
    let server = TestServer::start().await;

    let results = Benchmark::builder()
        .url(&format!("{}/status/500", server.base_url))
        .requests(5)
        .concurrency(1)
        .build()
        .unwrap()
        .run()
        .await
        .unwrap();

    assert_eq!(results.total_requests, 5);
    assert_eq!(results.failed_requests, 5);

    // Latency should still be recorded even for errors
    assert!(
        results.latency_min > Duration::ZERO,
        "Latency min should be greater than zero"
    );
    assert!(
        results.latency_max > Duration::ZERO,
        "Latency max should be greater than zero"
    );
}

#[tokio::test]
async fn test_throughput_calculated() {
    let server = TestServer::start().await;

    let results = Benchmark::builder()
        .url(&format!("{}/ok", server.base_url))
        .requests(50)
        .concurrency(4)
        .build()
        .unwrap()
        .run()
        .await
        .unwrap();

    assert_eq!(results.total_requests, 50);

    // Throughput should be positive
    assert!(
        results.throughput > 0.0,
        "Throughput should be positive, got {}",
        results.throughput
    );

    // Throughput should match total_requests / duration
    let calculated_throughput = results.total_requests as f64 / results.duration.as_secs_f64();
    let diff = (results.throughput - calculated_throughput).abs();
    assert!(
        diff < 1.0,
        "Throughput mismatch: reported {} vs calculated {}",
        results.throughput,
        calculated_throughput
    );
}
