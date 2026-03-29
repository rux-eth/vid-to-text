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
 video.mp4 ────────▶  upload     ─────────────────────────▶│  receive + save     │
                    │              │                        │       │              │
                    │              │                        │  ffmpeg: chunk video │
                    │              │                        │       │              │
                    │              │                        │  for each chunk:     │
                    │              │                        │    │                 │
                    │              │                        │    ▼                 │
                    │              │                        │  Whisper (CPU)       │
                    │              │                        │  → [SPEECH] [SOUND]  │
                    │              │                        │    │                 │
                    │              │                        │    ▼ transcript      │
                    │              │                        │  Ollama (GPU)        │
                    │              │                        │  → [VISUAL]          │
                    │              │                        │    │                 │
                    │              │                        │    checkpoint chunk  │
                    │              │                        │                     │
                    │              │                        │  merge & sort        │
 video.json ◀───────  write output◀────────────────────────│  return JSON         │
                    │              │                        │                     │
                    └──────────────┘                        └─────────────────────┘
```

Per chunk, Whisper runs first (CPU) so the transcript can be passed to Qwen3-VL (GPU) as context. Chunks are processed sequentially in v1. Parallel chunk processing is a future optimization, though it requires care around cross-chunk context continuity.

## Key Abstractions

| Abstraction | Description |
|-------------|-------------|
| `Chunk` | A time-bounded segment of a video with start/end timestamps. Unit of processing and checkpointing. |
| `Segment` | A single output entry: type (`speech`, `visual`, `sound`), start time, end time, content text. |
| `ChunkArtifacts` | Extracted audio (WAV) and frames (JPEG) for a single chunk, produced by ffmpeg. |
| `ChunkManifest` | All artifacts for a job: job directory, video duration, and list of `ChunkArtifacts`. |
| `Timeline` | The merged, sorted collection of all `Segment`s for a video. Serializes to the final JSON output. |

## Storage

No database. File-based only:

- **Chunk checkpoints**: Completed chunk results stored as JSON files at `{temp_dir}/{job_id}/checkpoints/chunk_NNN.json`. Atomic writes (tmp + rename) ensure crash safety. Enables resumability.
- **Final output**: JSON file written alongside the input mp4 (default) or at a user-specified path.
- **Config**: TOML files at `~/.config/vid-to-text/`. Client uses `client.toml`, server uses `server.toml` (separate files since they run on different machines).

## Testing Strategy

| Level | What | Where |
|-------|------|-------|
| Unit | Config parsing, timestamp math, segment merging, chunk planning, checkpoint I/O | Inline `#[cfg(test)]` modules |
| Integration | ffmpeg probing, Whisper transcription, Ollama vision, full pipeline | `#[ignore]` tests requiring external deps |
| System | End-to-end: real mp4 → real models → correct JSON output | `vtt-core/tests/system_test.rs` |
