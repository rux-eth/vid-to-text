# Research Backlog

Index of PRs with research status. Every PR runs `PROCEDURE-pr-research.md` before implementation — this document tracks the state of each PR's research and flags drift risk per the time-decay policy.

**Tiers (PR-path):**
- **Tier 1** — Design-time research exists. Phase 1 (State Assessment) required before implementation; Phases 2-4 may be light if no drift found.
- **Tier 2** — Design-time research is partial or absent. Full procedure required before the PR can be written in final form.

**Status legend:**
- `design-research ✓` — research done at design time
- `design-research ~` — partial design research
- `design-research ✗` — no design-time research
- `state-assessed YYYY-MM-DD` — Phase 1 completed
- `fully-researched YYYY-MM-DD` — all 5 phases completed
- `implementation-cleared YYYY-MM-DD` — Phase 5 Gate Check passed

---

## Pre-procedure PRs (landed before `PROCEDURE-pr-research.md` existed)

These shipped under an older `vibe-rails` revision that had no PR-research gate. They are recorded for completeness. **The gate applies going forward; these are not retroactively backfilled** (explicit scope decision in PR-019).

| PR | Description | Status | File |
|----|-------------|--------|------|
| PR-001 | Project skeleton | landed v0.0 | ✓ |
| PR-002 | Config system | landed v0.0 | ✓ |
| PR-003 | Video chunking | landed v0.0 | ✓ |
| PR-004 | Whisper pipeline | landed v0.0 | ✓ |
| PR-005 | Vision pipeline | landed v0.0 | ✓ |
| PR-006 | Timeline merge | landed v0.0 | ✓ |
| PR-007 | Server API | landed v0.0 | ✓ |
| PR-008 | Client and transfer | landed v0.0 | ✓ |
| PR-010 | Checkpointing | landed v0.0 | ✓ |

## Index gaps (recorded, not fabricated)

Reconciling `prs/`, `docs/0.0/ROADMAP.md`, and git history surfaced inconsistencies. These are recorded as found. **No PR files were invented to fill them.**

| PR | In ROADMAP | File in `prs/` | In git history | Note |
|----|-----------|----------------|----------------|------|
| PR-009 | no | no | no | Number skipped. Referenced only as a dependency (`PR-008+009`) by PR-011's ROADMAP row. Never existed. |
| PR-011 | `[x]` | **no** | `ee31c77` (YouTube URL support) | Landed without a PR file. |
| PR-012 | `[x]` | **no** | — | Landed without a PR file (format command). |
| PR-013 | `[x]` | **no** | — | Landed without a PR file (granular visual segments, retry, num_ctx). |
| PR-014 | `[x]` | **no** | — | Landed without a PR file (overlapped Whisper/Vision). |
| PR-015 | `[~]` | **no** | — | Marked in-progress in ROADMAP; no file. Status unverified. |
| PR-016 | **no** | **no** | `a822525` (merge #19, `pr-016-cache-system`) | Landed but indexed nowhere. |
| PR-017 | **no** | **no** | `24a52c8` (merge #20, `pr-017-config-profiles`) | Landed but indexed nowhere. |

Closing these gaps is **not** in PR-019's scope ("one PR, one thing"). It is a candidate follow-up.

---

## Tier 1 — Implementation-Ready (pending state assessment)

| PR | Design research | State assessed | Implementation cleared |
|----|-----------------|----------------|------------------------|
| [PR-019](../../prs/PR-019-vibe-rails-sync.md) | `design-research ✓` (template is the project's own methodology SSOT) | `state-assessed 2026-08-24` | `implementation-cleared 2026-08-24` |

## Tier 2 — Research-Pending

| PR | Design research | Required research topics |
|----|-----------------|--------------------------|
| [PR-018](../../prs/PR-018-causal-vision-context.md) | `design-research ✗` | **Non-conforming — see below.** Retrofit to `PR-TEMPLATE.md` and backfill research: (a) look-ahead contamination in LLM-derived corpora, (b) whether audio-conditioned visual descriptions are defensible for exploratory research use, (c) evaluation method for the fix. |
| PR-020 (not yet drafted) | `design-research ✗` | Lock the market-research capture config: fps / sampling rate, whisper decoding settings, transcript windowing, determinism, and **researched evaluation metrics** for corpus value in exploratory (non-training) use. Includes an empirical validation phase. Depends on PR-019. |

### PR-018 non-conformance (recorded)

PR-018 was written **and implemented** on 2026-08-24 without the `Landed-in:`, `Before Implementation`, or `Research findings` sections, and without running `PROCEDURE-pr-research.md` at all. This violates the Research-Backed Decisions and PR Research Procedure Required constraints that PR-019 installs.

Its code is implemented and covered by 8 tests (transcript windowing modes, per-batch prompt regression guard, dialogue gating). The evaluation metrics used to validate it (duplicated 8-gram rate, audio-citation rate, recall stratified by on-screen persistence) were **invented ad hoc, not research-backed**, and are labeled `best-guess-given-constraints`.

Agreed handling: PR-018 supplies the *mechanism* (the config knobs); PR-020 researches and locks the *values*. PR-018 is retrofitted to the template and its research backfilled as its own work item.

---

## Drift Watch

Per the time-decay policy in `PROCEDURE-pr-research.md`, any PR marked `fully-researched` or `state-assessed` more than the staleness threshold before implementation begins must re-run Phase 1.

**Project staleness threshold:** 30 days (set in `docs/CONSTRAINTS.md` § Research Time-Decay).

**Currently watching:** none — PR-019 was state-assessed and implemented the same day.

## Known documentation gaps

- **`docs/VERSIONING.md` bump rules are unset.** The file ships with `[Project-specific — ...]` placeholders for MAJOR/MINOR triggers and v1.0 criteria. Deferred by explicit user decision during PR-019 (2026-08-24). Until set, minor-vs-patch classification is undefined for this project.
- **No version tags exist.** Landed PRs are attributed `v0.0 (untagged)`. The first real cut should establish the tag per `docs/VERSIONING.md` §7.

- **`CLAUDE.md` § Remote server access is stale.** It states "the server is launched manually and foreground when needed — there is no systemd unit, launch script, or persistent log file." As of 2026-08-24 all three exist on the desktop: a `vtt-server.service` systemd unit (`Restart=always`, enabled at boot), `~/vid-to-text/start-vtt-server.sh`, and timestamped logs under `~/.vid-to-text/logs/`. These were installed while deploying an unrelated UTF-8 panic fix. Correcting this section is **out of PR-019's scope** ("one PR, one thing") and needs its own work item — it is a project-doc accuracy fix, not template scaffolding.

## How to update this document

- After Phase 4 (Docs) of a design session, add each new PR as a row with its design-research status
- After running `PROCEDURE-pr-research.md` Phase 1 for a PR, update `State assessed`
- After completing all 5 phases, update `Implementation cleared`
- If state assessment surfaces blocking drift, move the PR back to Tier 2 and document required research
- New PRs start as Tier 2 unless they explicitly inherit prior research
