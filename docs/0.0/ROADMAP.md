# Roadmap

Ordered list of PRs. Each PR is a single, reviewable change. Dependencies flow downward — no PR should be started before its predecessors are merged.

Full PR descriptions live in `prs/`. This file is the index.

**Status legend:** `[ ]` pending | `[x]` merged | `[~]` in progress

---

## Phase 1: Foundation

| PR | Description | Status | Depends on |
|----|-------------|--------|------------|
| [PR-001](../../prs/PR-001-project-skeleton.md) | Project skeleton — workspace, crates, config types, CLI scaffolding | `[x]` | — |
| [PR-002](../../prs/PR-002-config-system.md) | TOML config loading with CLI flag overrides | `[x]` | PR-001 |

## Phase 2: Server Core

| PR | Description | Status | Depends on |
|----|-------------|--------|------------|
| [PR-003](../../prs/PR-003-video-chunking.md) | ffmpeg video chunking — split mp4 into time-based chunks | `[x]` | PR-002 |
| [PR-004](../../prs/PR-004-whisper-pipeline.md) | Whisper pipeline — audio extraction and speech transcription on CPU | `[x]` | PR-003 |
| [PR-005](../../prs/PR-005-vision-pipeline.md) | Qwen3-VL pipeline — frame extraction and visual description via Ollama | `[x]` | PR-003 |
| [PR-006](../../prs/PR-006-timeline-merge.md) | Timeline merge — combine segments from both pipelines, sort, produce JSON | `[x]` | PR-004, PR-005 |

## Phase 3: Client-Server Communication

| PR | Description | Status | Depends on |
|----|-------------|--------|------------|
| [PR-007](../../prs/PR-007-server-api.md) | Server HTTP API — job submission, status, result retrieval | `[x]` | PR-006 |
| [PR-008+009](../../prs/PR-008-client-and-transfer.md) | Client CLI commands + streaming file transfer (combined) | `[x]` | PR-007 |

## Phase 4: Resilience

| PR | Description | Status | Depends on |
|----|-------------|--------|------------|
| [PR-010](../../prs/PR-010-checkpointing.md) | Chunk-level checkpointing and job resumability | `[x]` | PR-008+009 |

---

## Phase 5: Post-v1 Enhancements

| PR | Description | Status | Depends on |
|----|-------------|--------|------------|
| PR-011 | YouTube URL support via yt-dlp | `[x]` | PR-008+009 |
| PR-012 | Human-readable format command via OpenAI GPT-5.4 | `[x]` | PR-010 |
| PR-013 | Granular visual segments, retry logic, num_ctx, timing logs | `[x]` | PR-012 |
| PR-014 | Overlapped Whisper/Vision with cross-chunk context | `[x]` | PR-013 |
| PR-015 | Externalized prompts (CRISPE framework) | `[~]` | PR-014 |

## Future

- Wake-on-LAN support (`vid-to-text wake`)
- Dedicated audio classifier for `[SOUND]` stream
- Scene-change detection for adaptive frame batching
- SRT/VTT export
- `--prompt` flag for custom format prompts
- `--duration` flag to limit processing to first N seconds

---

## Phase: Process & Config Lock (2026-08)

| PR | Description | Status | Depends on |
|----|-------------|--------|------------|
| [PR-019](../../prs/PR-019-vibe-rails-sync.md) | Sync vibe-rails scaffolding to current template | `[x]` | — |
| [PR-020](../../prs/PR-020-market-research-capture-config.md) | Lock the market-research capture config | `[x]` | PR-019, PR-018 |
| [PR-021](../../prs/PR-021-vision-grounded-asr-correction.md) | Vision-grounded ASR correction | `[ ]` | PR-020 |
| [PR-022](../../prs/PR-022-content-adaptive-frame-sampling.md) | Content-adaptive frame sampling | `[x]` | PR-020 |
| [PR-023](../../prs/PR-023-visual-fidelity-metric.md) | Visual fidelity metric and sampling tune | `[ ]` | PR-022 |
| [PR-024](../../prs/PR-024-ocr-grounded-vision-prompt.md) | OCR-grounded vision prompt | `[ ]` | PR-022, PR-023 |

**Index note:** PR-009 never existed; PR-011..PR-015 are listed above without PR files;
PR-016/PR-017 landed in git history but were never indexed. See
`docs/0.0/RESEARCH-BACKLOG.md` § Index gaps. Reconciling those is a candidate follow-up.

---

## Notes

- Each PR must satisfy the verification criteria in `docs/CONSTRAINTS.md`
- No PR proceeds without user review and approval
- Full PR descriptions in `prs/` directory
