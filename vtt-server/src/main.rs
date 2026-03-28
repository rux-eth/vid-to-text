use axum::{routing::get, Json, Router};
use serde_json::json;

async fn health() -> Json<serde_json::Value> {
    Json(json!({ "status": "ok" }))
}

#[tokio::main]
async fn main() {
    let app = Router::new().route("/health", get(health));

    let addr = "0.0.0.0:3000";
    println!("vtt-server listening on {addr}");

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
