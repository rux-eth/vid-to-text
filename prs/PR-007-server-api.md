# PR-007: Server HTTP API

## Scope

HTTP API on the server for receiving and processing jobs.

- `POST /jobs` — submit a new processing job (receives mp4 file or file path)
- `GET /jobs/:id` — check job status (queued, processing, completed, failed)
- `GET /jobs/:id/result` — download completed JSON result
- `GET /health` — server health check (used by client and doctor)
- Job lifecycle: receive → chunk → process → merge → complete
- Wire up the full pipeline: chunking → parallel Whisper + Qwen3-VL → merge → store result
- Configurable: listen address, port

## Dependencies

PR-006

## Verification Criteria

- POST /jobs accepts an mp4 file and returns a job ID
- GET /jobs/:id returns correct status through the lifecycle
- GET /jobs/:id/result returns valid JSON for a completed job
- GET /health returns 200 when server is running
- Submitting a job for a non-mp4 file returns an error
- Server binds to configured address/port
