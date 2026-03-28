# Architecture

## System Overview

vid-to-text is a client/server system that converts mp4 videos into structured JSON combining speech transcription, visual scene descriptions, and sound event tags. The CLI client runs on the user's laptop and dispatches processing jobs to a server running on a desktop with an RTX 4090. The server orchestrates ffmpeg, Whisper (CPU), and Qwen3-VL via Ollama (GPU) to produce a unified, timestamped segment timeline.

## Components

| Component | Location | Description |
|-----------|----------|-------------|
| `vtt-client` | Laptop | Rust CLI binary. Sends mp4 files to the server, receives JSON results. Handles config, output writing, batch directory processing. |
| `vtt-server` | Desktop | Rust HTTP server. Receives jobs, chunks video, dispatches to Whisper and Qwen3-VL, merges results, returns JSON. |
| `ffmpeg` | Desktop | External dependency. Splits video into time-based chunks, extracts audio tracks, extracts frames. |
| `Ollama` | Desktop | External dependency. Serves Qwen3-VL-8B-Thinking model via HTTP API. |
| `Whisper` | Desktop | Integrated via `whisper-rs`. Runs on CPU for speech transcription and non-speech sound tagging. |

## Data Flow

```
                         LAPTOP                                    DESKTOP
                    ┌──────────────┐                        ┌─────────────────────┐
                    │  vtt-client  │                        │     vtt-server      │
                    │              │    HTTP/Tailscale       │                     │
 video.mp4 ────────▶  send job   ─────────────────────────▶│  receive job        │
                    │              │                        │       │              │
                    │              │                        │  ffmpeg: chunk video │
                    │              │                        │       │              │
                    │              │                        │  ┌────┴────┐         │
                    │              │                        │  │         │         │
                    │              │                        │ audio    frames      │
                    │              │                        │  │         │         │
                    │              │                        │ Whisper  Ollama      │
                    │              │                        │ (CPU)   (GPU)        │
                    │              │                        │  │         │         │
                    │              │                        │ [SPEECH] [VISUAL]    │
                    │              │                        │ [SOUND]    │         │
                    │              │                        │  │         │         │
                    │              │                        │  └────┬────┘         │
                    │              │                        │       │              │
                    │              │                        │  merge & sort        │
 video.json ◀───────  write output◀────────────────────────│  return JSON         │
                    │              │                        │                     │
                    └──────────────┘                        └─────────────────────┘
```

## Key Abstractions

| Abstraction | Description |
|-------------|-------------|
| `Job` | A processing request: source file path, config overrides, job ID. |
| `Chunk` | A time-bounded segment of a video with start/end timestamps. Unit of processing and checkpointing. |
| `Segment` | A single output entry: type (`speech`, `visual`, `sound`), start time, end time, content text. |
| `Pipeline` | Trait for a processing stage. Whisper and Qwen3-VL each implement this — takes a `Chunk`, returns `Vec<Segment>`. |
| `Timeline` | The merged, sorted collection of all `Segment`s for a video. Serializes to the final JSON output. |

## Storage

No database. File-based only:

- **Chunk checkpoints**: Completed chunk results stored as JSON files in a temp directory on the server (e.g., `/tmp/vtt-jobs/<job-id>/chunk-003.json`). Enables resumability.
- **Final output**: JSON file written alongside the input mp4 (default) or at a user-specified path.
- **Config**: TOML files at `~/.config/vid-to-text/config.toml` on both client and server.

## Testing Strategy

| Level | What | Where |
|-------|------|-------|
| Unit | Config parsing, timestamp math, segment merging, chunk planning | Inline `#[cfg(test)]` modules |
| Integration | Client-server communication, job lifecycle, checkpoint/resume | `tests/` directory with mock pipelines |
| System | End-to-end: real mp4 → real models → correct JSON output | Manual + scripted tests against sample videos |
