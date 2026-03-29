# CLAUDE.md

<!-- STATUS: initialized -->

## What Is vid-to-text

A Rust client/server tool that converts mp4 videos into structured JSON combining timestamped speech transcription (`[SPEECH]`), visual scene descriptions (`[VISUAL]`), and sound event tags (`[SOUND]`). The CLI client runs on the user's laptop and dispatches jobs to a server on a desktop with an RTX 4090, which runs Whisper (CPU) and Qwen3-VL-8B-Instruct-Q8 (GPU via Ollama) sequentially per chunk — Whisper first, then Vision with the transcript as context.

A `format` subcommand sends the JSON output to OpenAI's GPT-5.4 to produce human-readable Markdown with summary and detailed narrative transcript.

## Build & Test Commands

```bash
cargo build --workspace        # Build all crates
cargo test --workspace         # Run all tests
cargo run -p vtt-client -- --help   # Client CLI help
cargo run -p vtt-server -- --help   # Server help
```

## CLI Usage

```bash
# Process a local mp4 file
vid-to-text process video.mp4 --server host:port

# Process a YouTube URL
vid-to-text process --url "https://youtube.com/watch?v=..." --server host:port

# Process a directory of mp4 files
vid-to-text process ./videos/ --server host:port

# Format JSON output into human-readable Markdown
vid-to-text format video.json

# Check system health
vid-to-text doctor
```

## Architecture

Client/server split: `vtt-client` (laptop) sends mp4 files or YouTube URLs over HTTP to `vtt-server` (desktop with GPU). Server chunks video via ffmpeg, then for each chunk: runs Whisper on CPU first, passes the transcript to Qwen3-VL on GPU for context-aware visual description. Visual segments are produced per batch (~7.5s granularity at 2fps/15 frames). All segments are merged into a sorted JSON timeline and returned to the client.

The `format` command runs client-side only — reads the JSON, sends to OpenAI API, writes Markdown.

See `docs/ARCHITECTURE.md` for the full architecture reference.

## Implementation Status

See `docs/ROADMAP.md` for the ordered PR plan. Full PR descriptions in `prs/`.

**v1 complete.** Post-v1 features added: YouTube URL support, human-readable format command, vision pipeline optimizations.

## Design References

- `docs/ARCHITECTURE.md` — canonical architecture reference
- `docs/CONSTRAINTS.md` — hard rules
- `docs/ROADMAP.md` — PR index with phases and dependencies
- `prs/` — full PR descriptions
- `docs/DESIGN-log.md` — design conversation log
- `PROCEDURE-design-planning.md` — how to run design sessions
- `PROCEDURE-code-audit.md` — post-design-session code audit

## Key Configuration

**Server** (`~/.config/vid-to-text/server.toml`):
- `whisper.model_path` — path to Whisper GGUF model file
- `whisper.n_threads` — CPU threads for transcription (default 8)
- `ollama.model` — vision model (recommended: `qwen3-vl:8b-instruct-q8_0`)
- `ollama.num_ctx` — context window for multi-image requests (default 65536)
- `vision.max_frames_per_request` — frames per Ollama batch (default 15)
- `vision.fps` — frame extraction rate (default 2.0)
- `ffmpeg.chunk_duration_secs` — chunk size (default 180)

**Client** (`~/.config/vid-to-text/client.toml`):
- `server.host` / `server.port` — server address
- `openai.model` — GPT model for format command (default gpt-5.4)
- API key via `OPENAI_API_KEY` env var or `.env` file

## Constraints

- **No phantom implementations** — if it's not tested, it's not done
- **Documentation accuracy** — every code change includes doc updates in the same commit
- **One PR, one thing** — no scope creep
- **Config over hardcoding** — zero hardcoded parameters, all values from TOML config
- **No audio on GPU** — Whisper on CPU only, full VRAM for Qwen3-VL
- **mp4 only** — no format conversion
- **Segments immutable after merge** — output is faithful to model output
- **Checkpoint integrity** — only fully-processed chunks are checkpointed
- **Client never talks to models directly** — all model interaction through the server
