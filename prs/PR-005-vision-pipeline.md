# PR-005: Vision Pipeline

**Landed-in:** v0.0 (untagged — versioning introduced in PR-019)

## Scope

Qwen3-VL visual description pipeline via Ollama HTTP API.

- `OllamaClient` — wraps reqwest client with timeout, prompt template, model config
- `describe_chunk(chunk, frame_paths, transcript)` — sends base64-encoded frames to Ollama, returns `[VISUAL]` segment
- Accepts optional transcript from Whisper so visual descriptions reference audio context
- Strips `<think>...</think>` tags from Qwen3-VL Thinking mode output
- Configurable prompt template (default built-in, overridable via file path)
- Request batching when frames exceed `max_frames_per_request`
- `check_health()` — verifies Ollama reachable and model loaded
- Extended `OllamaConfig` with `timeout_seconds`, `VisionConfig` with `max_frames_per_request`

## Dependencies

PR-003

## Verification Criteria

- Prompt template: default, from file, missing file error
- Request building: correct structure, serialization, empty images skipped
- Response parsing: normal, thinking tags, empty content, invalid JSON
- Frame encoding: valid base64 roundtrip, missing file error
- Integration tests (`#[ignore]`): health check, single frame description
