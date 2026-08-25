mod api;
mod cache;
mod doctor;
mod format;
mod process;

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use vtt_core::{load_config, ClientConfig};

#[derive(Parser)]
#[command(name = "vid-to-text", about = "Convert videos to structured text descriptions")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Process an mp4 file, directory of mp4 files, or a URL
    Process {
        /// Path to an mp4 file or directory containing mp4 files
        #[arg(required_unless_present = "url")]
        path: Option<PathBuf>,

        /// YouTube or video URL to download and process
        #[arg(short, long)]
        url: Option<String>,

        /// Output path (default: alongside input file, or current dir for URLs)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Server address as host:port (overrides config)
        #[arg(short, long)]
        server: Option<String>,

        /// Force reprocessing, ignoring checkpoints
        #[arg(long)]
        force: bool,

        /// Max video resolution for URL downloads (e.g. "720", "1080")
        #[arg(long)]
        max_resolution: Option<String>,

        /// Max video FPS for URL downloads
        #[arg(long)]
        max_fps: Option<u32>,

        /// Server-side processing profile (e.g. "charts", "lectures")
        #[arg(long)]
        profile: Option<String>,
    },

    /// Check system dependencies and configuration
    Doctor,

    /// Cancel queued jobs on the server
    Cancel {
        /// Job ID to cancel, or "all" to clear the queue
        target: String,
    },

    /// List all cached processed videos
    List,

    /// Format a Timeline JSON file into human-readable Markdown via OpenAI
    Format {
        /// Path to Timeline JSON file or cache hash (8 hex chars)
        input: String,

        /// Output path (default: input_formatted.md)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// OpenAI model override (default from config)
        #[arg(long)]
        model: Option<String>,

        /// Custom system prompt file (overrides config and default)
        #[arg(short, long)]
        prompt: Option<PathBuf>,

        /// OpenAI stored prompt ID (uses Responses API instead of Chat Completions)
        #[arg(long)]
        prompt_id: Option<String>,
    },
}

fn load_client_config() -> ClientConfig {
    match load_config::<ClientConfig>("client.toml") {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Warning: {e}, using defaults");
            ClientConfig::default()
        }
    }
}

fn apply_cli_overrides(
    config: &mut ClientConfig,
    server: &Option<String>,
    output: &Option<PathBuf>,
) {
    if let Some(srv) = server {
        if let Some((host, port_str)) = srv.rsplit_once(':') {
            if let Ok(port) = port_str.parse::<u16>() {
                config.server.host = host.to_string();
                config.server.port = port;
            } else {
                config.server.host = srv.clone();
            }
        } else {
            config.server.host = srv.clone();
        }
    }
    if let Some(out) = output {
        config.output.dir = Some(out.display().to_string());
    }
}

#[tokio::main]
async fn main() {
    // Load .env file if present (for OPENAI_API_KEY etc.)
    let _ = dotenvy::dotenv();

    let cli = Cli::parse();
    let mut config = load_client_config();

    match &cli.command {
        Commands::Process {
            path,
            url,
            output,
            server,
            force,
            max_resolution,
            max_fps,
            profile,
        } => {
            let force = *force;
            apply_cli_overrides(&mut config, server, output);

            if let Err(e) = config.validate() {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }

            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(
                    config.polling.timeout_secs + 60,
                ))
                .build()
                .unwrap_or_else(|e| {
                    eprintln!("Error: failed to create HTTP client: {e}");
                    std::process::exit(1);
                });

            let result = if let Some(url) = url {
                process::process_url(
                    &client,
                    &config,
                    url,
                    force,
                    max_resolution.clone(),
                    *max_fps,
                    profile.clone(),
                )
                .await
            } else if let Some(path) = path {
                if path.is_file() {
                    if path.extension().and_then(|e| e.to_str()) != Some("mp4") {
                        eprintln!("Error: only mp4 files are supported");
                        std::process::exit(1);
                    }
                    process::process_single_file(&client, &config, path, force).await
                } else if path.is_dir() {
                    process::process_directory(&client, &config, path, force).await
                } else {
                    Err(format!("{} not found", path.display()))
                }
            } else {
                Err("either a path or --url must be provided".to_string())
            };

            if let Err(e) = result {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
        Commands::Cancel { target } => {
            let server_url = config.server_url();
            let client = reqwest::Client::new();

            let result = if target == "all" {
                let resp = client
                    .delete(format!("{server_url}/jobs"))
                    .send()
                    .await;
                match resp {
                    Ok(r) => {
                        let body: serde_json::Value = r.json().await.unwrap_or_default();
                        let count = body.get("cancelled").and_then(|v| v.as_u64()).unwrap_or(0);
                        eprintln!("Cancelled {count} queued job(s).");
                        Ok(())
                    }
                    Err(e) => Err(format!("failed to cancel jobs: {e}")),
                }
            } else {
                let resp = client
                    .delete(format!("{server_url}/jobs/{target}"))
                    .send()
                    .await;
                match resp {
                    Ok(r) if r.status().is_success() => {
                        eprintln!("Cancelled job {target}.");
                        Ok(())
                    }
                    Ok(r) => {
                        let body = r.text().await.unwrap_or_default();
                        Err(format!("{body}"))
                    }
                    Err(e) => Err(format!("failed to cancel job: {e}")),
                }
            };

            if let Err(e) = result {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
        Commands::Doctor => {
            doctor::run_doctor(&config).await;
        }
        Commands::List => {
            match cache::cache_list() {
                Ok(entries) => {
                    if entries.is_empty() {
                        eprintln!("No cached videos.");
                    } else {
                        println!("{:<10} {:<50} {:>6} {:>10} {}", "HASH", "TITLE", "DUR", "SEGMENTS", "DATE");
                        println!("{}", "-".repeat(90));
                        for e in &entries {
                            let dur_min = e.duration_seconds / 60.0;
                            let title = if e.title.len() > 48 {
                                // byte-based cap: back off to a char boundary
                                let mut end = 45;
                                while !e.title.is_char_boundary(end) {
                                    end -= 1;
                                }
                                format!("{}...", &e.title[..end])
                            } else {
                                e.title.clone()
                            };
                            let date = &e.processed_at[..10];
                            println!(
                                "{:<10} {:<50} {:>5.1}m {:>10} {}",
                                e.hash, title, dur_min, format!("{} segs", e.segment_count), date
                            );
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            }
        }
        Commands::Format {
            input,
            output,
            model,
            prompt,
            prompt_id,
        } => {
            let resolved_path = match cache::resolve_input(input) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            };
            if let Err(e) = format::run_format(
                &config.openai,
                &resolved_path,
                output.as_deref(),
                model.as_deref(),
                prompt.as_deref(),
                prompt_id.as_deref(),
            )
            .await
            {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
    }
}
