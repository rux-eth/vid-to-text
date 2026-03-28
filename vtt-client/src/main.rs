use clap::{Parser, Subcommand};
use std::path::PathBuf;
use vtt_core::{load_config, config_file_exists, config_file_path, ClientConfig};

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

fn load_client_config() -> ClientConfig {
    match load_config::<ClientConfig>("client.toml") {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Warning: {e}, using defaults");
            ClientConfig::default()
        }
    }
}

fn apply_cli_overrides(config: &mut ClientConfig, server: &Option<String>, output: &Option<PathBuf>) {
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

fn run_doctor(config: &ClientConfig) {
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
    println!("  server.host = {}", config.server.host);
    println!("  server.port = {}", config.server.port);
    match &config.output.dir {
        Some(dir) => println!("  output.dir  = {dir}"),
        None => println!("  output.dir  = (alongside input file)"),
    }
    println!("  server_url  = {}", config.server_url());

    // Validation
    println!();
    match config.validate() {
        Ok(()) => println!("[ok] Config is valid"),
        Err(e) => println!("[!!] Config validation error: {e}"),
    }
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let mut config = load_client_config();

    match &cli.command {
        Commands::Process { path, output, server, force } => {
            apply_cli_overrides(&mut config, server, output);

            if let Err(e) = config.validate() {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }

            println!("Processing: {}", path.display());
            println!("Server: {}", config.server_url());
            if let Some(dir) = &config.output.dir {
                println!("Output dir: {dir}");
            }
            if *force {
                println!("Force reprocessing enabled");
            }
            // TODO: implement in PR-008
        }
        Commands::Doctor => {
            run_doctor(&config);
        }
    }
}
