use std::path::Path;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use tokio_util::io::ReaderStream;
use vtt_core::{ClientConfig, Timeline};

#[derive(Debug, Deserialize)]
pub struct JobResponse {
    pub id: String,
    #[allow(dead_code)]
    pub source: String,
    pub status: String,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Debug, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub ffmpeg: serde_json::Value,
    pub ollama: serde_json::Value,
    #[serde(default)]
    pub ytdlp: serde_json::Value,
    /// "disabled", {"version": ...} or {"error": ...} (PR-023).
    #[serde(default)]
    pub ocr: serde_json::Value,
}

/// Upload an mp4 file to the server and return the job response.
pub async fn upload_file(
    client: &reqwest::Client,
    server_url: &str,
    input_path: &Path,
    force: bool,
) -> Result<JobResponse, String> {
    let filename = input_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let file = tokio::fs::File::open(input_path)
        .await
        .map_err(|e| format!("failed to open {}: {e}", input_path.display()))?;

    let stream = ReaderStream::new(file);
    let body = reqwest::Body::wrap_stream(stream);
    let part = reqwest::multipart::Part::stream(body)
        .file_name(filename)
        .mime_str("video/mp4")
        .map_err(|e| format!("failed to set mime type: {e}"))?;
    let mut form = reqwest::multipart::Form::new().part("file", part);
    if force {
        form = form.text("force", "true");
    }

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

    resp.json()
        .await
        .map_err(|e| format!("failed to parse job response: {e}"))
}

/// Poll job status until completed or failed. Returns the final status.
pub async fn poll_until_done(
    client: &reqwest::Client,
    server_url: &str,
    job_id: &str,
    filename: &str,
    config: &ClientConfig,
) -> Result<(), String> {
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
                "timed out after {}s waiting for job {job_id}",
                config.polling.timeout_secs
            ));
        }

        let resp = client
            .get(format!("{server_url}/jobs/{job_id}"))
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
            "completed" => return Ok(()),
            "failed" => {
                let msg = status.error.unwrap_or_else(|| "unknown error".into());
                return Err(format!("job failed: {msg}"));
            }
            _ => continue,
        }
    }
}

/// Download the completed timeline result for a job.
pub async fn download_result(
    client: &reqwest::Client,
    server_url: &str,
    job_id: &str,
) -> Result<Timeline, String> {
    let resp = client
        .get(format!("{server_url}/jobs/{job_id}/result"))
        .send()
        .await
        .map_err(|e| format!("failed to download result: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("failed to get result: {}", resp.status()));
    }

    resp.json()
        .await
        .map_err(|e| format!("failed to parse timeline: {e}"))
}

#[derive(Debug, Serialize)]
struct UrlJobRequestBody {
    url: String,
    #[serde(default)]
    force: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_resolution: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_fps: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    profile: Option<String>,
}

/// Submit a URL job to the server for download and processing.
pub async fn submit_url_job(
    client: &reqwest::Client,
    server_url: &str,
    url: &str,
    force: bool,
    max_resolution: Option<String>,
    max_fps: Option<u32>,
    profile: Option<String>,
) -> Result<JobResponse, String> {
    let body = UrlJobRequestBody {
        url: url.to_string(),
        force,
        max_resolution,
        max_fps,
        profile,
    };

    let resp = client
        .post(format!("{server_url}/jobs/url"))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("failed to submit URL job: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        if let Ok(err) = serde_json::from_str::<ErrorResponse>(&body) {
            return Err(format!("server error ({status}): {}", err.error));
        }
        return Err(format!("server error ({status}): {body}"));
    }

    resp.json()
        .await
        .map_err(|e| format!("failed to parse job response: {e}"))
}

/// Fetch the server health status.
pub async fn fetch_health(
    client: &reqwest::Client,
    server_url: &str,
) -> Result<HealthResponse, String> {
    let resp = client
        .get(format!("{server_url}/health"))
        .send()
        .await
        .map_err(|e| format!("server not reachable: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("server returned {}", resp.status()));
    }

    resp.json()
        .await
        .map_err(|e| format!("failed to parse health response: {e}"))
}
