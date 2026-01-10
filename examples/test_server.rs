use std::time::Duration;

use axum::{
    Router,
    extract::Path,
    http::StatusCode,
    routing::get,
    Json,
};
use serde::{Deserialize, Serialize};
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

async fn get_user(Path(user_id): Path<u64>) -> Json<UserResponse> {
    Json(UserResponse {
        user_id,
        message: "User retrieved".to_string(),
    })
}

#[derive(Deserialize)]
struct CreateUserRequest {
    user_id: u64,
    worker: usize,
}

#[derive(Serialize)]
struct UserResponse {
    user_id: u64,
    message: String,
}

async fn create_user(Json(payload): Json<CreateUserRequest>) -> (StatusCode, Json<UserResponse>) {
    (
        StatusCode::CREATED,
        Json(UserResponse {
            user_id: payload.user_id,
            message: format!("User created by worker {}", payload.worker),
        }),
    )
}

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/", get(root))
        .route("/delay/{ms}", get(delay))
        .route("/status/{code}", get(status))
        .route("/user/{user_id}", get(get_user).post(create_user));

    let listener = TcpListener::bind("127.0.0.1:3000").await.unwrap();
    println!("Test server running on http://127.0.0.1:3000");
    println!("Endpoints:");
    println!("  GET  /              - Returns 'OK'");
    println!("  GET  /delay/:ms     - Returns 'OK' after :ms milliseconds");
    println!("  GET  /status/:code  - Returns the specified HTTP status code");
    println!("  GET  /user/:id      - Returns user info as JSON");
    println!("  POST /user/:id      - Creates user with JSON body");

    axum::serve(listener, app).await.unwrap();
}
