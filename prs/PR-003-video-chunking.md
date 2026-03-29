# PR-003: Video Chunking

## Scope

ffmpeg-based video chunking on the server.

- `compute_chunks(duration, chunk_duration_secs)` — pure math for chunk boundaries
- `probe_duration(config, video_path)` — gets video length via ffprobe JSON output
- `extract_audio(config, video_path, chunk, output_dir)` — extracts WAV (16kHz mono) per chunk
- `extract_frames(config, vision_config, video_path, chunk, output_dir)` — extracts JPEG/PNG frames at configurable FPS
- `check_ffmpeg(config)` — version check for doctor command
- `prepare_chunks(config, video_path, job_id)` — orchestrates full job extraction
- Extended `FfmpegConfig` with `frame_format` and `frame_quality` fields
- `ffprobe_path()` helper to derive ffprobe binary path

## Dependencies

PR-002

## Verification Criteria

- `compute_chunks` correctly handles: short video, exact multiple, remainder, zero duration, long video
- Command argument builders produce correct ffmpeg flags
- Integration tests (`#[ignore]`): probe duration, extract audio/frames, full prepare_chunks
