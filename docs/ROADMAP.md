# Roadmap

Ordered list of PRs. Each PR is a single, reviewable change. Dependencies flow downward — no PR should be started before its predecessors are merged.

Full PR descriptions live in `prs/`. This file is the index.

**Status legend:** `[ ]` pending | `[x]` merged | `[~]` in progress

---

## Phase 1: Foundation

| PR | Description | Status | Depends on |
|----|-------------|--------|------------|
| [PR-001](../prs/PR-001-project-skeleton.md) | Project skeleton — workspace, crates, config types, CLI scaffolding | `[x]` | — |
| PR-002 | TOML config loading with CLI flag overrides | `[x]` | PR-001 |

## Phase 2: Server Core

| PR | Description | Status | Depends on |
|----|-------------|--------|------------|
| PR-003 | ffmpeg video chunking — split mp4 into time-based chunks | `[x]` | PR-002 |
| PR-004 | Whisper pipeline — audio extraction and speech transcription on CPU | `[x]` | PR-003 |
| PR-005 | Qwen3-VL pipeline — frame extraction and visual description via Ollama | `[x]` | PR-003 |
| PR-006 | Timeline merge — combine segments from both pipelines, sort, produce JSON | `[x]` | PR-004, PR-005 |

## Phase 3: Client-Server Communication

| PR | Description | Status | Depends on |
|----|-------------|--------|------------|
| [PR-007](../prs/PR-007-server-api.md) | Server HTTP API — job submission, status, result retrieval | `[ ]` | PR-006 |
| [PR-008](../prs/PR-008-client-commands.md) | Client CLI commands — process (single + directory), doctor | `[ ]` | PR-007 |
| [PR-009](../prs/PR-009-file-transfer.md) | File transfer — client uploads mp4 to server, downloads JSON result | `[ ]` | PR-008 |

## Phase 4: Resilience

| PR | Description | Status | Depends on |
|----|-------------|--------|------------|
| [PR-010](../prs/PR-010-checkpointing.md) | Chunk-level checkpointing and job resumability | `[ ]` | PR-009 |

---

## Future (post-v1)

- Wake-on-LAN support (`vid-to-text wake`)
- YouTube URL support (`--url`)
- Dedicated audio classifier for `[SOUND]` stream
- Human-readable output layer
- SRT/VTT export

---

## Notes

- Each PR must satisfy the verification criteria in `docs/CONSTRAINTS.md`
- No PR proceeds without user review and approval
- Full PR descriptions in `prs/` directory
