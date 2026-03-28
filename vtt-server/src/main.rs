use axum::{routing::get, Json, Router};
use serde_json::json;
use vtt_core::{load_config, ServerConfig};

async fn health() -> Json<serde_json::Value> {
    Json(json!({ "status": "ok" }))
}

#[tokio::main]
async fn main() {
    let config = match load_config::<ServerConfig>("server.toml") {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Warning: {e}, using defaults");
            ServerConfig::default()
        }
    };

    if let Err(e) = config.validate() {
        eprintln!("Error: invalid configuration: {e}");
        std::process::exit(1);
    }

    let app = Router::new().route("/health", get(health));

    let addr = config.bind_address();
    println!("vtt-server listening on {addr}");

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
