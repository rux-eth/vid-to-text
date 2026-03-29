use std::path::{Path, PathBuf};
use std::time::Instant;

use clap::{Parser, Subcommand};
use serde::Deserialize;
use tokio_util::io::ReaderStream;
use vtt_core::{
    config_file_exists, config_file_path, load_config, ClientConfig, Timeline,
};

#[derive(Parser)]
#[command(name = "vid-to-text", about = "Convert videos to structured text descriptions")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Process an mp4 file or directory of mp4 files
    Process {
        /// Path to an mp4 file or directory containing mp4 files
        path: PathBuf,

        /// Output path (default: alongside input file)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Server address as host:port (overrides config)
        #[arg(short, long)]
        server: Option<String>,

        /// Force reprocessing, ignoring checkpoints
        #[arg(long)]
        force: bool,
    },

    /// Check system dependencies and configuration
    Doctor,
}

// --- Server response types ---

#[derive(Debug, Deserialize)]
struct JobResponse {
    id: String,
    #[allow(dead_code)]
    source: String,
    status: String,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ErrorResponse {
    error: String,
}

#[derive(Debug, Deserialize)]
struct HealthResponse {
    status: String,
    ffmpeg: serde_json::Value,
    ollama: serde_json::Value,
}

// --- Config helpers ---

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

// --- Output path computation ---

fn compute_output_path(input_path: &Path, output_dir: &Option<String>) -> PathBuf {
    let stem = input_path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy();
    let json_name = format!("{stem}.json");

    match output_dir {
        Some(dir) => PathBuf::from(dir).join(&json_name),
        None => input_path.with_file_name(&json_name),
    }
}

// --- Process a single file ---

async fn process_single_file(
    client: &reqwest::Client,
    config: &ClientConfig,
    input_path: &Path,
) -> Result<(), String> {
    let server_url = config.server_url();
    let filename = input_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    // Upload
    eprintln!("Uploading {filename}...");
    let file = tokio::fs::File::open(input_path)
        .await
        .map_err(|e| format!("failed to open {}: {e}", input_path.display()))?;

    let stream = ReaderStream::new(file);
    let body = reqwest::Body::wrap_stream(stream);
    let part = reqwest::multipart::Part::stream(body)
        .file_name(filename.clone())
        .mime_str("video/mp4")
        .map_err(|e| format!("failed to set mime type: {e}"))?;
    let form = reqwest::multipart::Form::new().part("file", part);

    let resp = client
        .post(format!("{server_url}/jobs/upload"))
        .multipart(form)
        .send()
        .await
        .map_err(|e| format!("upload failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        if let Ok(err) = serde_json::from_str::<ErrorResponse>(&body) {
            return Err(format!("server error ({status}): {}", err.error));
        }
        return Err(format!("server error ({status}): {body}"));
    }

    let job: JobResponse = resp
        .json()
        .await
        .map_err(|e| format!("failed to parse job response: {e}"))?;

    eprintln!("Job {} created for {filename}", job.id);

    // Poll
    let start = Instant::now();
    let mut last_status = String::new();

    loop {
        tokio::time::sleep(std::time::Duration::from_secs(
            config.polling.poll_interval_secs,
        ))
        .await;

        let elapsed = start.elapsed().as_secs();
        if elapsed > config.polling.timeout_secs {
            return Err(format!(
                "timed out after {}s waiting for job {}",
                config.polling.timeout_secs, job.id
            ));
        }

        let resp = client
            .get(format!("{server_url}/jobs/{}", job.id))
            .send()
            .await
            .map_err(|e| format!("failed to poll job status: {e}"))?;

        if !resp.status().is_success() {
            return Err(format!("failed to get job status: {}", resp.status()));
        }

        let status: JobResponse = resp
            .json()
            .await
            .map_err(|e| format!("failed to parse status response: {e}"))?;

        if status.status != last_status {
            eprintln!("[{}] {filename} ({}s)", status.status, elapsed);
            last_status = status.status.clone();
        }

        match status.status.as_str() {
            "completed" => break,
            "failed" => {
                let msg = status.error.unwrap_or_else(|| "unknown error".into());
                return Err(format!("job failed: {msg}"));
            }
            _ => continue,
        }
    }

    // Download result
    let resp = client
        .get(format!("{server_url}/jobs/{}/result", job.id))
        .send()
        .await
        .map_err(|e| format!("failed to download result: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!(
            "failed to get result: {}",
            resp.status()
        ));
    }

    let timeline: Timeline = resp
        .json()
        .await
        .map_err(|e| format!("failed to parse timeline: {e}"))?;

    // Write output
    let output_path = compute_output_path(input_path, &config.output.dir);
    if let Some(parent) = output_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("failed to create output dir: {e}"))?;
    }

    let json = serde_json::to_string_pretty(&timeline)
        .map_err(|e| format!("failed to serialize timeline: {e}"))?;
    tokio::fs::write(&output_path, &json)
        .await
        .map_err(|e| format!("failed to write {}: {e}", output_path.display()))?;

    eprintln!(
        "Wrote {} ({} segments, {:.1}s duration)",
        output_path.display(),
        timeline.segments.len(),
        timeline.duration_seconds
    );

    Ok(())
}

// --- Find mp4 files in a directory ---

fn find_mp4_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let entries = std::fs::read_dir(dir)
        .map_err(|e| format!("failed to read directory {}: {e}", dir.display()))?;

    let mut files: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().and_then(|e| e.to_str()) == Some("mp4"))
        .collect();

    files.sort();
    Ok(files)
}

// --- Doctor ---

async fn run_doctor(config: &ClientConfig) {
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
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build();

    let client = match client {
        Ok(c) => c,
        Err(e) => {
            println!("[!!] Failed to create HTTP client: {e}");
            return;
        }
    };

    let server_url = config.server_url();
    match client.get(format!("{server_url}/health")).send().await {
        Ok(resp) => {
            if resp.status().is_success() {
                match resp.json::<HealthResponse>().await {
                    Ok(health) => {
                        println!("[ok] Server reachable at {server_url}");

                        // ffmpeg
                        if let Some(err) = health.ffmpeg.get("error") {
                            println!("[!!] ffmpeg: {}", err.as_str().unwrap_or("error"));
                        } else {
                            let version = health.ffmpeg.get("ffmpeg_version")
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

                        // overall
                        if health.status == "ok" {
                            println!("\nAll systems operational.");
                        } else {
                            println!("\nServer is degraded — some dependencies unavailable.");
                        }
                    }
                    Err(e) => {
                        println!("[ok] Server reachable at {server_url}");
                        println!("[!!] Failed to parse health response: {e}");
                    }
                }
            } else {
                println!("[!!] Server returned {}", resp.status());
            }
        }
        Err(e) => {
            println!("[!!] Server not reachable at {server_url}: {e}");
        }
    }
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let mut config = load_client_config();

    match &cli.command {
        Commands::Process {
            path,
            output,
            server,
            force: _,
        } => {
            apply_cli_overrides(&mut config, server, output);

            if let Err(e) = config.validate() {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }

            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(config.polling.timeout_secs + 60))
                .build()
                .unwrap_or_else(|e| {
                    eprintln!("Error: failed to create HTTP client: {e}");
                    std::process::exit(1);
                });

            if path.is_file() {
                if path.extension().and_then(|e| e.to_str()) != Some("mp4") {
                    eprintln!("Error: only mp4 files are supported");
                    std::process::exit(1);
                }
                if let Err(e) = process_single_file(&client, &config, path).await {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            } else if path.is_dir() {
                let files = match find_mp4_files(path) {
                    Ok(f) => f,
                    Err(e) => {
                        eprintln!("Error: {e}");
                        std::process::exit(1);
                    }
                };

                if files.is_empty() {
                    eprintln!("No mp4 files found in {}", path.display());
                    std::process::exit(1);
                }

                eprintln!("Found {} mp4 file(s) in {}", files.len(), path.display());

                let mut success = 0;
                let total = files.len();

                for file in &files {
                    match process_single_file(&client, &config, file).await {
                        Ok(()) => success += 1,
                        Err(e) => eprintln!("Error processing {}: {e}", file.display()),
                    }
                }

                eprintln!("\nProcessed {success}/{total} files successfully.");
                if success < total {
                    std::process::exit(1);
                }
            } else {
                eprintln!("Error: {} not found", path.display());
                std::process::exit(1);
            }
        }
        Commands::Doctor => {
            run_doctor(&config).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_output_path_alongside_input() {
        let path = compute_output_path(Path::new("/videos/test.mp4"), &None);
        assert_eq!(path, PathBuf::from("/videos/test.json"));
    }

    #[test]
    fn test_compute_output_path_with_output_dir() {
        let path =
            compute_output_path(Path::new("/videos/test.mp4"), &Some("/output".to_string()));
        assert_eq!(path, PathBuf::from("/output/test.json"));
    }

    #[test]
    fn test_find_mp4_files_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let files = find_mp4_files(dir.path()).unwrap();
        assert!(files.is_empty());
    }

    #[test]
    fn test_find_mp4_files_mixed() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.mp4"), "").unwrap();
        std::fs::write(dir.path().join("b.txt"), "").unwrap();
        std::fs::write(dir.path().join("c.mp4"), "").unwrap();
        let files = find_mp4_files(dir.path()).unwrap();
        assert_eq!(files.len(), 2);
        assert!(files[0].file_name().unwrap().to_str().unwrap() == "a.mp4");
        assert!(files[1].file_name().unwrap().to_str().unwrap() == "c.mp4");
    }

    #[test]
    fn test_find_mp4_files_sorted() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("z.mp4"), "").unwrap();
        std::fs::write(dir.path().join("a.mp4"), "").unwrap();
        std::fs::write(dir.path().join("m.mp4"), "").unwrap();
        let files = find_mp4_files(dir.path()).unwrap();
        assert_eq!(files.len(), 3);
        assert!(files[0] < files[1]);
        assert!(files[1] < files[2]);
    }
}
