# CLAUDE.md

<!-- STATUS: initialized -->

## What Is vid-to-text

A Rust client/server tool that converts mp4 videos into structured JSON combining timestamped speech transcription (`[SPEECH]`), visual scene descriptions (`[VISUAL]`), and sound event tags (`[SOUND]`). The CLI client runs on the user's laptop and dispatches jobs to a server on a desktop with an RTX 4090, which runs Whisper (CPU) and Qwen3-VL-8B-Instruct-Q8 (GPU via Ollama) sequentially per chunk — Whisper first, then Vision with the transcript as context.

A `format` subcommand sends the JSON output to OpenAI's GPT-5.4 to produce human-readable Markdown with summary and detailed narrative transcript.

## Build & Test Commands

```bash
cargo build --workspace        # Build all crates
cargo test --workspace         # Run all tests
cargo test -p vtt-core config::tests::test_name   # Run a single test by path
cargo test --workspace -- --nocapture             # Show println output during tests
cargo run -p vtt-client -- --help   # Client CLI help
cargo run -p vtt-server -- --help   # Server help
```

External runtime dependencies (not in Cargo.toml; `doctor` checks for these):
- `ffmpeg` / `ffprobe` on PATH
- `yt-dlp` on PATH (for `--url` inputs)
- RapidOCR (`pip install rapidocr onnxruntime` in a venv) reachable via `fidelity.ocr_command`, only when the fidelity diagnostic is enabled
- Ollama running on the server host with the configured vision model pulled

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

# Render a fidelity review sheet for a job's results directory (server side)
vid-to-text review ~/.vid-to-text/server/results/<job-id>

# Re-score a job's fidelity offline (candidates reference / other tolerance)
vid-to-text rescore ~/.vid-to-text/server/results/<job-id> --candidates cand.jsonl
```

## Architecture

Client/server split: `vtt-client` (laptop) sends mp4 files or YouTube URLs over HTTP to `vtt-server` (desktop with GPU). Server chunks video via ffmpeg, then for each chunk: runs Whisper on CPU first, passes the transcript to Qwen3-VL on GPU for context-aware visual description. Frames are selected per chunk either at a fixed rate or content-adaptively (uniform floor + scene-change triggers; see `docs/ARCHITECTURE.md` § Frame Sampling), and visual segments are produced per request batch with real frame timestamps. All segments are merged into a sorted JSON timeline and returned to the client.

The `format` command runs client-side only — reads the JSON, sends to OpenAI API, writes Markdown.

Workspace layout:
- `vtt-core/` — pipeline orchestration, config, ffmpeg, whisper, vision, checkpoint, ytdlp (library used by both client and server)
- `vtt-client/` — CLI (`process` / `format` / `doctor`), HTTP client, response cache
- `vtt-server/` — single `main.rs` axum server that wraps `vtt-core::pipeline`
- `prompts/` — `vision.txt` (Qwen3-VL system prompt), `format.txt` (GPT format prompt)

The top-level `README.md` is the `vibe-rails` template README, not documentation for this project — read `docs/ARCHITECTURE.md` for project context.

See `docs/ARCHITECTURE.md` for the full architecture reference.

## Implementation Status

See `docs/0.0/ROADMAP.md` for the ordered PR plan. Full PR descriptions in `prs/`.

**v1 complete.** Post-v1 features added: YouTube URL support, human-readable format command, vision pipeline optimizations.

## Ongoing Behavior (MANDATORY)

- **Every PR runs `PROCEDURE-pr-research.md` before implementation.** No exceptions. Tier-1 PRs get the light path (Phase 1 State Assessment); Tier-2 PRs get the full 5-phase path.
- **Research findings travel with the PR** — appended to the PR file's `## Research findings` section.
- **State drifts** — even research-backed decisions need Phase 1 state assessment before implementation.
- **Every minor/major version bump runs `PROCEDURE-design-planning.md` first.** See `docs/VERSIONING.md`. Patch bumps do not.
- **Halt at every phase boundary** — per the Per-Phase Approval Gate in `docs/CONSTRAINTS.md`, a response covers at most one phase, then requests approval.

## Design References

- `docs/ARCHITECTURE.md` — canonical architecture reference
- `docs/CONSTRAINTS.md` — hard rules
- `docs/0.0/ROADMAP.md` — PR index with phases and dependencies
- `prs/` — full PR descriptions
- `docs/0.0/DESIGN-log.md` — design conversation log (versioned)
- `docs/0.0/RESEARCH-BACKLOG.md` — per-PR research status + drift watch (versioned)
- `docs/CONVENTIONS.md` — code conventions (flat, SSOT)
- `docs/DEPLOYMENT.md` — deploy how-to (flat, SSOT)
- `docs/VERSIONING.md` — versioning policy + bump rules + changelog format (flat, meta-rule)
- `/CHANGELOG.md` — user-facing changelog (Keep-a-Changelog 1.1.0)
- `prs/PR-TEMPLATE.md` — start new PRs from this template
- `PROCEDURE-pr-research.md` — mandatory research procedure before every PR implementation
- `PROCEDURE-design-planning.md` — how to run design sessions
- `PROCEDURE-code-audit.md` — post-design-session code audit

## Remote server access

The GPU desktop (RTX 4090, Tailscale) is reachable via the `ssh-desktop` shell alias (= `ssh rux@100.90.42.41`). The server is launched manually and foreground when needed — there is no systemd unit, launch script, or persistent log file. The repo is also checked out at `~/vid-to-text/` on the desktop.

Useful one-shots (prefer `ssh-desktop '<cmd>'` over interactive sessions):

```bash
ssh-desktop 'cat ~/.vid-to-text/config/server.toml'    # Server config on desktop
ssh-desktop 'pgrep -a vtt-server'                      # Is the server running?
ssh-desktop 'ollama ps'                                # Loaded vision model + VRAM usage
ssh-desktop 'nvidia-smi'                               # GPU state
ssh-desktop 'ls ~/vid-to-text/target/release/'         # Release binaries on desktop
ssh-desktop 'ls /tmp/vtt-jobs/'                        # Active job working dirs (default processing.temp_dir)
```

To launch the server on the desktop manually:
```bash
ssh-desktop 'cd ~/vid-to-text && cargo run --release -p vtt-server'
```

## Key Configuration

**Server** (`~/.vid-to-text/config/server.toml`):
- `whisper.model_path` — path to Whisper GGUF model file
- `whisper.n_threads` — CPU threads for transcription (default 8)
- `ollama.model` — vision model (recommended: `qwen3-vl:8b-instruct-q8_0`)
- `ollama.num_ctx` — context window for multi-image requests (default 65536)
- `ollama.prompt_template_path` — vision prompt file, relative to the server's working directory
  (`prompts/vision.txt` general, `prompts/vision-chart.txt` for chart/screencast content; the
  `market-research` profile selects the latter). Deploy with `config/deploy-prompts.sh`.
- `vision.max_frames_per_request` — frames per Ollama batch (default 15)
- `vision.fps` — frame candidate rate (default 2.0); in fixed mode every candidate is kept
- `[vision.adaptive]` — `enabled` (default false), `scene_threshold`, `max_gap_secs`,
  `min_trigger_interval_secs`, `max_frames_per_chunk` — see `docs/ARCHITECTURE.md` § Frame Sampling
- `ollama.prompt_reserve_tokens` — context reserved for prompt text in the per-request token pre-flight
- `whisper.repetition_window_secs` — window over which the post-hoc repetition report is scored
- `vision.max_numeric_run` — cap on consecutive numbers in a visual description, guarding against degenerate enumeration (default 40; 0 disables)
- `vision.max_skeleton_repeat` — cap on how many times one sentence *skeleton* (the sentence with its numeric tokens masked) may recur in a visual description, guarding against a repeated template with a varying slot (default 24; 0 disables)
- `vision.min_skeleton_chars` — minimum skeleton length in characters for `max_skeleton_repeat` (default 10)
- `[ocr]` — OCR engine shared by the fidelity diagnostic and OCR grounding (`command`, `workers`, `threads`)
- `[vision.ocr_grounding]` — give the vision model each frame's detected text (`enabled`, `max_items_per_frame`, `min_score`, `tokens_per_item`) — see `docs/ARCHITECTURE.md` § Frame Sampling
- `[fidelity]` — post-run visual fidelity diagnostic (`enabled`, `recall_reference`, `number_tolerance`, `min_persist_secs`, `min_text_height_px`, `label_stoplist`, thumbnails) — see `docs/ARCHITECTURE.md` § Fidelity Diagnostic
- `ffmpeg.chunk_duration_secs` — chunk size (default 180)

**Client** (`~/.vid-to-text/config/client.toml`):
- `server.host` / `server.port` — server address (default port: **3001**; port 3000 is not available on the desktop)
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
