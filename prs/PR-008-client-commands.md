# PR-008: Client CLI Commands

## Scope

Full client CLI implementation.

- `vid-to-text process <file.mp4>` — send single file to server, poll for completion, write output JSON
- `vid-to-text process <directory>` — find all mp4 files in directory, process each sequentially
- `vid-to-text doctor` — full dependency and connectivity check (server reachable, health endpoint, config valid)
- Progress reporting: show which file is processing, chunk progress if available
- Output path: default alongside input, `--output` override
- Configurable: server address (from config or `--server` flag)

## Dependencies

PR-007

## Verification Criteria

- Single file processing: sends mp4, polls, writes JSON next to input
- Directory processing: finds all mp4s, processes each, writes JSON for each
- `--output` flag overrides output location
- `doctor` reports server connectivity, model availability, config status
- Non-existent file produces clear error
- Non-mp4 file produces clear error
- Server unreachable produces clear error with helpful message
