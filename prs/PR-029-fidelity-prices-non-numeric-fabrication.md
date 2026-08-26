# PR-029: Fidelity diagnostic prices non-numeric fabrication

<!-- Landed-in: set to the released version this PR shipped under (e.g. v0.1.0).
     Use "(not yet landed)" for in-flight or dormant PRs.
     Use "superseded by PR-XXX" for replaced PRs.
     See docs/VERSIONING.md §4 for the policy. -->
**Landed-in:** (not yet landed)

**Path tier:** Tier-2 (what counts as a scored "fact" is an open design question with no in-repo
answer; the choice moves every historical precision figure and breaks comparability with every run
already recorded, so it needs the full path)

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

_To be populated by `PROCEDURE-pr-research.md`. Do not begin implementation until this section exists
with completed findings from all required phases._

---

## Motivation

**The metric we steer by cannot see the failure that costs the most output.** `score_segments`
extracts numbers, labels, tickers and timeframes. Text that fabricates none of those is unscored, so a
segment can be almost entirely invented and still price at ~1.0.

Measured three times, on three different jobs:

| job | what the segment does | chars | scored facts | segment precision |
|---|---|---|---|---|
| `84149f3b` seg 109 | 144 fabricated trend lines, circled-glyph slot | 15,905 | **25** | 0.840 |
| `046cd326` seg 6 | 1,140 of 3,140 words are the token `light` | 16,644 | **22** | 0.909 |
| `046cd326` seg 8 | `near the end of` x215, year marching 2024 -> 2033 | 13,701 | **28** | 0.643 |

The sharpest demonstration is from PR-028's implementation validation: applying a guard that removed
**12,236 characters** and 118 fabricated claims from `84149f3b` changed **not one** fidelity number —
precision, recall, F0.5, stated, supported, prominent and mentioned were all bit-identical before and
after.

**On the live run of 2026-08-25 (`046cd326`), 70% of all generated visual text — 30,345 of 43,411
characters — sat in the two degenerate segments above. Removing both moves whole-video precision by
1.6 points**, from 0.863 to 0.878.

**Why this blocks the project's objective.** The stated goal is quality per hour of compute, and
`docs/0.0/RESEARCH-BACKLOG.md` records that PR-023's F0.5-per-GPU-hour objective was never exercised
and has no owner. Neither that objective nor any prompt A/B can be trusted while the numerator is
blind to majority-by-volume fabrication. Every figure in PR-026's prompt table carries this
contamination to an unknown degree.

## Scope

**In scope:**
- Extend the fidelity diagnostic so that fabricated **non-numeric** content is priced rather than
  ignored. What "priced" means — an additional fact kind, a separate volume-based term reported
  alongside precision, or a distinct statistic that never enters precision — **is the research**, not
  a decision to be made here.
- A **migration and comparability story**, which is mandatory rather than optional: changing what
  counts as a fact moves every precision figure already recorded in `prs/`, `docs/` and 44 stored
  timelines. The research must decide whether the change is versioned, whether historical arms are
  re-scored, and how a reader tells which metric version produced a number.
- Re-scoring the existing arms with the new metric, using `vid-to-text rescore` (which recomputes
  offline from `timeline.json` + `ocr.json`, so no GPU time is needed for jobs that have both).

**Explicitly out of scope:**
- Guarding or truncating the fabricated text. This PR measures; PR-031 guards.
- Changing frame sampling, prompts, or any capture parameter.
- Re-running any job on the GPU. Re-scoring is offline.

## Dependencies

- **PR-023** — the fidelity diagnostic this extends. Landed `f570aec`.
- **PR-028** — supplied the measurement that proves the blindness. Landed `82df218`.

## Architecture section implemented

`docs/ARCHITECTURE.md` — § Fidelity Diagnostic.

## Verification criteria

<!-- Populated after research. -->
- [ ] The three segments in the Motivation table are priced materially differently from clean
      segments of comparable length
- [ ] Removing PR-028's 12,236 fabricated characters from `84149f3b` now moves the score
- [ ] Every historical arm is either re-scored or explicitly marked as scored under the old metric
- [ ] A metric version is recorded in the timeline, so a figure can never be silently compared across
      versions
- [ ] `cargo test --workspace` passes

## Research backing

**Tier-2.** Load-bearing questions with no in-repo answer:

1. What is the unit of a fabricated non-numeric claim, and how is it scored without a reference? OCR
   supplies ground truth for numbers and labels; there is no equivalent for "the presenter draws a
   line from point 4 to point 3".
2. Should this enter precision, or be reported as a separate statistic? Folding it in changes the
   meaning of every recorded figure; keeping it separate risks it being ignored, which is the present
   failure.
3. How do comparable systems score free-text fabrication against a partial reference? This is the
   question most likely to have external prior art and is the reason for the Tier-2 assignment.
4. What is the migration rule for a metric change in a corpus that is meant as research substrate?

## Notes

- Compression ratio is not a candidate: measured and rejected on this corpus in PR-025, and the
  rejection is recorded so it is not re-opened without new justification.
- The three guards at generation time treat symptoms. This PR is the instrument, and until it is
  fixed no threshold set against it — including PR-031's — is trustworthy.
