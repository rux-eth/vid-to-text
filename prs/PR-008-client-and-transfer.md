# PR-008+009: Client CLI Commands with File Transfer

**Landed-in:** v0.0 (untagged — versioning introduced in PR-019)

## Scope

Combined PR-008 (client commands) and PR-009 (file transfer) — client can't function without upload.

**Server:**
- `POST /jobs/upload` — streaming multipart mp4 upload (no full-file buffering for 1GB+ files)
- `spawn_processing_task()` refactored as shared helper
- `DefaultBodyLimit` layer (configurable `max_upload_bytes`, default 4GB)

**Client:**
- `vid-to-text process <file.mp4>` — upload → poll status → download result → write JSON
- `vid-to-text process <directory>` — find all mp4s, process each sequentially
- `vid-to-text doctor` — config display + server health check
- Streaming upload via `reqwest::multipart::Part::stream()` with `ReaderStream`
- Configurable polling: `poll_interval_secs` (default 3), `timeout_secs` (default 1800)

**Client structure:** Modularized into `main.rs` (CLI), `api.rs` (HTTP), `process.rs` (orchestration), `doctor.rs` (health check).

## Dependencies

PR-007

## Verification Criteria

- Output path computation: alongside input, with --output dir
- mp4 file discovery: empty dir, mixed files, sorted
- Config: polling defaults, validation rejects zero values
