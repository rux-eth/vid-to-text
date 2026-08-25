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

## Capture Configuration

The operating point for market-research corpus capture. Every value below is research-backed; see
`prs/PR-020-market-research-capture-config.md` Research findings for sources and epistemic status.

| Setting | Value | Basis |
|---|---|---|
| `whisper.beam_size` | 5 | Whisper paper Sec 4.5 prescribes beam-5 from temperature 0; greedy is documented to fall into repetition loops on long-form audio. |
| `whisper.initial_prompt` | `""` (off) | Upstream-documented failure mode: the prompt feature "will cause specific hallucinations and repetitions". Its jargon benefit did not materialise on this corpus. |
| `whisper.entropy_thold` | 2.4 | whisper.cpp default; its analogue of OpenAI's `compression_ratio_threshold`. **Retry trigger, not a filter.** |
| `whisper.logprob_thold` | -1.0 | whisper.cpp / Whisper paper Sec 4.5 default. |
| `whisper.no_speech_thold` | 0.6 | whisper.cpp / Whisper paper Sec 4.5 default. |
| `whisper.temperature_inc` | 0.2 | Temperature ladder 0.0 -> 1.0. Note the paper's own Table 7 shows zero WER gain from fallback. |
| `vision.use_transcript` | `false` | Audio-to-vision conditioning is the documented-harmful direction: language priors displace visual grounding. |
| `vision.transcript_window` | `causal` | Enforces the Corpus Look-Ahead Freedom constraint. |
| `ollama.temperature` | 0.0 | Removes sampling variance. |
| `ffmpeg.chunk_duration_secs` | 180 | Retained. Its original stated rationale (a "768-frame cap") is unsupported — see `docs/0.0/DESIGN-log.md`. |

**Deliberately not locked**, recorded so the gap is legible rather than implied-settled:

| Setting | Value carried | Why not locked |
|---|---|---|
| `vision.fps` | **unset (sentinel `0.0`)** | Phase 5.5 measured the corpus directly: meaningful content change every 48-72s, so every fps tested oversamples 11-90x, and the rate varies ~3x between videos. The diversity metrics cannot choose a value (raw scores are length-confounded; controlling length introduces a time-coverage confound). Escalated to **PR-022** (content-adaptive sampling). A job using this profile fails validation until a value is set. |
| `vision.max_frames_per_request` | 15 | Measured effect of raising it was -5.9% wall time, not statistically established. |
| `whisper.model_path` | `large-v3-turbo` | Measured equal to `large-v3` on repetition and content retention, at 2.3x lower cost. |

**Reproducibility, stated honestly.** `temperature = 0` removes sampling variance but does **not**
deliver bit-identical output. Non-determinism at temperature 0 arises from batch-size dependence of
reduction kernels, and the accepted fix — batch-invariant kernels, ~60% throughput overhead — is not
implemented by Ollama. A fixed seed does not help, because greedy decoding has no sampling step to
seed. Runs are repeatable in distribution, not bit-exact.

**Repetition guarding is asymmetric.** `truncate_repetition` (`vtt-core/src/vision.rs`) guards vision
output only. Whisper output is ungated: its in-decoder thresholds are retry triggers that accept the
result unconditionally at the final temperature. Post-hoc `compression_ratio` flagging closes this,
as a diagnostic that flags rather than a filter that edits.


## Testing Strategy

| Level | What | Where |
|-------|------|-------|
| Unit | Config parsing, timestamp math, segment merging, chunk planning, checkpoint I/O | Inline `#[cfg(test)]` modules |
| Integration | ffmpeg probing, Whisper transcription, Ollama vision, full pipeline | `#[ignore]` tests requiring external deps |
| System | End-to-end: real mp4 → real models → correct JSON output | `vtt-core/tests/system_test.rs` |
