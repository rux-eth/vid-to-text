# Design Log

Design decisions and rationale from planning sessions. Append new sessions below.

---

## Session: 2026-03-28 — Initial Design

### Context

Greenfield project. User wants a CLI tool to convert mp4 videos into structured text combining speech transcription, visual scene descriptions, and sound event tags. Prior research confirmed no existing tool does this — the project fills a gap in the ecosystem.

### Research Summary

Evaluated Video-LLaVA (outdated, 8 frames only), InternVideo2.5 (capable but heavy, research-grade), Qwen2.5-VL-7B (strong but superseded), and Qwen3-VL-8B (best fit). Chose Qwen3-VL-8B-Thinking for visual analysis — fits on RTX 4090, Thinking mode produces verbose descriptions, strong temporal reasoning. Whisper via whisper-rs for speech transcription.

### Decisions

1. **Client/server architecture**: CLI on laptop, processing server on desktop (RTX 4090). Connected via Tailscale.
   **Rationale**: GPU is on a separate machine from the development laptop. Tailscale provides reliable connectivity from anywhere.

2. **Fixed time chunking (configurable, default ~3 min)**: Videos split into time-based chunks for processing.
   **Rationale**: Derived from Qwen3-VL's 768-frame cap at 2fps. Simple, predictable. Scene-based splitting deferred as enhancement.

3. **Ollama for model serving**: Qwen3-VL served via Ollama HTTP API on desktop.
   **Rationale**: Mature tooling, easy model management, clean HTTP interface. Avoids Python subprocess complexity or immature ONNX export.

4. **Parallel GPU+CPU processing**: Qwen3-VL on GPU, Whisper on CPU simultaneously.
   **Rationale**: Avoids VRAM contention (Qwen3-VL needs ~16-20GB, leaving no room for Whisper). i9-13900k handles Whisper at reasonable speed.

5. **Whisper non-speech tags for [SOUND] stream (v1)**: Leverage Whisper's incidental non-speech tags rather than a dedicated classifier.
   **Rationale**: Free — already running Whisper. Good enough for v1. Dedicated audio classifier (hybrid approach) deferred.

6. **Flat sorted timeline**: All segments merged into a single list sorted by start time.
   **Rationale**: Simplest structure, most flexible for consumers. Nesting/grouping can be done by a presentation layer.

7. **TOML config with CLI overrides**: Config at `~/.config/vid-to-text/config.toml`, CLI flags override.
   **Rationale**: TOML is idiomatic Rust, human-readable, supports comments. Separate client/server configs.

8. **Chunk-level checkpointing**: Completed chunks saved to disk for resumability.
   **Rationale**: Long videos may take significant time. Losing progress on failure is unacceptable. File-based state keeps it simple.

9. **Dependencies pre-installed with doctor command**: User installs ffmpeg, Ollama, etc. manually. `vid-to-text doctor` validates setup.
   **Rationale**: Single user, personal tool. Docker is overkill.

### Scope

**v1**: Single/batch mp4 processing, [SPEECH]/[VISUAL]/[SOUND] streams, JSON output, client/server, TOML config, resumability, doctor command.

**Later**: Wake-on-LAN, YouTube URL support, dedicated sound classifier, human-readable output layer, SRT/VTT export.

### Changes to docs
- Populated ARCHITECTURE.md with full system design
- Added domain constraints to CONSTRAINTS.md
- Created PR plan in ROADMAP.md
- Created PR description files in prs/

## Session: 2026-03-29 — Post-v1 Enhancements & Optimization

### Context

v1 complete and system-tested. Session focused on post-v1 features, quality improvements, and performance optimization through iterative testing with real YouTube videos.

### Decisions

1. **YouTube URL support (PR-011)**: Server downloads via yt-dlp, client sends `--url` flag. Configurable resolution/FPS.
   **Rationale**: User shouldn't need to manually download videos.

2. **Human-readable format command (PR-012)**: GPT-5.4 via OpenAI API produces Markdown from JSON. Separate `format` subcommand runs client-side.
   **Rationale**: Machine-readable JSON needs a presentation layer. Separate command allows re-formatting without reprocessing.

3. **Q8 Instruct model over Q4 Thinking (PR-013)**: Switched from `qwen3-vl:8b` (Q4_K_M, Thinking) to `qwen3-vl:8b-instruct-q8_0`. 3x more reliable, faster per batch, no empty response issues.
   **Rationale**: Q4 Thinking variant produced empty responses non-deterministically. Q8 Instruct uses more VRAM (20GB/24GB) but eliminates the problem.

4. **Granular visual segments (PR-013)**: One visual segment per batch (~7.5s) instead of one per chunk (3min). Went from 2 to 28 visual segments.
   **Rationale**: Fine-grained timestamps correlate better with speech for the format command.

5. **Overlapped processing (PR-014)**: Pre-spawn Whisper(N+1) on CPU while Vision(N) runs on GPU. Cross-chunk context passed to vision prompt.
   **Rationale**: Whisper is free when overlapped. Cross-chunk context prevents vision model from losing continuity at chunk boundaries.

6. **Release build**: Whisper 2.6x faster in release vs debug (45s vs 118s for 3-min chunk).
   **Rationale**: Free performance win for CPU-bound work.

7. **CRISPE prompt framework (PR-015)**: Prompts externalized to `prompts/vision.txt` and `prompts/format.txt`. Structured with Context, Role, Instruction, Specification, Parameters, Examples.
   **Rationale**: Prompts were too long for source code. CRISPE produces robust, situation-agnostic prompts. Editing prompts no longer requires recompilation.

8. **num_ctx=65536**: Increased Ollama context window from default to 65536. Enables reliable multi-image requests.
   **Rationale**: Default context window caused empty responses with 15+ images.

### Performance summary (3.5 min TED-Ed video)

| Stage | First run | Final run |
|-------|-----------|-----------|
| Whisper (chunk_0) | 125s (debug) | 45s (release) |
| Vision (chunk_0) | 648s (Q4) | 639s (Q8 + CRISPE) |
| Total | 883s | 790s |

### Quality improvements
- Vision: spatial storytelling, character tracking, OCR, expression analysis
- Format: scene-based grouping, character identification with confidence, narrator attribution
- Tested on animation (TED-Ed) and live chart analysis (crypto) — both produce high-quality output

### Changes to docs
- Updated CLAUDE.md with CLI usage, key config, Q8 model recommendation
- Updated ARCHITECTURE.md with sequential Whisper→Vision flow
- Updated ROADMAP.md with Phase 5 post-v1 PRs
