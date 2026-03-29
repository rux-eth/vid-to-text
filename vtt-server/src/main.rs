use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::Semaphore;
use uuid::Uuid;
use vtt_core::{
    check_ffmpeg, load_config, process_video, JobStatus, OllamaClient, ServerConfig, Timeline,
};

// --- Request/Response types ---

#[derive(Debug, Deserialize)]
struct CreateJobRequest {
    source: String,
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

    let overall = if ffmpeg_status.get("error").is_none() && ollama_status == json!("ok") {
        "ok"
    } else {
        "degraded"
    };

    Json(json!({
        "status": overall,
        "ffmpeg": ffmpeg_status,
        "ollama": ollama_status,
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

    // Spawn background processing
    let state_clone = Arc::clone(&state);
    let source = request.source.clone();
    tokio::spawn(async move {
        // Acquire semaphore — only one job processes at a time (GPU safety)
        let _permit = state_clone.processing_semaphore.acquire().await.unwrap();

        // Update status to Processing
        {
            let mut jobs = state_clone.jobs.lock().unwrap();
            if let Some(entry) = jobs.get_mut(&job_id) {
                entry.status = JobStatus::Processing;
            }
        }

        let job_id_str = job_id.to_string();
        let video_path = std::path::Path::new(&source);

        match process_video(&state_clone.config, video_path, &job_id_str).await {
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
            serde_json::to_string(&status).unwrap_or_default().trim_matches('"')
        ))),
    }
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

    let state = Arc::new(AppState {
        config: config.clone(),
        jobs: Mutex::new(HashMap::new()),
        results: Mutex::new(HashMap::new()),
        processing_semaphore: Semaphore::new(1),
    });

    let app = Router::new()
        .route("/health", get(health))
        .route("/jobs", post(create_job))
        .route("/jobs/{id}", get(get_job))
        .route("/jobs/{id}/result", get(get_job_result))
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
