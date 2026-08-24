# PR-002: Config System

**Landed-in:** v0.0 (untagged — versioning introduced in PR-019)

## Scope

TOML config loading with CLI flag overrides for both client and server.

- Nested config structs: `ClientConfig` (server, output) and `ServerConfig` (server, ffmpeg, whisper, ollama, vision, processing)
- Load from `~/.config/vid-to-text/client.toml` and `server.toml` (separate files for separate machines)
- Generic `load_config::<T>(filename)` with serde defaults for missing fields
- CLI flags override config values
- `validate()` method on each config struct
- `vid-to-text doctor` prints resolved config and file status

## Dependencies

PR-001

## Verification Criteria

- Config loads from TOML file correctly
- Missing config file uses defaults
- CLI flag overrides config value
- Invalid config produces clear error message
- Partial TOML fills defaults for missing fields
- TOML roundtrip serialization works
