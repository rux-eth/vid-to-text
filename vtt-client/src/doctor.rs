use vtt_core::{config_file_exists, config_file_path, ClientConfig};

use crate::api;

pub async fn run_doctor(config: &ClientConfig) {
    println!("vid-to-text doctor");
    println!("==================");

    // Config file status
    let config_path = config_file_path("client.toml")
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    if config_file_exists("client.toml") {
        println!("[ok] Config file found: {config_path}");
    } else {
        println!("[--] No config file at: {config_path} (using defaults)");
    }

    // Resolved config
    println!();
    println!("Resolved configuration:");
    println!("  server.host           = {}", config.server.host);
    println!("  server.port           = {}", config.server.port);
    match &config.output.dir {
        Some(dir) => println!("  output.dir            = {dir}"),
        None => println!("  output.dir            = (alongside input file)"),
    }
    println!(
        "  polling.interval      = {}s",
        config.polling.poll_interval_secs
    );
    println!(
        "  polling.timeout       = {}s",
        config.polling.timeout_secs
    );
    println!("  server_url            = {}", config.server_url());

    // Validation
    println!();
    match config.validate() {
        Ok(()) => println!("[ok] Config is valid"),
        Err(e) => println!("[!!] Config validation error: {e}"),
    }

    // Server connectivity
    println!();
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            println!("[!!] Failed to create HTTP client: {e}");
            return;
        }
    };

    let server_url = config.server_url();
    match api::fetch_health(&client, &server_url).await {
        Ok(health) => {
            println!("[ok] Server reachable at {server_url}");

            // ffmpeg
            if let Some(err) = health.ffmpeg.get("error") {
                println!("[!!] ffmpeg: {}", err.as_str().unwrap_or("error"));
            } else {
                let version = health
                    .ffmpeg
                    .get("ffmpeg_version")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                println!("[ok] ffmpeg: {version}");
            }

            // ollama
            if health.ollama == serde_json::json!("ok") {
                println!("[ok] ollama: ok");
            } else if let Some(err) = health.ollama.get("error") {
                println!("[!!] ollama: {}", err.as_str().unwrap_or("error"));
            } else {
                println!("[!!] ollama: {}", health.ollama);
            }

            // yt-dlp
            if let Some(v) = health.ytdlp.get("version") {
                println!("[ok] yt-dlp: {}", v.as_str().unwrap_or("unknown"));
            } else if let Some(err) = health.ytdlp.get("error") {
                println!("[!!] yt-dlp: {}", err.as_str().unwrap_or("error"));
            }

            // OCR (fidelity diagnostic, PR-023)
            if health.ocr == serde_json::json!("disabled") {
                println!("[--] ocr: fidelity diagnostic disabled on server");
            } else if let Some(v) = health.ocr.get("version") {
                println!("[ok] ocr: {}", v.as_str().unwrap_or("unknown"));
            } else if let Some(err) = health.ocr.get("error") {
                println!("[!!] ocr: {}", err.as_str().unwrap_or("error"));
            }

            // overall
            if health.status == "ok" {
                println!("\nAll systems operational.");
            } else {
                println!("\nServer is degraded — some dependencies unavailable.");
            }
        }
        Err(e) => {
            println!("[!!] {e}");
        }
    }
}
