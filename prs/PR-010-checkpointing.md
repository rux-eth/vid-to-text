# PR-010: Checkpointing and Resumability

## Scope

Chunk-level checkpointing so jobs can resume after failure.

- After each chunk is fully processed (both pipelines complete), write a checkpoint file to disk
- Checkpoint contains the chunk's segments as JSON, keyed by chunk index
- On job start, check for existing checkpoints and skip already-completed chunks
- `--force` flag to ignore checkpoints and reprocess from scratch
- Checkpoint directory: configurable, default `/tmp/vtt-jobs/<job-id>/`
- Cleanup: remove checkpoint directory after successful job completion
- Job ID is deterministic (derived from file hash + config) so the same file resumes correctly

## Dependencies

PR-009

## Verification Criteria

- Processing a video creates checkpoint files per chunk
- Killing the server mid-job and restarting resumes from the last completed chunk
- `--force` flag causes full reprocessing even with existing checkpoints
- Completed job cleans up checkpoint directory
- Same file submitted twice (without changes) reuses checkpoints
- Different config (e.g., different chunk duration) does not reuse stale checkpoints
