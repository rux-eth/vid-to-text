# PR-001: Project Skeleton

## Scope

Set up the Rust workspace with two crates and shared types.

- Cargo workspace with `vtt-client` and `vtt-server` binary crates, plus a `vtt-core` library crate for shared types
- Core types: `Job`, `Chunk`, `Segment`, `Timeline`, `SegmentType` enum
- Basic CLI scaffolding for client (`clap` with subcommands: `process`, `doctor`)
- Basic server binary scaffolding (placeholder HTTP server with `axum`)
- Shared error types
- `.gitignore`, `rust-toolchain.toml`

## Dependencies

None — this is the first PR.

## Verification Criteria

- `cargo build --workspace` succeeds
- `cargo test --workspace` succeeds
- `vtt-client --help` prints usage with subcommands
- `vtt-server --help` prints usage
- Core types can be serialized to/from JSON (`serde` round-trip test)
