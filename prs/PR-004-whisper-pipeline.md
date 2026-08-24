# PR-004: Whisper Pipeline

**Landed-in:** v0.0 (untagged — versioning introduced in PR-019)

## Scope

Whisper-based audio transcription pipeline on the server, running on CPU.

- `WhisperModel` — loads model once (~1.5GB), reused across chunks via `Arc`
- `load_wav_samples` — reads 16kHz mono WAV into f32 samples
- `transcribe` (blocking) — creates WhisperState, runs inference, extracts segments with timestamps
- `transcribe_chunk` (async) — wraps transcribe via `spawn_blocking`
- Classifies output: bracketed tokens like `[MUSIC]` become `SegmentType::Sound`, others become `Speech`
- Timestamps offset from chunk-local centiseconds to absolute video time
- Extended `WhisperConfig` with `n_threads` field

## Dependencies

PR-003

## Verification Criteria

- Classification: speech, sound ([MUSIC], [LAUGHTER]), mixed brackets, empty
- Timestamp offset: chunk-local to global
- WAV loading: synthetic roundtrip via hound, nonexistent file error
- Integration tests (`#[ignore]`): model load, silent WAV transcription, async wrapper
