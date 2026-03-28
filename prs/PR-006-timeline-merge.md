# PR-006: Timeline Merge

## Scope

Merge segments from Whisper and Qwen3-VL pipelines into a unified, sorted JSON output.

- Collect `Vec<Segment>` from both pipelines across all chunks
- Sort all segments by start time
- Deduplicate any overlapping segments from chunk boundaries (if applicable)
- Produce the final `Timeline` struct with metadata (source file, duration)
- Serialize to JSON matching the defined output format
- Write JSON to output path

## Dependencies

PR-004, PR-005

## Verification Criteria

- Segments from both pipelines are present in output
- Segments are sorted by start time
- Output JSON matches the defined schema (source, duration_seconds, segments array)
- Each segment has type, start, end, content fields
- Empty video (no speech, no visual content) produces valid JSON with empty segments array
- JSON is valid and parseable by `serde_json::from_str`
