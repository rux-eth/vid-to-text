mod api;
mod doctor;
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
    },

    /// Check system dependencies and configuration
    Doctor,
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
        Commands::Doctor => {
            doctor::run_doctor(&config).await;
        }
    }
}
