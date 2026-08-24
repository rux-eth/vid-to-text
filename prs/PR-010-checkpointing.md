# PR-010: Checkpointing and Resumability

**Landed-in:** v0.0 (untagged — versioning introduced in PR-019)

## Scope

Chunk-level checkpointing so jobs can resume after failure.

- `save_checkpoint` — atomic write (`.tmp` then rename) of `Vec<Segment>` per chunk
- `load_checkpoints` — reads all `chunk_NNN.json` files, ignores `.tmp` (crash-safe)
- `clear_checkpoints` — removes checkpoint directory
- Checkpoint path: `{temp_dir}/{job_id}/checkpoints/chunk_NNN.json`
- Pipeline loads checkpoints on start, skips cached chunks, saves after each new chunk
- Skips model initialization when all chunks are cached
- Cleans up checkpoints after successful merge (configurable: `cleanup_checkpoints`)
- `--force` flag wired end-to-end: client CLI → multipart form → server → pipeline

## Dependencies

PR-008+009

## Verification Criteria

- Save/load roundtrip
- Multiple chunks saved and loaded correctly
- `.tmp` files ignored during load (crash safety)
- Clear removes checkpoint directory
- Atomic write: final file exists, .tmp does not
- `--force` bypasses existing checkpoints
