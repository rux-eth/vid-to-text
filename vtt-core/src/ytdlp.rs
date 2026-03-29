use std::path::{Path, PathBuf};
use std::time::Duration;

use tokio::process::Command;

use crate::{VttError, YtDlpConfig};

/// Result of a successful video download.
#[derive(Debug, Clone)]
pub struct DownloadResult {
    pub video_path: PathBuf,
    pub title: String,
}

/// Check that yt-dlp is available and return its version string.
pub async fn check_ytdlp(config: &YtDlpConfig) -> Result<String, VttError> {
    let output = Command::new(&config.path)
        .arg("--version")
        .output()
        .await
        .map_err(|e| VttError::Download(format!("failed to run yt-dlp: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(VttError::Download(format!(
            "yt-dlp exited with {}: {}",
            output.status,
            stderr.trim()
        )));
    }

    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(version)
}

/// Build the yt-dlp format selection string from config.
#[cfg(test)]
fn build_format_selector(config: &YtDlpConfig) -> String {
    format!(
        "bestvideo[height<={}][fps<={}]+bestaudio/best[height<={}]",
        config.max_resolution, config.max_fps, config.max_resolution
    )
}

/// Build the yt-dlp format selection string with overrides from the request.
fn build_format_selector_with_overrides(
    config: &YtDlpConfig,
    max_resolution: Option<&str>,
    max_fps: Option<u32>,
) -> String {
    let resolution = max_resolution.unwrap_or(&config.max_resolution);
    let fps = max_fps.unwrap_or(config.max_fps);
    format!(
        "bestvideo[height<={}][fps<={}]+bestaudio/best[height<={}]",
        resolution, fps, resolution
    )
}

/// Download a video from a URL using yt-dlp.
/// Returns the path to the downloaded mp4 file and the video title.
pub async fn download_video(
    config: &YtDlpConfig,
    url: &str,
    output_dir: &Path,
    max_resolution: Option<&str>,
    max_fps: Option<u32>,
) -> Result<DownloadResult, VttError> {
    tokio::fs::create_dir_all(output_dir)
        .await
        .map_err(|e| VttError::Download(format!("failed to create download dir: {e}")))?;

    let format_selector = build_format_selector_with_overrides(config, max_resolution, max_fps);
    let output_template = output_dir.join("%(title)s.%(ext)s").display().to_string();

    // First, get the filename yt-dlp will produce
    let filename_output = Command::new(&config.path)
        .args([
            "-f",
            &format_selector,
            "--merge-output-format",
            "mp4",
            "-o",
            &output_template,
            "--print",
            "filename",
            "--no-download",
        ])
        .arg(url)
        .output()
        .await
        .map_err(|e| VttError::Download(format!("failed to run yt-dlp: {e}")))?;

    if !filename_output.status.success() {
        let stderr = String::from_utf8_lossy(&filename_output.stderr);
        return Err(VttError::Download(format!(
            "yt-dlp failed to resolve filename: {}",
            stderr.trim()
        )));
    }

    let filename = String::from_utf8_lossy(&filename_output.stdout)
        .trim()
        .to_string();

    // Extract title from filename (remove extension)
    let title = Path::new(&filename)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    // Now download
    let output = tokio::time::timeout(
        Duration::from_secs(config.timeout_seconds),
        Command::new(&config.path)
            .args([
                "-f",
                &format_selector,
                "--merge-output-format",
                "mp4",
                "-o",
                &output_template,
                "--no-playlist",
            ])
            .arg(url)
            .output(),
    )
    .await
    .map_err(|_| {
        VttError::Download(format!(
            "yt-dlp download timed out after {}s",
            config.timeout_seconds
        ))
    })?
    .map_err(|e| VttError::Download(format!("failed to run yt-dlp: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(VttError::Download(format!(
            "yt-dlp download failed: {}",
            stderr.trim()
        )));
    }

    let video_path = PathBuf::from(&filename);
    if !video_path.exists() {
        return Err(VttError::Download(format!(
            "yt-dlp completed but output file not found: {}",
            video_path.display()
        )));
    }

    Ok(DownloadResult { video_path, title })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_format_selector_defaults() {
        let config = YtDlpConfig::default();
        let selector = build_format_selector(&config);
        assert_eq!(
            selector,
            "bestvideo[height<=1080][fps<=30]+bestaudio/best[height<=1080]"
        );
    }

    #[test]
    fn test_build_format_selector_custom() {
        let config = YtDlpConfig {
            max_resolution: "720".to_string(),
            max_fps: 60,
            ..YtDlpConfig::default()
        };
        let selector = build_format_selector(&config);
        assert_eq!(
            selector,
            "bestvideo[height<=720][fps<=60]+bestaudio/best[height<=720]"
        );
    }

    #[test]
    fn test_build_format_selector_with_overrides() {
        let config = YtDlpConfig::default();
        let selector = build_format_selector_with_overrides(&config, Some("480"), Some(24));
        assert_eq!(
            selector,
            "bestvideo[height<=480][fps<=24]+bestaudio/best[height<=480]"
        );
    }

    #[test]
    fn test_build_format_selector_partial_override() {
        let config = YtDlpConfig::default();
        let selector = build_format_selector_with_overrides(&config, Some("720"), None);
        assert_eq!(
            selector,
            "bestvideo[height<=720][fps<=30]+bestaudio/best[height<=720]"
        );
    }

    #[tokio::test]
    #[ignore]
    async fn test_check_ytdlp_available() {
        let config = YtDlpConfig::default();
        let version = check_ytdlp(&config).await.unwrap();
        assert!(!version.is_empty());
    }
}
