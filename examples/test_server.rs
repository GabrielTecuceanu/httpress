use std::time::Duration;

use axum::{
    Router,
    extract::Path,
    http::StatusCode,
    routing::get,
};
use tokio::net::TcpListener;

async fn root() -> &'static str {
    "OK"
}

async fn delay(Path(ms): Path<u64>) -> &'static str {
    tokio::time::sleep(Duration::from_millis(ms)).await;
    "OK"
}

async fn status(Path(code): Path<u16>) -> StatusCode {
    StatusCode::from_u16(code).unwrap_or(StatusCode::BAD_REQUEST)
}

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/", get(root))
        .route("/delay/{ms}", get(delay))
        .route("/status/{code}", get(status));

    let listener = TcpListener::bind("127.0.0.1:3000").await.unwrap();
    println!("Test server running on http://127.0.0.1:3000");
    println!("Endpoints:");
    println!("  GET /           - Returns 'OK'");
    println!("  GET /delay/:ms  - Returns 'OK' after :ms milliseconds");
    println!("  GET /status/:code - Returns the specified HTTP status code");

    axum::serve(listener, app).await.unwrap();
}
