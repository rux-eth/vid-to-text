use clap::{Parser, Subcommand};
use std::path::PathBuf;

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

        /// Server address (overrides config)
        #[arg(short, long)]
        server: Option<String>,

        /// Force reprocessing, ignoring checkpoints
        #[arg(long)]
        force: bool,
    },

    /// Check system dependencies and configuration
    Doctor,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Process { path, output, server, force } => {
            println!("Processing: {}", path.display());
            if let Some(out) = &output {
                println!("Output: {}", out.display());
            }
            if let Some(srv) = &server {
                println!("Server: {}", srv);
            }
            if force {
                println!("Force reprocessing enabled");
            }
            // TODO: implement in PR-008
        }
        Commands::Doctor => {
            println!("vid-to-text doctor");
            println!("Checking system dependencies...");
            // TODO: implement in PR-002/PR-008
        }
    }
}
