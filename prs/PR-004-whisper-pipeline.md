# PR-004: Whisper Pipeline

## Scope

Whisper-based audio transcription pipeline on the server, running on CPU.

- Integrate `whisper-rs` (Rust bindings for whisper.cpp)
- Implement the `Pipeline` trait for Whisper: takes a `Chunk` (with extracted audio), returns `Vec<Segment>`
- Produce `[SPEECH]` segments with start/end timestamps and transcribed text
- Capture Whisper's non-speech tags (`[MUSIC]`, `[LAUGHTER]`, etc.) as `[SOUND]` segments
- Force CPU-only inference (no GPU)
- Configurable: model size (tiny/base/small/medium/large), language
- Validate whisper model file is available (used by `doctor`)

## Dependencies

PR-003

## Verification Criteria

- A chunk with spoken audio produces `[SPEECH]` segments with correct timestamps (relative to original video, not chunk-local)
- Non-speech audio tags from Whisper are captured as `[SOUND]` segments
- Silent audio produces no segments
- Timestamps are adjusted to be relative to the full video (chunk offset applied)
- Runs on CPU only — GPU VRAM usage does not increase
- `doctor` validates whisper model is downloaded
