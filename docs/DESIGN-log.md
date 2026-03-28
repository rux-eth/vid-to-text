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
