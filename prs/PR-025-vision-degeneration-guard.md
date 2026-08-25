# PR-025: Vision degeneration guard

**Landed-in:** (not yet landed)

**Path tier:** Tier-1 (failure mode and threshold measured on this corpus; the mechanism is an
extension of prior art already in this repo)

## Before Implementation (NON-NEGOTIABLE)

This PR MUST NOT be implemented until `PROCEDURE-pr-research.md` has been completed in full and its output appended to the `## Research findings` section below.

**Tier-1 PRs** (research-backed at design time): Phase 1 (State Assessment) is required to catch drift. Phases 2-4 may be light if no drift is found.

**Tier-2 PRs** (research-pending): all 5 phases of `PROCEDURE-pr-research.md` must run before this PR is written in final form.

Skipping the PR research procedure is a hard violation of the research-backed-decisions constraint in `docs/CONSTRAINTS.md`.

## Research findings

### State Assessment (2026-08-25)

**Current state**: `truncate_repetition` (`vision.rs`, from `6f10acd`, merge #18) guards vision output
by cutting text where a *sentence* of >=15 chars repeats a third time. `repetition_report`
(`whisper.rs`, PR-020 Q5, window-scored in PR-022) flags speech by compression ratio and
**deliberately excludes visual segments**, documented as "`vision::truncate_repetition` already guards
them at generation time". PR-023's fidelity diagnostic exposed that this assumption is false.

**The failure, measured.** Vision output degenerates into numeric enumeration that contains no
repeated sentence, so `truncate_repetition` cannot see it and `repetition_report` never looks. On
clip900 the model wrote a legitimate Fibonacci list followed by a constant +0.001 arithmetic ramp:

> ... key Fibonacci retracement levels (e.g., 0, 0.25, 0.5, 0.618, 0.786, 1, 1.272, 1.382, 1.493,
> 1.618, 1.738, **1.801, 1.802, 1.803, 1.804, ... 1.928** ...

564 numeric tokens, 82% of the segment's words, 4,762 characters, compression ratio 2.95.
**3 of 7 completed runs contained such a segment** (clip900 twice, `2024_2_19` at T0.05/G15 once);
in them 13-35 of ~570 stated facts were supported, dragging whole-video precision to 0.15-0.23
against 0.86-0.95 for clean runs.

**A second mode**, found while setting the threshold: the model repeats one value rather than
ramping — "... 1.618, 1.802, 1.801, 1.801T, **1.738T, 1.738T, 1.738T** ..." x40 (compression ratio
11.32, the highest of any segment). Capping run length catches both modes; nothing else needed.

**Threshold, measured over 2,433 visual segments from every run on disk, with the same tokenizer the
guard uses**: legitimate runs top out at **38**; degenerate runs start at **166** (then 440, 455, 460,
500, 501, 529, 545, 548, 556, 564x3, 567, 568, 589, 633, 705). A cap of **40** sits in that gap and
truncates exactly those 18 segments (0.74%).

*A first pass at this measurement used a tokenizer that split at unit suffixes ("1.738T" -> "1.738"
+ "T"), which broke runs apart and suggested a boundary of 19 vs 183 and a cap of 24. That cap would
have truncated five segments with runs of 25-38 that are plausibly legitimate axis lists. Corrected
by measuring with the implementation's own tokenizer; recorded because the wrong number was nearly
shipped.*

**Compression ratio was evaluated as the detector and rejected on measurement.** It does not separate
the classes: at 2.4 it flags 302/2415 clean segments (12.5%) and still misses 3 of 18 degenerate ones;
at 3.0 it flags 38 clean but catches only 2 of 18. Clean segments have median CR 2.18 and p99 3.17,
while the numeric ramps sit at ~2.95 — inside the normal distribution. Run length separates perfectly;
compression ratio does not. The visual-segment repetition flag proposed in the draft is therefore
dropped rather than shipped as noise.

**Assumptions at draft time**: that a retry would fix it. **False** — `ollama.temperature = 0`, so
the existing 3-attempt retry loop reproduces the same output. The guard must edit, as
`truncate_repetition` already does, or nothing changes.

**New constraints**:
- Editing at generation time is consistent with existing behaviour (`truncate_repetition`) and with
  Segments Are Immutable After Merge, which binds *after* merge. Editing after merge is forbidden.
- Legitimate chart content lists axis ticks and Fibonacci levels; the guard must keep the prefix and
  cut only the excess, never drop the whole segment.

**Downstream contracts**: **none** (`grep -rn "PR-025" prs/ docs/`). Upstream: PR-023 supplied the
diagnostic that found this; PR-022 supplied the segment shape.

**Path-tier checkpoint**: Tier-1. No external round: the failure, the threshold and the prior art are
all measured or in-repo. The one external question (is compression ratio a defensible degeneration
signal) was researched in PR-020 Q5 and is reused unchanged.

### Gate Check (2026-08-25)

- Premise valid: ✓ (3/7 runs affected, threshold measured)
- Prerequisites: none
- Risks accepted: a legitimate list longer than the cap would be truncated (measured headroom: 19
  observed vs 24 cap); the guard edits model output, which is why it logs every truncation
- User approved: ✓ ("lets do it", 2026-08-25)
- Implementation cleared: ✓

### Implementation Validation (2026-08-25)

`truncate_numeric_run` runs after `truncate_repetition` at generation time and logs every cut.
228 tests pass, including one that feeds the function the verbatim clip900 failure.

**Verified on the case that reliably degenerated.** clip900 produced the ramp in both prior runs;
re-run with the guard on the same profile:

| | max numeric run | stated facts | supported | precision |
|---|---|---|---|---|
| clip900 before | **564** | 740 | 160 | **0.216** |
| clip900 with guard | **40** (the cap) | 217 | 158 | **0.728** |

Server log: `[vision] batch 1 enumerated 564 consecutive numbers (cap 40), truncated from 4762 to
1101 chars`. Unsupported facts fell from 580 to 59 while supported facts were untouched (160 -> 158),
which is the intended shape: the guard removes fabricated residue and leaves real content alone.

**Residue, stated honestly.** Cap 40 keeps 40 numbers of the 564, of which ~29 are ramp values, so a
degenerate segment still carries ~29 fabricated numbers instead of ~553. A lower cap would remove
more residue at the risk of truncating legitimate lists (the longest legitimate run measured is 38).
The cap is set for the false-positive side of that trade.

**clip900 remains the worst clip** at 0.728 against 0.86-0.95 for others, so the guard fixes this
failure mode without making clip900 a clean video. Whatever else is weak there is unexplained and
not addressed here.

---

## Motivation

A corpus intended as research substrate cannot carry segments that are 95% fabricated numeric noise,
and 3 of 7 runs produced one. This is a larger quality defect than the ~2% invention rate PR-024
addresses, and it is invisible to both existing guards.

## Scope

- **`truncate_numeric_run`** in `vtt-core/src/vision.rs`: cap a run of consecutive numeric tokens
  (separated only by whitespace, commas or semicolons) at `vision.max_numeric_run`, cutting at the
  start of the first token past the cap and closing the sentence. Applied after `truncate_repetition`
  at generation time; every truncation is logged with the run length.
- **`vision.max_numeric_run`** (default 40; `0` disables). Measured headroom: longest legitimate run
  observed is 38, shortest degenerate run 166.
- ~~Visual segments enter the repetition report by compression ratio~~ — **dropped on measurement**
  (see Research findings): compression ratio cannot separate degenerate from clean visual output at
  any threshold. The truncation log is the flag.

**Out of scope**: retrying or re-prompting a degenerate batch (temperature 0 makes retries identical);
any post-merge edit.

## Dependencies

- **PR-023** — the fidelity diagnostic that surfaced this. Landed `f570aec`.

## Architecture section implemented

`docs/ARCHITECTURE.md` — Fidelity Diagnostic / vision output guards.

## Verification criteria

- [x] The real clip900 degenerate text is truncated to the cap, keeping the legitimate Fibonacci
      prefix, pinned by a test using the verbatim string
- [x] A legitimate 38-number run is untouched at the default cap
- [x] `max_numeric_run = 0` disables the guard
- [x] Truncations are logged with the observed run length
- [x] The repeated-value degeneration mode ("1.738T" x40) is caught by the same cap
- [x] Re-running clip900 (which degenerated twice) produces no segment above the cap
- [x] `cargo test --workspace` passes

## Research backing

Tier-1. Failure mode, frequency (3/7 runs) and threshold (19 legitimate vs 183+ degenerate across
2,423 segments) measured on this corpus. Compression ratio as a reference-free degeneration signal is
carried unchanged from PR-020 Q5. Prior art `truncate_repetition` (`6f10acd`) establishes that editing
vision output at generation time is this project's accepted remedy.

## Notes

- The retry loop cannot help at `temperature = 0`; recorded so nobody proposes it again.
- The guard treats a symptom. Why the model ramps at all is unexplained; a candidate contributor is
  that chart axis ladders give it a numeric pattern to continue.
