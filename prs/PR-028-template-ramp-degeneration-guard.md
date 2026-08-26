# PR-028: Template-ramp degeneration guard

<!-- Landed-in: set to the released version this PR shipped under (e.g. v0.1.0).
     Use "(not yet landed)" for in-flight or dormant PRs.
     Use "superseded by PR-XXX" for replaced PRs.
     See docs/VERSIONING.md §4 for the policy. -->
**Landed-in:** (not yet landed)

**Path tier:** Tier-1 (the failure is measured in-repo with a reproducing case; the threshold is not
yet set, and setting it is the research)

## Before Implementation (NON-NEGOTIABLE)

This PR MUST NOT be implemented until `PROCEDURE-pr-research.md` has been completed in full and its
output appended to the `## Research findings` section below.

**Tier-1 PRs** (research-backed at design time): Phase 1 (State Assessment) is required to catch
drift. Phases 2-4 may be light if no drift is found.

**Tier-2 PRs** (research-pending): all 5 phases of `PROCEDURE-pr-research.md` must run before this PR
is written in final form.

Skipping the PR research procedure is a hard violation of the research-backed-decisions constraint in
`docs/CONSTRAINTS.md`.

## Research findings

_To be populated by `PROCEDURE-pr-research.md`. The reproducing case below is measurement, not the
research round: the detector's shape and its threshold are what the procedure must settle._

---

## Motivation

**A third degeneration mode exists that both shipped guards are structurally blind to.** Found
2026-08-25 while measuring a vision-prompt revision (PR-026), on job
`2fc10c93-ec66-4602-8259-ee016ee0de1e`, clip `2024_4_8_5-20`, visual segment 6:

> "A horizontal line is drawn at 38,720. A horizontal line is drawn at 33,907. A horizontal line is
> drawn at 32,276. A horizontal line is drawn at 31,145. A horizontal line is drawn at 30,000. A
> horizontal line is drawn at 29,000. … A horizontal line is drawn at 16,000. …"

It marches down in round steps and runs past zero into **negative prices** (`-2,000` … `-16,000`) for
an asset trading near 70,000. That one segment stated **284 facts of which 239 are unsupported**,
dragging whole-video precision from **0.926 to 0.529**. Excluding it, the same run scores 0.915.

**Why each existing guard misses it, structurally rather than by tuning:**

- **`truncate_numeric_run`** (PR-025, `vision.max_numeric_run = 40`) counts *consecutive numeric
  tokens*. Here the longest consecutive run is **2** — every number is separated by prose. Raising the
  cap cannot help; the signal is absent.
- **`truncate_repetition`** (from `6f10acd`) cuts where a sentence of ≥15 characters recurs a third
  time. Here **every sentence is unique**, because the number differs. Verified live in the same job's
  log: the guard fired on *other* batches (`truncated from 21063 to 1284 chars`), so it is working —
  it simply cannot see this shape.

So the mode is a **repeated sentence template with a varying numeric slot**. PR-025 documented two
modes — an arithmetic ramp of bare numbers, and one value repeated — and this is a third that defeats
both detectors by construction.

**It is prompt-inducible, which raises the priority.** The PR-026 prompt that triggered it contained
the example `Say "a line is drawn at 71,700"` — handing the model a canned sentence form for an
enumerable feature. PR-026 removed that template, which addresses *this* trigger, but any future
prompt that supplies a sentence pattern for something countable can re-trigger it, and so can content
with many genuinely drawn levels. A prompt fix is not a guard.

## Scope

**In scope:**
- A detector for a repeated sentence **skeleton** — the sentence with its numeric tokens masked —
  recurring more than a threshold number of times within one visual segment, truncating at the cap
  and keeping the legitimate head, exactly as the two existing guards do.
- The threshold, **measured** the way PR-025's was: over every visual segment on disk, with the
  implementation's own tokenizer, reporting where legitimate runs top out and where degenerate ones
  begin, and choosing a value in the gap. PR-025's near-miss (a wrong tokenizer suggested a cap of 24
  against a true 40) is the pattern to avoid.
- Config key under `[vision]` alongside `max_numeric_run`, with `0` disabling.
- A log line per truncation stating the observed repeat count, matching existing guard behaviour.

**Explicitly out of scope:**
- Changing `truncate_repetition` or `truncate_numeric_run`. Both work on what they were built for;
  this is a third detector, not a replacement.
- Retrying generation. `ollama.temperature = 0` means the existing retry loop reproduces the same
  output — PR-025 established this.
- Editing after merge (forbidden by Segments Are Immutable After Merge). The guard runs at generation
  time, where the other two already live.
- A general-purpose repetition metric. Compression ratio was measured and rejected as a degeneration
  detector in PR-025; re-opening it needs its own justification.

## Dependencies

- **PR-025** — the existing numeric-run guard and the measurement method for setting its threshold.
  Landed `f45ce72`.
- **PR-023** — the fidelity diagnostic, which is how this class becomes visible at all. Landed `f570aec`.

## Architecture section implemented

`docs/ARCHITECTURE.md` — the **Vision output guards** subsection of § Fidelity Diagnostic, which
currently documents two guards and will document three.

## Verification criteria

- [ ] The reproducing case (job `2fc10c93`, segment 6) is truncated, and its whole-video precision
      recovers toward the 0.915 the rest of that run scores
- [ ] Skeleton extraction masks numeric tokens and is pinned by tests, including the case where the
      varying slot is a percentage or a suffixed value (`1.738T`, `-0.70%`)
- [ ] Threshold measured over every visual segment on disk, with the observed separation between
      legitimate and degenerate runs recorded here, and the tokenizer used stated explicitly
- [ ] A legitimate list of drawn levels — the head of the reproducing segment is one — survives intact
- [ ] Truncations are logged with the observed repeat count
- [ ] `0` disables the guard; default is the measured value
- [ ] `cargo test --workspace` passes

## Research backing

**Tier-1.** The failure, the reproducing case and the structural blindness of both existing guards are
measured in-repo (see Motivation) and need no external round. What the procedure must settle is the
detector's shape and its threshold:

1. Is a masked-skeleton match the right detector, or does it false-positive on legitimate structured
   description? Candidate counter-case: a segment that legitimately reports six drawn levels in six
   sentences.
2. What separation exists between legitimate and degenerate skeleton-repeat counts on this corpus?
   PR-025's method — measure over every segment on disk with the implementation's own tokenizer —
   transfers directly.
3. Does the guard interact with `truncate_repetition`? Both edit the same text at generation time and
   the order of application must be decided rather than inherited.

## Notes

- Found by PR-026's A/B, which is a point in favour of running prompt changes through the fidelity
  diagnostic even when it is used only as a guardrail: a metric with no calibration still surfaced a
  correctness bug that produced negative Bitcoin prices.
- The v3 run that produced the case was **cancelled mid-flight by user direction** once the defect was
  understood, so only `2024_4_8` has a v3 timeline; `2024_6_24` was cancelled and `2025_05_26` never
  started. The reproducing job is preserved and named above.
- Worth checking during implementation whether the March 2026 pre-PR-020 timelines contain this mode
  too. If they do, it predates every guard and the corpus-wide rate is larger than one segment.
