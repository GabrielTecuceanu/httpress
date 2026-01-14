#![allow(dead_code)]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::get,
    Router,
};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

/// A test HTTP server for integration tests.
pub struct TestServer {
    pub base_url: String,
    handle: JoinHandle<()>,
}

impl TestServer {
    /// Start a new test server on a random available port.
    pub async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let base_url = format!("http://127.0.0.1:{}", port);

        let app = Router::new()
            .route("/ok", get(ok_handler))
            .route("/delay/{ms}", get(delay_handler))
            .route("/status/{code}", get(status_handler));

        let handle = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        // Give the server a moment to start
        tokio::time::sleep(Duration::from_millis(10)).await;

        Self { base_url, handle }
    }

    /// Start a test server with a flaky endpoint that fails N times before succeeding.
    pub async fn start_flaky(fail_count: usize) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let base_url = format!("http://127.0.0.1:{}", port);

        let counter = Arc::new(AtomicUsize::new(0));

        let app = Router::new()
            .route("/ok", get(ok_handler))
            .route("/delay/{ms}", get(delay_handler))
            .route("/status/{code}", get(status_handler))
            .route("/flaky", get(flaky_handler))
            .with_state(FlakyState {
                counter,
                fail_count,
            });

        let handle = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        tokio::time::sleep(Duration::from_millis(10)).await;

        Self { base_url, handle }
    }

    /// Start a test server with rotating status codes.
    pub async fn start_rotating(codes: Vec<u16>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let base_url = format!("http://127.0.0.1:{}", port);

        let counter = Arc::new(AtomicUsize::new(0));

        let app = Router::new()
            .route("/ok", get(ok_handler))
            .route("/delay/{ms}", get(delay_handler))
            .route("/status/{code}", get(status_handler))
            .route("/rotating", get(rotating_handler))
            .with_state(RotatingState { counter, codes });

        let handle = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        tokio::time::sleep(Duration::from_millis(10)).await;

        Self { base_url, handle }
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

// Handlers

async fn ok_handler() -> &'static str {
    "OK"
}

async fn delay_handler(Path(ms): Path<u64>) -> &'static str {
    tokio::time::sleep(Duration::from_millis(ms)).await;
    "OK"
}

async fn status_handler(Path(code): Path<u16>) -> StatusCode {
    StatusCode::from_u16(code).unwrap_or(StatusCode::BAD_REQUEST)
}

// Flaky server state
#[derive(Clone)]
struct FlakyState {
    counter: Arc<AtomicUsize>,
    fail_count: usize,
}

async fn flaky_handler(State(state): State<FlakyState>) -> StatusCode {
    let count = state.counter.fetch_add(1, Ordering::SeqCst);
    if count < state.fail_count {
        StatusCode::INTERNAL_SERVER_ERROR
    } else {
        StatusCode::OK
    }
}

// Rotating status code state
#[derive(Clone)]
struct RotatingState {
    counter: Arc<AtomicUsize>,
    codes: Vec<u16>,
}

async fn rotating_handler(State(state): State<RotatingState>) -> StatusCode {
    let count = state.counter.fetch_add(1, Ordering::SeqCst);
    let code = state.codes[count % state.codes.len()];
    StatusCode::from_u16(code).unwrap_or(StatusCode::BAD_REQUEST)
}
