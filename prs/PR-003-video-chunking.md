# PR-003: Video Chunking

## Scope

ffmpeg-based video chunking on the server.

- Given an mp4 file and chunk duration (from config), produce a list of `Chunk` structs with start/end timestamps
- Shell out to ffmpeg to split video into chunk files (both video segments and audio segments)
- Extract audio track as separate file per chunk (for Whisper)
- Extract frames at configurable FPS per chunk (for Qwen3-VL)
- Temp directory management for chunk artifacts
- Validate ffmpeg is available (used by `doctor`)

## Dependencies

PR-002

## Verification Criteria

- A sample mp4 is split into the expected number of chunks given a configured duration
- Audio files are extracted for each chunk (WAV format for Whisper compatibility)
- Frames are extracted at the configured FPS (as image files or a format Ollama can accept)
- Chunk start/end timestamps are correct and contiguous
- Works for videos shorter than one chunk (produces a single chunk)
- `doctor` validates ffmpeg is installed and accessible
