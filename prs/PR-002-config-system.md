# PR-002: Config System

## Scope

TOML config loading with CLI flag overrides for both client and server.

- Config structs for client (server address, output preferences) and server (model settings, chunk duration, ffmpeg path, whisper model path, Ollama endpoint)
- Load from `~/.config/vid-to-text/config.toml` with sensible defaults
- CLI flags override config values (clap + config merge)
- `vid-to-text doctor` checks config file exists and is valid
- Generate default config file if missing (`vid-to-text init` or first run)

## Dependencies

PR-001

## Verification Criteria

- Config loads from TOML file correctly
- Missing config file uses defaults
- CLI flag overrides config value (test: set chunk_duration in file, override with `--chunk-duration`)
- Invalid config produces clear error message
- `doctor` reports config status
