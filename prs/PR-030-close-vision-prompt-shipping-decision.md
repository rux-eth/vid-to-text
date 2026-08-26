# PR-030: Close the vision-prompt shipping decision

**Landed-in:** (not yet landed)

**Path tier:** Tier-1 (the comparison design, metric and arms were all settled by PR-026's full
Tier-2 round; what remains is one missing measurement and the decision it feeds — but Phase 1 must
catch the drift PR-029 deliberately introduces)

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

_To be populated by `PROCEDURE-pr-research.md`._

---

## Motivation

**PR-026 shipped a prompt but not a decision.** `prompts/vision-chart.txt` holds v3.1 (`cfab896e`) and
PR-026 states plainly that the shipping decision is **OPEN**: v2 (`a4a133fc`) was the only version
measured on all three clips, including the held-out one.

Two of the three cells are now filled — `2024_6_24` was completed on 2026-08-25 by the live run that
deployed PR-028 (`046cd326`). State of the comparison, all arms at identical adaptive sampling and
therefore paired:

| prompt | `2024_4_8` | `2024_6_24` | `2025_05_26` | mean of the two shared |
|---|---|---|---|---|
| general `c0fe5d36` | 0.516 | **0.659** | 0.536 | 0.588 |
| v1 `923b869a` | 0.488 | 0.530 | — | 0.509 |
| v2 `a4a133fc` | 0.584 | 0.663 | 0.638 | 0.624 |
| v3 `c0921846` | 0.472 | — | — | — |
| **v3.1 `cfab896e`** | 0.604 | 0.668 | **MISSING** | **0.636** |

v3.1 leads v2 on both shared clips by the same trade each time — roughly 4 points of precision for
3 points of recall. **One 8-minute GPU run on `2025_05_26` completes the table.**

**Two findings say this decision must not simply be read off the F0.5 column:**

- **The general prompt still has the best precision of any arm on `2024_6_24` (0.950 against v3.1's
  0.863).** The chart prompt buys recall, not accuracy — which is what PR-026's own research
  predicted. Whether that trade is the right one for a research corpus is a decision, not a
  measurement.
- **Every figure in the table was produced by a metric that cannot see majority-by-volume
  fabrication** (PR-029). On the v3.1 `2024_6_24` run, 70% of generated visual text sat in two
  degenerate segments and removing them moved precision by 1.6 points. The arms may reorder under a
  metric that prices this, which is why PR-029 comes first.

## Scope

**In scope:**
- Run the missing arm: v3.1 on `2025_05_26_5-20.mp4`, `market-research` profile, same sampling as
  every other arm.
- Re-score all arms under PR-029's metric so the comparison is made on one metric version, using
  `vid-to-text rescore` where `ocr.json` exists.
- **Make and record the decision**: keep v3.1, revert to v2, or revert to the general prompt.
  Update `prompts/vision-chart.txt` and PR-026's shipping section accordingly.
- Record the decision's basis, including the precision-versus-recall trade, explicitly rather than
  implicitly.

**Explicitly out of scope:**
- Authoring a new prompt version. The candidates are the four already measured.
- Changing sampling or any capture parameter — arms must stay paired.
- Re-opening PR-026's research; its round stands.

## Dependencies

- **PR-029** — so the decision is made on a metric that can see the failure mode. If PR-029 reorders
  the arms, this PR's answer changes.
- **PR-032** — so the run is submitted with the profile actually applied. Today the documented CLI
  path silently ignores `--profile` for local files.
- **PR-026** — supplies the comparison design, the metric and three of the four arms. Landed `38fb3c8`.

## Architecture section implemented

`docs/ARCHITECTURE.md` — § Capture Configuration (which prompt is the locked default) and
§ Fidelity Diagnostic (§ Comparing two arms).

## Verification criteria

- [ ] v3.1 measured on `2025_05_26_5-20.mp4` at the same sampling as every other arm
- [ ] All arms scored under a single, named metric version
- [ ] The decision is recorded with its basis, including the precision/recall trade and the
      general prompt's precision lead on `2024_6_24`
- [ ] `prompts/vision-chart.txt` and the `market-research` profile reflect the decision
- [ ] PR-026's "Shipping decision is OPEN" section is closed and points here
- [ ] `cargo test --workspace` passes

## Research backing

**Tier-1.** PR-026's Tier-2 round settled the comparison design, the primary measure and the
tuning/held-out split before any cycle was run; those are not re-opened. Phase 1 must confirm the
metric version, that the arms are still paired, and that PR-029 has not invalidated the recorded
figures.

## Notes

- **v2 and v3 prompt files no longer exist.** Only v1 (`923b869a`, in `38fb3c8`) and v3.1
  (`cfab896e`, in `c2d11ae`) were ever committed; v2 and v3 survive only as hashes in PR-026's table,
  and the desktop's `~/vtt-exp/prompt-ab*` directories retain manifests but no prompt text. **If this
  PR decides to revert to v2, there is nothing to revert to.** Reconstructing or re-deriving v2 is a
  cost that must be counted before choosing it, and committing every prompt version that is measured
  is a process fix worth making regardless.
- `2025_05_26` was never started under v3; `2024_6_24` was cancelled mid-flight. Only the missing
  v3.1 cell is needed.
