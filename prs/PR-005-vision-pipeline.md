# PR-005: Vision Pipeline

## Scope

Qwen3-VL visual description pipeline on the server, via Ollama HTTP API.

- Implement the `Pipeline` trait for vision: takes a `Chunk` (with extracted frames), returns `Vec<Segment>`
- Send frames to Ollama's Qwen3-VL-8B-Thinking endpoint with a prompt requesting verbose scene description with timestamps
- Parse model response into `[VISUAL]` segments with start/end timestamps
- Configurable: Ollama endpoint, model name, prompt template, FPS for frame extraction, max output tokens
- Handle Ollama API errors gracefully (timeout, model not loaded, etc.)
- Validate Ollama is running and model is available (used by `doctor`)

## Dependencies

PR-003

## Verification Criteria

- A chunk with visual content produces `[VISUAL]` segments with timestamps
- Timestamps are adjusted to be relative to the full video
- Prompt template is loaded from config (not hardcoded)
- Ollama connection failure produces a clear error
- Model not found produces a clear error
- `doctor` validates Ollama is reachable and model is pulled
