mod common;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use common::TestServer;
use httpress::{Benchmark, HookAction};

#[tokio::test]
async fn test_before_hook_continue() {
    let server = TestServer::start().await;
    let hook_called = Arc::new(AtomicUsize::new(0));
    let hook_called_clone = hook_called.clone();

    let results = Benchmark::builder()
        .url(&format!("{}/ok", server.base_url))
        .requests(10)
        .concurrency(2)
        .before_request(move |_ctx| {
            hook_called_clone.fetch_add(1, Ordering::SeqCst);
            HookAction::Continue
        })
        .build()
        .unwrap()
        .run()
        .await
        .unwrap();

    assert_eq!(results.total_requests, 10);
    assert_eq!(results.successful_requests, 10);
    assert_eq!(hook_called.load(Ordering::SeqCst), 10);
}

#[tokio::test]
async fn test_before_hook_abort() {
    let server = TestServer::start().await;
    let aborted = Arc::new(AtomicUsize::new(0));
    let aborted_clone = aborted.clone();

    let results = Benchmark::builder()
        .url(&format!("{}/ok", server.base_url))
        .requests(20)
        .concurrency(2)
        .before_request(move |ctx| {
            // Abort every 4th request (0, 4, 8, 12, 16)
            if ctx.request_number % 4 == 0 {
                aborted_clone.fetch_add(1, Ordering::SeqCst);
                HookAction::Abort
            } else {
                HookAction::Continue
            }
        })
        .build()
        .unwrap()
        .run()
        .await
        .unwrap();

    assert_eq!(results.total_requests, 20);
    // Aborted requests count as failed
    assert!(
        results.failed_requests >= 4,
        "Expected at least 4 aborted requests, got {}",
        results.failed_requests
    );
}

#[tokio::test]
async fn test_after_hook_continue() {
    let server = TestServer::start().await;
    let hook_called = Arc::new(AtomicUsize::new(0));
    let hook_called_clone = hook_called.clone();

    let results = Benchmark::builder()
        .url(&format!("{}/ok", server.base_url))
        .requests(10)
        .concurrency(2)
        .after_request(move |_ctx| {
            hook_called_clone.fetch_add(1, Ordering::SeqCst);
            HookAction::Continue
        })
        .build()
        .unwrap()
        .run()
        .await
        .unwrap();

    assert_eq!(results.total_requests, 10);
    assert_eq!(results.successful_requests, 10);
    assert_eq!(hook_called.load(Ordering::SeqCst), 10);
}

#[tokio::test]
async fn test_after_hook_retry_on_500() {
    let server = TestServer::start_flaky(5).await; // Fail first 5 requests

    let retry_count = Arc::new(AtomicUsize::new(0));
    let retry_count_clone = retry_count.clone();

    let results = Benchmark::builder()
        .url(&format!("{}/flaky", server.base_url))
        .requests(10)
        .concurrency(1) // Single worker for predictable behavior
        .max_retries(3)
        .after_request(move |ctx| {
            if ctx.status == Some(500) {
                retry_count_clone.fetch_add(1, Ordering::SeqCst);
                HookAction::Retry
            } else {
                HookAction::Continue
            }
        })
        .build()
        .unwrap()
        .run()
        .await
        .unwrap();

    // Should have retried some requests
    assert!(
        retry_count.load(Ordering::SeqCst) > 0,
        "Expected some retries"
    );
    // Most requests should eventually succeed
    assert!(
        results.successful_requests >= 5,
        "Expected at least 5 successful requests, got {}",
        results.successful_requests
    );
}

#[tokio::test]
async fn test_max_retries_limit() {
    let server = TestServer::start().await;
    let attempts = Arc::new(AtomicUsize::new(0));
    let attempts_clone = attempts.clone();

    let results = Benchmark::builder()
        .url(&format!("{}/status/500", server.base_url)) // Always 500
        .requests(1)
        .concurrency(1)
        .max_retries(3)
        .after_request(move |_ctx| {
            attempts_clone.fetch_add(1, Ordering::SeqCst);
            HookAction::Retry
        })
        .build()
        .unwrap()
        .run()
        .await
        .unwrap();

    // Original + 3 retries = 4 attempts max
    let total_attempts = attempts.load(Ordering::SeqCst);
    assert!(
        total_attempts <= 4,
        "Expected at most 4 attempts, got {}",
        total_attempts
    );
    assert_eq!(results.failed_requests, 1);
}

#[tokio::test]
async fn test_hook_context_has_worker_id() {
    let server = TestServer::start().await;
    let worker_ids = Arc::new(Mutex::new(Vec::new()));
    let worker_ids_clone = worker_ids.clone();

    let results = Benchmark::builder()
        .url(&format!("{}/ok", server.base_url))
        .requests(20)
        .concurrency(4)
        .after_request(move |ctx| {
            worker_ids_clone.lock().unwrap().push(ctx.worker_id);
            HookAction::Continue
        })
        .build()
        .unwrap()
        .run()
        .await
        .unwrap();

    assert_eq!(results.total_requests, 20);

    let ids = worker_ids.lock().unwrap();
    assert_eq!(ids.len(), 20);

    // Worker IDs should be in range [0, concurrency)
    for &id in ids.iter() {
        assert!(id < 4, "Worker ID {} out of range", id);
    }

    // With 4 workers and 20 requests, we should see multiple worker IDs
    let unique_workers: std::collections::HashSet<_> = ids.iter().collect();
    assert!(
        unique_workers.len() > 1,
        "Expected multiple workers, only saw {:?}",
        unique_workers
    );
}

#[tokio::test]
async fn test_hook_context_has_latency() {
    let server = TestServer::start().await;
    let latencies = Arc::new(Mutex::new(Vec::new()));
    let latencies_clone = latencies.clone();

    let results = Benchmark::builder()
        .url(&format!("{}/delay/20", server.base_url)) // 20ms delay
        .requests(5)
        .concurrency(1)
        .after_request(move |ctx| {
            latencies_clone.lock().unwrap().push(ctx.latency);
            HookAction::Continue
        })
        .build()
        .unwrap()
        .run()
        .await
        .unwrap();

    assert_eq!(results.total_requests, 5);

    let lats = latencies.lock().unwrap();
    assert_eq!(lats.len(), 5);

    // All latencies should be at least 20ms (the delay)
    for &lat in lats.iter() {
        assert!(
            lat >= Duration::from_millis(15),
            "Latency {:?} too low",
            lat
        );
    }
}

#[tokio::test]
async fn test_hook_context_has_status() {
    let server = TestServer::start().await;
    let statuses = Arc::new(Mutex::new(Vec::new()));
    let statuses_clone = statuses.clone();

    let results = Benchmark::builder()
        .url(&format!("{}/ok", server.base_url))
        .requests(5)
        .concurrency(1)
        .after_request(move |ctx| {
            statuses_clone.lock().unwrap().push(ctx.status);
            HookAction::Continue
        })
        .build()
        .unwrap()
        .run()
        .await
        .unwrap();

    assert_eq!(results.total_requests, 5);

    let stats = statuses.lock().unwrap();
    assert_eq!(stats.len(), 5);

    // All should have status 200
    for &status in stats.iter() {
        assert_eq!(status, Some(200), "Expected status 200, got {:?}", status);
    }
}

#[tokio::test]
async fn test_multiple_before_hooks() {
    let server = TestServer::start().await;
    let hook1_count = Arc::new(AtomicUsize::new(0));
    let hook2_count = Arc::new(AtomicUsize::new(0));
    let hook1_clone = hook1_count.clone();
    let hook2_clone = hook2_count.clone();

    let results = Benchmark::builder()
        .url(&format!("{}/ok", server.base_url))
        .requests(10)
        .concurrency(2)
        .before_request(move |_ctx| {
            hook1_clone.fetch_add(1, Ordering::SeqCst);
            HookAction::Continue
        })
        .before_request(move |_ctx| {
            hook2_clone.fetch_add(1, Ordering::SeqCst);
            HookAction::Continue
        })
        .build()
        .unwrap()
        .run()
        .await
        .unwrap();

    assert_eq!(results.total_requests, 10);
    assert_eq!(hook1_count.load(Ordering::SeqCst), 10);
    assert_eq!(hook2_count.load(Ordering::SeqCst), 10);
}
