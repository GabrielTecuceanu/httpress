mod common;

use common::TestServer;
use httpress::{Benchmark, HookAction};
use serde_json::Value;

#[tokio::test]
async fn test_bytes_nonzero_for_successful_requests() {
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

    assert_eq!(results.successful_requests, 100);
    // "/ok" returns "OK" = 2 bytes
    assert!(
        results.total_bytes >= 100 * 2,
        "expected at least {} bytes, got {}",
        100 * 2,
        results.total_bytes
    );
}

#[tokio::test]
async fn test_bytes_zero_for_failed_requests() {
    // Use a port that is not listening
    let results = Benchmark::builder()
        .url("http://127.0.0.1:1")
        .requests(5)
        .concurrency(1)
        .build()
        .unwrap()
        .run()
        .await
        .unwrap();

    assert_eq!(results.successful_requests, 0);
    assert_eq!(results.total_bytes, 0);
}

#[tokio::test]
async fn test_bytes_scale_with_request_count() {
    let server = TestServer::start().await;

    let results = Benchmark::builder()
        .url(&format!("{}/body/1000", server.base_url))
        .requests(10)
        .concurrency(1)
        .build()
        .unwrap()
        .run()
        .await
        .unwrap();

    assert_eq!(results.successful_requests, 10);
    assert_eq!(
        results.total_bytes,
        10 * 1000,
        "expected exactly {} bytes, got {}",
        10 * 1000,
        results.total_bytes
    );
}

#[tokio::test]
async fn test_bytes_zero_for_aborted_hooks() {
    let server = TestServer::start().await;

    let results = Benchmark::builder()
        .url(&format!("{}/ok", server.base_url))
        .requests(10)
        .concurrency(2)
        .before_request(|_ctx| HookAction::Abort)
        .build()
        .unwrap()
        .run()
        .await
        .unwrap();

    assert_eq!(results.successful_requests, 0);
    assert_eq!(results.total_bytes, 0);
}

#[tokio::test]
async fn test_json_serialization_has_readable_durations() {
    let server = TestServer::start().await;

    let results = Benchmark::builder()
        .url(&format!("{}/ok", server.base_url))
        .requests(10)
        .concurrency(1)
        .build()
        .unwrap()
        .run()
        .await
        .unwrap();

    let json: Value = serde_json::to_value(&results).unwrap();

    let duration_fields = ["duration", "latency_min", "latency_max", "latency_mean"];

    for field in duration_fields {
        let val = &json[field];
        assert!(
            val.is_string(),
            "expected '{}' to be a string, got: {}",
            field,
            val
        );
    }

    // Check dynamic field (latency_percentiles)
    let percentiles = json["latency_percentiles"]
        .as_object()
        .expect("latency_percentiles must be an object");
    assert!(
        !percentiles.is_empty(),
        "latency_percentiles should not be empty"
    );
    for (_, value) in percentiles {
        // assert!(
        //     ke
        //     "percentile key should be a number, got: {}",
        //     key
        // );
        assert!(
            value.is_string(),
            "latency value should be a human-readable string, got: {}",
            value
        );
    }
}
