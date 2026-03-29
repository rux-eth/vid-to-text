# CLAUDE.md

<!-- STATUS: initialized -->

## What Is vid-to-text

A Rust client/server tool that converts mp4 videos into structured JSON combining timestamped speech transcription (`[SPEECH]`), visual scene descriptions (`[VISUAL]`), and sound event tags (`[SOUND]`). The CLI client runs on the user's laptop and dispatches jobs to a server on a desktop with an RTX 4090, which runs Whisper (CPU) and Qwen3-VL-8B-Thinking (GPU via Ollama) sequentially per chunk — Whisper first, then Vision with the transcript as context.

## Build & Test Commands

```bash
cargo build --workspace        # Build all crates
cargo test --workspace         # Run all tests
cargo run -p vtt-client -- --help   # Client CLI help
cargo run -p vtt-server -- --help   # Server help
```

## Architecture

Client/server split: `vtt-client` (laptop) sends mp4 files over HTTP to `vtt-server` (desktop with GPU). Server chunks video via ffmpeg, then for each chunk: runs Whisper on CPU first, passes the transcript to Qwen3-VL on GPU for context-aware visual description. All segments are merged into a sorted JSON timeline and returned to the client.

See `docs/ARCHITECTURE.md` for the full architecture reference.

## Implementation Status

See `docs/ROADMAP.md` for the ordered PR plan. Full PR descriptions in `prs/`.

## Design References

- `docs/ARCHITECTURE.md` — canonical architecture reference
- `docs/CONSTRAINTS.md` — hard rules
- `docs/ROADMAP.md` — PR index with phases and dependencies
- `prs/` — full PR descriptions
- `docs/DESIGN-log.md` — design conversation log
- `PROCEDURE-design-planning.md` — how to run design sessions
- `PROCEDURE-code-audit.md` — post-design-session code audit

## Constraints

- **No phantom implementations** — if it's not tested, it's not done
- **Documentation accuracy** — every code change includes doc updates in the same commit
- **One PR, one thing** — no scope creep
- **Config over hardcoding** — zero hardcoded parameters, all values from TOML config
- **No audio on GPU (v1)** — Whisper on CPU only, full VRAM for Qwen3-VL
- **mp4 only (v1)** — no format conversion
- **Segments immutable after merge** — output is faithful to model output
- **Checkpoint integrity** — only fully-processed chunks are checkpointed
- **Client never talks to models directly** — all model interaction through the server
