# PR-019: Sync vibe-rails scaffolding

<!-- Landed-in: set to the released version this PR shipped under (e.g. v0.1.0).
     Use "(not yet landed)" for in-flight or dormant PRs.
     Use "superseded by PR-XXX" for replaced PRs.
     See docs/VERSIONING.md §4 for the policy. -->
**Landed-in:** (not yet landed)

**Path tier:** Tier-1 (light path — Phase 1 State Assessment is the requirement)

## Before Implementation (NON-NEGOTIABLE)

This PR MUST NOT be implemented until `PROCEDURE-pr-research.md` has been completed in full and its output appended to the `## Research findings` section below.

**Tier-1 PRs** (research-backed at design time): Phase 1 (State Assessment) is required to catch drift. Phases 2-4 may be light if no drift is found.

**Tier-2 PRs** (research-pending): all 5 phases of `PROCEDURE-pr-research.md` must run before this PR is written in final form.

Skipping the PR research procedure is a hard violation of the research-backed-decisions constraint in `docs/CONSTRAINTS.md`.

## Research findings

### State Assessment (2026-08-24)

**Current state**:
- Repo was created from a `vibe-rails` revision predating `PROCEDURE-pr-research.md`, the hybrid doc layout, and three structural constraints.
- Docs are flat: `docs/{ARCHITECTURE,CONSTRAINTS,DESIGN-log,ROADMAP}.md`. No `docs/0.0/`.
- `docs/CONSTRAINTS.md` has 4 of 7 template structural constraints (No Phantom Implementations, Documentation Accuracy, One PR One Thing, Config Over Hardcoding) plus **5 project-specific domain constraints** that exist only here.
- `prs/` holds 10 files: PR-001..008, PR-010, PR-018.
- 14 commits have touched `docs/`; the last doc-affecting commit is `f70e5a5`.

**Assumptions at PR draft time**:
- That the only gap was *missing* files, and the fix was a mechanical import plus a move.
- That `PROCEDURE-design-planning.md` and `PROCEDURE-code-audit.md` were current, since they already exist here.
- That `RESEARCH-BACKLOG.md` could index "every existing PR" straightforwardly.

**Stale assumptions** (current state disagrees with the draft):
1. **Both existing PROCEDURE files are stale, not current.** `PROCEDURE-design-planning.md` differs from the template by 64 lines (the template's Phase 2 is "Decisions (**with research**)" and it adds the version-cut trigger). `PROCEDURE-code-audit.md` differs by 2 lines (`docs/ROADMAP.md` -> `docs/0.0/ROADMAP.md`). The draft scoped them as untouched. **Scope amended** to re-import both.
2. **The PR index is internally inconsistent**, so "index every existing PR" is not mechanical:
   - `docs/ROADMAP.md` references PR-011..PR-015, which have **no file** in `prs/`.
   - PR-016 and PR-017 appear in git history as landed branches (`pr-016-cache-system`, `pr-017-config-profiles`) but appear in **neither** `ROADMAP.md` nor `prs/`.
   - PR-018 has a file but is absent from `ROADMAP.md`.
   - PR-009 does not exist anywhere (number skipped).
   `RESEARCH-BACKLOG.md` therefore cannot index from `prs/` alone; it must be reconciled against `ROADMAP.md` **and** git history, and the gaps recorded rather than invented.
3. **Cross-reference migration is wider than the two moved files.** 6 references to the flat paths exist across `CLAUDE.md` (3) and the two PROCEDURE files (3). Re-importing the PROCEDURE files fixes their 3 automatically; `CLAUDE.md`'s 3 need editing. `docs/ROADMAP.md` additionally contains 9 internal `../` links that shift one level deeper on the move (`../prs/` -> `../../prs/`); `docs/DESIGN-log.md` contains none.

**New constraints** (learned from the codebase):
- `docs/CONSTRAINTS.md` is **SSOT and must be merged, not overwritten**. It carries 5 domain constraints (No Audio Data on GPU, mp4 Input Only, Segments Immutable After Merge, Checkpoint Integrity, No Network Calls From Client to Models) that exist nowhere else. Copying the template file over it would destroy locked project decisions.
- Same for `CLAUDE.md`: it holds project-specific sections (CLI Usage, Remote server access, Key Configuration) absent from the template. Merge, do not replace.
- Per `docs/VERSIONING.md` §5, the move should be split into a rename-only commit plus a content-fixup commit if any moved file's internal links drop below git's 50% rename-similarity threshold, so `git log --follow` still traces it.

**Downstream contracts**:
- **none currently** — `grep -rl "PR-019" prs/ docs/` returns no file other than PR-019 itself (verified 2026-08-24).
- **PR-020 (config lock) will depend on this PR** by declaration in this session: it is specified as Tier-2, and Tier-2 requires `PROCEDURE-pr-research.md`, which this PR imports. Contract: PR-019 must deliver `PROCEDURE-pr-research.md` and `prs/PR-TEMPLATE.md` before PR-020 can be written in final form. **Current scope satisfies this.**
- **PR-018 (retrofit)** will likewise consume `prs/PR-TEMPLATE.md`. **Current scope satisfies this.**

**Path-tier checkpoint**:
- Assignment in this PR's header: **Tier-1**. `docs/0.0/ROADMAP.md` does not yet exist, so there is no roadmap entry to disagree with; no cut-plan exists.
- Drift found is **mechanical and scope-affecting, not premise-changing**: no downstream contract is unsatisfiable, and no must-answer question requiring external evidence was surfaced. The imported artifacts are the project's own methodology SSOT, not a claim about the outside world.
- **Tier-1 holds.** Phases 2-4 do not run. Proceed to Phase 5 Gate Check after scope amendment.

**Scope amendments arising from Phase 1** (folded into Scope above):
- Add: re-import `PROCEDURE-design-planning.md` and `PROCEDURE-code-audit.md` (both stale).
- Add: reconcile `RESEARCH-BACKLOG.md` against ROADMAP + git history, recording PR-009/011..017 gaps explicitly rather than fabricating entries.
- Clarify: `docs/CONSTRAINTS.md` and `CLAUDE.md` are **merged**, not overwritten.

### Gate Check (2026-08-24)

- Premise still valid: ✓ — Phase 1 found only mechanical drift; the PR's purpose (bring scaffolding to current template) is unchanged.
- No prerequisite PRs surfaced: ✓ — this PR has no upstream dependencies and is itself the prerequisite for PR-020 and the PR-018 retrofit.
- User approved updated spec: ✓ (2026-08-24) — scope amendments from Phase 1 approved; `docs/VERSIONING.md` bracketed bump rules explicitly deferred by user decision.
- Implementation cleared: ✓

**Deferred by user decision (recorded, not invented):**
- `docs/VERSIONING.md` ships with `[Project-specific — ...]` placeholders for MAJOR/MINOR triggers and v1.0 criteria. The user deferred setting these. The file is imported verbatim with placeholders intact; `docs/0.0/RESEARCH-BACKLOG.md` records the gap.

---

## Scope

Bring this repo's scaffolding up to the current `vibe-rails` template (`~/templates/vibe-rails`). This repo was created from an older revision that predates the PR-research procedure, the hybrid doc layout, and several structural constraints.

**Import (new files):**
- `PROCEDURE-pr-research.md`
- `prs/PR-TEMPLATE.md`
- `docs/CONVENTIONS.md`, `docs/DEPLOYMENT.md`, `docs/VERSIONING.md`
- `CHANGELOG.md`

**Re-import (existing but stale — see Phase 1):**
- `PROCEDURE-design-planning.md` (64 lines behind; template Phase 2 is "Decisions (with research)")
- `PROCEDURE-code-audit.md` (2 lines behind; versioned ROADMAP path)

**Restructure to the hybrid doc layout** (`docs/VERSIONING.md` §3):
- Move `docs/DESIGN-log.md` → `docs/0.0/DESIGN-log.md`
- Move `docs/ROADMAP.md` → `docs/0.0/ROADMAP.md`
- Create `docs/0.0/RESEARCH-BACKLOG.md`
- Keep `ARCHITECTURE.md`, `CONSTRAINTS.md` flat (SSOT)
- Migrate cross-references to the moved paths (`docs/VERSIONING.md` §5)

**Update in place:**
- `docs/CONSTRAINTS.md` — **merge, do not overwrite** (it holds 5 project-only domain constraints). Add the structural constraints absent from this repo's older copy: Research-Backed Decisions, PR Research Procedure Required, Per-Phase Approval Gate, Config Over Hardcoding, No Phantom Implementations, Documentation Accuracy, One PR One Thing. Set the project staleness threshold.
- `CLAUDE.md` — **merge, do not overwrite** (it holds project-only CLI/remote/config sections). Add the Ongoing Behavior (MANDATORY) block and Design References pointing at the new layout.
- Existing `prs/PR-0*.md` — add the `Landed-in:` header field.

**Explicitly out of scope:**
- Retroactively backfilling `## Research findings` for already-landed PRs (PR-001..PR-010). They shipped under the older procedure; the gate applies going forward.
- PR-018's non-conformance (see Notes) — handled separately, not by this PR.
- Any change to `vtt-core` / `vtt-server` / `vtt-client` source. This PR is docs and process only.

## Dependencies

None.

## Architecture section implemented

None — this PR is process scaffolding, not system architecture. It establishes the procedure that governs subsequent PRs (notably PR-020).

## Verification criteria

- [ ] Every file listed under "Import" exists in this repo and matches the template's current content
- [ ] `docs/0.0/{DESIGN-log,ROADMAP,RESEARCH-BACKLOG}.md` exist; `docs/{DESIGN-log,ROADMAP}.md` no longer exist at the flat path
- [ ] No dangling references to the pre-move paths: `grep -rn "docs/ROADMAP.md\|docs/DESIGN-log.md" . --exclude-dir=.git` returns only matches inside `docs/VERSIONING.md`'s migration table
- [ ] Moved files' internal relative links resolve (`../prs/` → `../../prs/`)
- [ ] `docs/CONSTRAINTS.md` contains all seven structural constraints listed in scope, and a stated staleness threshold
- [ ] `CLAUDE.md` contains the Ongoing Behavior (MANDATORY) block referencing `PROCEDURE-pr-research.md`
- [ ] Every file in `prs/` matching `PR-0*.md` has a `Landed-in:` header
- [ ] `docs/0.0/RESEARCH-BACKLOG.md` indexes every existing PR with a tier and status
- [ ] `cargo test --workspace` still passes (proves no source was touched)

## Research backing

Tier-1. The scaffolding being imported is the `vibe-rails` template itself, which is the project's own methodology SSOT — the "research" is a mechanical diff against `~/templates/vibe-rails`, not an external-evidence question. Phase 1 State Assessment establishes what has drifted between this repo's copy and the template.

No must-answer questions are expected. If Phase 1 surfaces one (e.g. the template's layout conflicts with an existing locked decision in this repo's `ARCHITECTURE.md`), the PR re-tiers to Tier-2 per `PROCEDURE-pr-research.md` Phase 1's path-tier checkpoint.

## Notes

- **PR-018 is non-conforming.** It was written and implemented earlier the same day without the `Landed-in:`, `Before Implementation`, or `Research findings` sections, and without running `PROCEDURE-pr-research.md` at all — a violation of the research-backed-decisions constraint that this PR is installing. Its code (causal transcript windowing, `use_transcript`) is implemented and covered by 8 tests. Recommended handling: PR-018 supplies the *mechanism*; PR-020 researches and locks the *values*. PR-018 should be retrofitted to the template and its research backfilled, tracked as its own work item rather than absorbed here ("one PR, one thing").
- The template's `docs/0.0/` layout assumes version `0.0`. This repo has no `CHANGELOG.md` and no version tags; it starts at `0.0` by default per the template.
- `docs/VERSIONING.md` ships with `[Project-specific — ...]` bracketed sections. Filling those in is a design decision (what triggers MINOR vs PATCH here), not a mechanical import. This PR imports the file; the bracketed rules are left for the user to set, and that gap is recorded in Phase 1 output rather than invented.
