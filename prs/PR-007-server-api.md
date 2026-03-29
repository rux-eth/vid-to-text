# PR-007: Server HTTP API

## Scope

HTTP API on the server for receiving and processing jobs.

- `POST /jobs` — submit job with `{ "source": "/path/to/video.mp4" }`, returns 201 with job ID
- `GET /jobs/:id` — poll job status (queued/processing/completed/failed + error)
- `GET /jobs/:id/result` — retrieve Timeline JSON (409 if not completed)
- `GET /health` — enhanced: checks ffmpeg + Ollama availability, returns structured status
- Background processing via `tokio::spawn` with single-permit semaphore (GPU safety)
- In-memory job state and result storage via `AppState` with `Mutex<HashMap>`
- `ApiError` enum with proper HTTP status codes (400/404/409/500)

## Dependencies

PR-006

## Verification Criteria

- Request/response serialization correct
- Error field skipped when None, included when present
- Background task spawns and updates status
