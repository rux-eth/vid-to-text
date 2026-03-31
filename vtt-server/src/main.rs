use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use axum::extract::{DefaultBodyLimit, Multipart, Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::io::AsyncWriteExt;
use tokio::sync::Semaphore;
use uuid::Uuid;
use vtt_core::{
    check_ffmpeg, check_ytdlp, download_video, load_config, load_profile, process_video,
    JobStatus, OllamaClient, ServerConfig, Timeline,
};

// --- Request/Response types ---

#[derive(Debug, Deserialize)]
struct CreateJobRequest {
    source: String,
    #[serde(default)]
    force: bool,
    profile: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UrlJobRequest {
    url: String,
    #[serde(default)]
    force: bool,
    max_resolution: Option<String>,
    max_fps: Option<u32>,
    profile: Option<String>,
}

#[derive(Debug, Serialize)]
struct JobResponse {
    id: Uuid,
    source: String,
    status: JobStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

// --- Internal state ---

struct JobEntry {
    id: Uuid,
    source: String,
    status: JobStatus,
    error: Option<String>,
}

impl JobEntry {
    fn to_response(&self) -> JobResponse {
        JobResponse {
            id: self.id,
            source: self.source.clone(),
            status: self.status.clone(),
            error: self.error.clone(),
        }
    }
}

struct AppState {
    config: ServerConfig,
    jobs: Mutex<HashMap<Uuid, JobEntry>>,
    results: Mutex<HashMap<Uuid, Timeline>>,
    processing_semaphore: Semaphore,
}

// --- Error handling ---

enum ApiError {
    NotFound(String),
    BadRequest(String),
    Conflict(String),
    Internal(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            ApiError::Conflict(msg) => (StatusCode::CONFLICT, msg),
            ApiError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };
        (status, Json(json!({ "error": message }))).into_response()
    }
}

// --- Background processing ---

/// Resolve an optional profile name to a ServerConfig.
fn resolve_config(base: &ServerConfig, profile: &Option<String>) -> Result<ServerConfig, ApiError> {
    match profile {
        Some(name) => load_profile(base, name)
            .map_err(|e| ApiError::BadRequest(e.to_string())),
        None => Ok(base.clone()),
    }
}

fn spawn_processing_task(state: Arc<AppState>, job_id: Uuid, video_path: PathBuf, force: bool, source_name: String, config: ServerConfig) {
    tokio::spawn(async move {
        let _permit = state.processing_semaphore.acquire().await.unwrap();

        {
            let mut jobs = state.jobs.lock().unwrap();
            if let Some(entry) = jobs.get_mut(&job_id) {
                entry.status = JobStatus::Processing;
            }
        }

        let job_id_str = job_id.to_string();
        match process_video(&config, &video_path, &job_id_str, force, Some(&source_name)).await {
            Ok(timeline) => {
                {
                    let mut results = state.results.lock().unwrap();
                    results.insert(job_id, timeline);
                }
                {
                    let mut jobs = state.jobs.lock().unwrap();
                    if let Some(entry) = jobs.get_mut(&job_id) {
                        entry.status = JobStatus::Completed;
                    }
                }
            }
            Err(e) => {
                let mut jobs = state.jobs.lock().unwrap();
                if let Some(entry) = jobs.get_mut(&job_id) {
                    entry.status = JobStatus::Failed;
                    entry.error = Some(e.to_string());
                }
            }
        }
    });
}

// --- Handlers ---

async fn health(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let ffmpeg_status = match check_ffmpeg(&state.config.ffmpeg).await {
        Ok(info) => json!({
            "ffmpeg_version": info.ffmpeg_version,
            "ffprobe_version": info.ffprobe_version,
        }),
        Err(e) => json!({ "error": e.to_string() }),
    };

    let ollama_status = match OllamaClient::new(&state.config.ollama, &state.config.vision) {
        Ok(client) => match client.check_health().await {
            Ok(()) => json!("ok"),
            Err(e) => json!({ "error": e.to_string() }),
        },
        Err(e) => json!({ "error": e.to_string() }),
    };

    let ytdlp_status = match check_ytdlp(&state.config.ytdlp).await {
        Ok(version) => json!({ "version": version }),
        Err(e) => json!({ "error": e.to_string() }),
    };

    let overall = if ffmpeg_status.get("error").is_none()
        && ollama_status == json!("ok")
        && ytdlp_status.get("error").is_none()
    {
        "ok"
    } else {
        "degraded"
    };

    Json(json!({
        "status": overall,
        "ffmpeg": ffmpeg_status,
        "ollama": ollama_status,
        "ytdlp": ytdlp_status,
    }))
}

async fn create_job(
    State(state): State<Arc<AppState>>,
    Json(request): Json<CreateJobRequest>,
) -> Result<impl IntoResponse, ApiError> {
    if request.source.is_empty() {
        return Err(ApiError::BadRequest("source must not be empty".into()));
    }

    let path = std::path::Path::new(&request.source);
    if !path.exists() {
        return Err(ApiError::BadRequest(format!(
            "file not found: {}",
            request.source
        )));
    }

    if path.extension().and_then(|e| e.to_str()) != Some("mp4") {
        return Err(ApiError::BadRequest("only mp4 files are supported".into()));
    }

    let job_id = Uuid::new_v4();
    let entry = JobEntry {
        id: job_id,
        source: request.source.clone(),
        status: JobStatus::Queued,
        error: None,
    };
    let response = entry.to_response();

    {
        let mut jobs = state.jobs.lock().unwrap();
        jobs.insert(job_id, entry);
    }

    let cfg = resolve_config(&state.config, &request.profile)?;
    spawn_processing_task(Arc::clone(&state), job_id, PathBuf::from(&request.source), request.force, request.source.clone(), cfg);

    Ok((StatusCode::CREATED, Json(response)))
}

async fn upload_job(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, ApiError> {
    let job_id = Uuid::new_v4();
    let job_dir = PathBuf::from(&state.config.processing.temp_dir).join(job_id.to_string());

    tokio::fs::create_dir_all(&job_dir)
        .await
        .map_err(|e| ApiError::Internal(format!("failed to create job dir: {e}")))?;

    let mut filename = String::new();
    let mut force = false;
    let upload_path = job_dir.join("input.mp4");

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| ApiError::BadRequest(format!("invalid multipart data: {e}")))?
    {
        if field.name() == Some("force") {
            let text = field.text().await.unwrap_or_default();
            force = text == "true";
            continue;
        }

        if field.name() == Some("file") {
            filename = field
                .file_name()
                .unwrap_or("upload.mp4")
                .to_string();

            if !filename.ends_with(".mp4") {
                return Err(ApiError::BadRequest("only mp4 files are supported".into()));
            }

            let mut file = tokio::fs::File::create(&upload_path)
                .await
                .map_err(|e| ApiError::Internal(format!("failed to create upload file: {e}")))?;

            let mut stream = field;
            while let Some(chunk) = stream
                .chunk()
                .await
                .map_err(|e| ApiError::Internal(format!("failed to read upload chunk: {e}")))?
            {
                file.write_all(&chunk)
                    .await
                    .map_err(|e| ApiError::Internal(format!("failed to write upload chunk: {e}")))?;
            }

            file.flush()
                .await
                .map_err(|e| ApiError::Internal(format!("failed to flush upload file: {e}")))?;

            break;
        }
    }

    if filename.is_empty() {
        return Err(ApiError::BadRequest(
            "missing 'file' field in multipart upload".into(),
        ));
    }

    let entry = JobEntry {
        id: job_id,
        source: filename.clone(),
        status: JobStatus::Queued,
        error: None,
    };
    let response = entry.to_response();

    {
        let mut jobs = state.jobs.lock().unwrap();
        jobs.insert(job_id, entry);
    }

    spawn_processing_task(Arc::clone(&state), job_id, upload_path, force, filename.clone(), state.config.clone());

    Ok((StatusCode::CREATED, Json(response)))
}

async fn create_url_job(
    State(state): State<Arc<AppState>>,
    Json(request): Json<UrlJobRequest>,
) -> Result<impl IntoResponse, ApiError> {
    if request.url.is_empty() {
        return Err(ApiError::BadRequest("url must not be empty".into()));
    }

    let job_id = Uuid::new_v4();
    let entry = JobEntry {
        id: job_id,
        source: request.url.clone(),
        status: JobStatus::Queued,
        error: None,
    };
    let response = entry.to_response();

    {
        let mut jobs = state.jobs.lock().unwrap();
        jobs.insert(job_id, entry);
    }

    // Resolve profile before spawning (so errors return to client)
    let config_override = resolve_config(&state.config, &request.profile)?;

    // Spawn background: download then process
    let state_clone = Arc::clone(&state);
    let url = request.url.clone();
    let force = request.force;
    let max_resolution = request.max_resolution.clone();
    let max_fps = request.max_fps;

    tokio::spawn(async move {
        let _permit = state_clone.processing_semaphore.acquire().await.unwrap();

        {
            let mut jobs = state_clone.jobs.lock().unwrap();
            if let Some(entry) = jobs.get_mut(&job_id) {
                entry.status = JobStatus::Processing;
            }
        }

        let job_id_str = job_id.to_string();
        let download_dir = std::path::PathBuf::from(&state_clone.config.processing.temp_dir)
            .join(&job_id_str);

        // Download video
        let download_result = download_video(
            &state_clone.config.ytdlp,
            &url,
            &download_dir,
            max_resolution.as_deref(),
            max_fps,
        )
        .await;

        let download = match download_result {
            Ok(d) => d,
            Err(e) => {
                let mut jobs = state_clone.jobs.lock().unwrap();
                if let Some(entry) = jobs.get_mut(&job_id) {
                    entry.status = JobStatus::Failed;
                    entry.error = Some(e.to_string());
                }
                return;
            }
        };

        // Update source to video title
        {
            let mut jobs = state_clone.jobs.lock().unwrap();
            if let Some(entry) = jobs.get_mut(&job_id) {
                entry.source = format!("{}.mp4", download.title);
            }
        }

        // Process video
        let source_name = format!("{}.mp4", download.title);
        match process_video(
            &config_override,
            &download.video_path,
            &job_id_str,
            force,
            Some(&source_name),
        )
        .await
        {
            Ok(timeline) => {
                {
                    let mut results = state_clone.results.lock().unwrap();
                    results.insert(job_id, timeline);
                }
                {
                    let mut jobs = state_clone.jobs.lock().unwrap();
                    if let Some(entry) = jobs.get_mut(&job_id) {
                        entry.status = JobStatus::Completed;
                    }
                }
            }
            Err(e) => {
                let mut jobs = state_clone.jobs.lock().unwrap();
                if let Some(entry) = jobs.get_mut(&job_id) {
                    entry.status = JobStatus::Failed;
                    entry.error = Some(e.to_string());
                }
            }
        }
    });

    Ok((StatusCode::CREATED, Json(response)))
}

async fn get_job(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<JobResponse>, ApiError> {
    let job_id: Uuid = id
        .parse()
        .map_err(|_| ApiError::BadRequest(format!("invalid job ID: {id}")))?;

    let jobs = state.jobs.lock().unwrap();
    let entry = jobs
        .get(&job_id)
        .ok_or_else(|| ApiError::NotFound(format!("job not found: {job_id}")))?;

    Ok(Json(entry.to_response()))
}

async fn get_job_result(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Timeline>, ApiError> {
    let job_id: Uuid = id
        .parse()
        .map_err(|_| ApiError::BadRequest(format!("invalid job ID: {id}")))?;

    let status = {
        let jobs = state.jobs.lock().unwrap();
        let entry = jobs
            .get(&job_id)
            .ok_or_else(|| ApiError::NotFound(format!("job not found: {job_id}")))?;
        entry.status.clone()
    };

    match status {
        JobStatus::Completed => {
            let results = state.results.lock().unwrap();
            let timeline = results
                .get(&job_id)
                .ok_or_else(|| ApiError::Internal("result missing for completed job".into()))?;
            Ok(Json(timeline.clone()))
        }
        JobStatus::Failed => Err(ApiError::Conflict("job failed".into())),
        _ => Err(ApiError::Conflict(format!(
            "job is not completed (status: {})",
            serde_json::to_string(&status)
                .unwrap_or_default()
                .trim_matches('"')
        ))),
    }
}

async fn cancel_job(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let job_id: Uuid = id
        .parse()
        .map_err(|_| ApiError::BadRequest(format!("invalid job ID: {id}")))?;

    let mut jobs = state.jobs.lock().unwrap();
    let entry = jobs
        .get_mut(&job_id)
        .ok_or_else(|| ApiError::NotFound(format!("job not found: {job_id}")))?;

    match entry.status {
        JobStatus::Queued => {
            entry.status = JobStatus::Failed;
            entry.error = Some("cancelled by user".into());
            Ok(Json(json!({ "cancelled": job_id.to_string() })))
        }
        JobStatus::Processing => {
            Err(ApiError::Conflict("cannot cancel a job that is currently processing".into()))
        }
        _ => {
            Err(ApiError::Conflict(format!("job already {}",
                serde_json::to_string(&entry.status).unwrap_or_default().trim_matches('"')
            )))
        }
    }
}

async fn cancel_all_jobs(
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    let mut jobs = state.jobs.lock().unwrap();
    let mut cancelled = 0;

    for entry in jobs.values_mut() {
        if entry.status == JobStatus::Queued {
            entry.status = JobStatus::Failed;
            entry.error = Some("cancelled by user".into());
            cancelled += 1;
        }
    }

    Json(json!({ "cancelled": cancelled }))
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

    let max_upload = config.processing.max_upload_bytes as usize;

    let state = Arc::new(AppState {
        config: config.clone(),
        jobs: Mutex::new(HashMap::new()),
        results: Mutex::new(HashMap::new()),
        processing_semaphore: Semaphore::new(config.server.max_concurrent_jobs as usize),
    });

    let app = Router::new()
        .route("/health", get(health))
        .route("/jobs", post(create_job))
        .route("/jobs/upload", post(upload_job))
        .route("/jobs/url", post(create_url_job))
        .route("/jobs/{id}", get(get_job))
        .route("/jobs/{id}/result", get(get_job_result))
        .route("/jobs/{id}", delete(cancel_job))
        .route("/jobs", delete(cancel_all_jobs))
        .layer(DefaultBodyLimit::max(max_upload))
        .with_state(state);

    let addr = config.bind_address();
    println!("vtt-server listening on {addr}");

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_response_serialization() {
        let resp = JobResponse {
            id: Uuid::nil(),
            source: "video.mp4".into(),
            status: JobStatus::Queued,
            error: None,
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["status"], "queued");
        assert_eq!(json["source"], "video.mp4");
        assert!(json.get("error").is_none());
    }

    #[test]
    fn test_job_response_with_error() {
        let resp = JobResponse {
            id: Uuid::nil(),
            source: "video.mp4".into(),
            status: JobStatus::Failed,
            error: Some("something broke".into()),
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["status"], "failed");
        assert_eq!(json["error"], "something broke");
    }

    #[test]
    fn test_create_job_request_deserialization() {
        let json = r#"{"source": "/path/to/video.mp4"}"#;
        let req: CreateJobRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.source, "/path/to/video.mp4");
    }
}
