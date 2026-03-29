# PR-006: Timeline Merge

## Scope

Pipeline orchestration and timeline merge — combines all processing stages.

- `process_video(config, video_path, job_id, force)` — full orchestrator: prepare_chunks → Whisper → Vision → merge
- `merge_segments(source, duration, chunk_segments)` — flattens all segments, sorts by start timestamp
- `extract_transcript(segments)` — extracts speech content for vision context
- Per-chunk processing is sequential: Whisper first (CPU), then Vision with transcript (GPU)
- One `[VISUAL]` segment per chunk, multiple `[SPEECH]`/`[SOUND]` segments per chunk

## Dependencies

PR-004, PR-005

## Verification Criteria

- `merge_segments`: empty, single, multiple types, interleaved, same-start, large count, JSON roundtrip
- `extract_transcript`: empty, only sound/visual, single/multiple speech, mixed types
- Integration test (`#[ignore]`): end-to-end with synthetic ffmpeg video
