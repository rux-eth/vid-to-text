# PR-030: Close the vision-prompt shipping decision

**Landed-in:** (not yet landed)

**Path tier:** Tier-1 (the comparison design, metric and arms were all settled by PR-026's full
Tier-2 round; what remains is one missing measurement and the judgement it informs — but Phase 1 must
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

Each cell is **F0.5 / `stated` facts / thousands of characters of visual text** — the three reported
together, per `docs/ARCHITECTURE.md`, because an F0.5 gap next to a large fact-count gap is
uninterpretable on its own. Counts re-derived from the stored timelines on 2026-08-26.

| prompt | `2024_4_8` | `2024_6_24` | `2025_05_26` |
|---|---|---|---|
| general `c0fe5d36` | 0.516 / 111 / 32.5k | 0.659 / 240 / 41.5k | 0.536 / 226 / 42.5k |
| v1 `923b869a` | 0.488 / 159 / 12.7k | 0.530 / 218 / 18.5k | — |
| v2 `a4a133fc` | 0.584 / 285 / 13.1k | 0.663 / 310 / 38.4k | 0.638 / 310 / 18.4k |
| v3 `c0921846` | 0.472 / 556 / 24.9k | — | — |
| **v3.1 `cfab896e`** | 0.604 / 291 / 9.5k | 0.668 / 386 / 43.4k | **MISSING** |

**No mean-of-shared-clips column, and no bolded best cell** — both were removed on 2026-08-26 when the
ranking was dropped; they invited exactly the reading this PR may not make. The bold on the v3.1 row
marks the **currently shipped default**, not a winner. v3.1 and v2 differ on both
shared clips by the same trade each time — roughly 4 points of precision for 3 points of recall — and
that trade, not the F0.5 gap, is what the decision turns on. **One 8-minute GPU run on `2025_05_26`
fills the last cell.**

**This decision is NOT a ranking, and the table above is not a leaderboard.** Recorded 2026-08-26,
during PR-029's Phase 1, which surfaced the conflict:

`docs/ARCHITECTURE.md` § Review states that **"the metric is not trusted for tuning until that κ has
been reported"**, and PR-026 records the consequence of cutting the κ calibration study in terms that
bind here: *"no claim of the form 'prompt B is more accurate than prompt A' may be made."* κ has never
been reported, and the calibration is blocked on the **review-sheet interaction** — `cohen_kappa` and
`review --labels` both work, but `render_html` emits a table row per fact with three radio buttons and
~150 of those was rejected as unworkable (`docs/0.0/RESEARCH-BACKLOG.md`). Unblocking it means
re-opening the measurement programme that was deliberately deleted with PR-027 on 2026-08-25.

**So this PR decides the prompt the way PR-026 already constrained itself to: on the
precision-versus-recall trade, and by reading the output.** Fidelity figures serve as a **guardrail**
— did anything collapse — never as the ranking function. Three findings shape that judgement:

- **The general prompt still has the best precision of any arm on `2024_6_24` (0.950 against v3.1's
  0.863).** The chart prompt buys recall, not accuracy — which is what PR-026's own research
  predicted. Whether that trade is the right one for a research corpus is a decision, not a
  measurement.
- **The arms are not length-comparable, so a score gap between them is not a win.**
  `docs/ARCHITECTURE.md` records that *"a score difference accompanied by a large fact-count
  difference is uninterpretable, not a win."* Measured 2026-08-26: on `2024_4_8` the arms span **3.4x
  in visual text volume** (9,507 → 32,508 chars) and **5.0x in fact count** (111 → 556 stated). F0.5
  also carries a brevity reward that is wrong for a verbosity comparison (`ARCHITECTURE.md`, "β is not
  neutral") — and these arms differ mainly in verbosity.
- **Every figure in the table was produced by a metric that cannot see majority-by-volume
  fabrication** (PR-029). On the v3.1 `2024_6_24` run, 70% of generated visual text sat in two
  degenerate segments and removing them moved precision by **1.53 points**. That is why PR-029 still
  comes first: the guardrail has to be able to see the failure it is guarding against, even though it
  is not the decider.

## Scope

**In scope:**
- Run the missing arm: v3.1 on `2025_05_26_5-20.mp4`, `market-research` profile, same sampling as
  every other arm.
- Re-score all arms under PR-029's metric so the guardrail figures are read on one metric version,
  using `vtt-client rescore`. **All 11 arms have `ocr.json`** (verified 2026-08-26), so this is fully
  offline and needs no GPU time.
- **Read the output of the candidate arms.** This is the primary evidence, not a supplement to the
  numbers.
- **Make and record the decision**: keep v3.1, revert to v2, or revert to the general prompt.
  Update `prompts/vision-chart.txt` and PR-026's shipping section accordingly.
- Record the decision's basis explicitly — the precision-versus-recall trade, what reading the output
  showed, and the fact that no arm was declared more accurate than another.

**Explicitly out of scope:**
- Authoring a new prompt version. The candidates are the four already measured.
- Changing sampling or any capture parameter — arms must stay paired.
- Re-opening PR-026's research; its round stands.

## Dependencies

- **PR-029** — so the guardrail figures can see the failure mode. This PR does not rank the arms, so
  PR-029 cannot "reorder" them; what it changes is whether a collapse would be visible at all. On the
  v3.1 arm the metric currently prices majority-fabricated output at 0.863, which is not a usable
  guardrail.
- **PR-032** — so the run is submitted with the profile actually applied. Today the documented CLI
  path silently ignores `--profile` for local files.
- **PR-026** — supplies the comparison design, the metric and three of the four arms. Landed `38fb3c8`.

## Architecture section implemented

`docs/ARCHITECTURE.md` — § Capture Configuration (which prompt is the locked default) and
§ Fidelity Diagnostic (§ Comparing two arms).

## Verification criteria

- [ ] v3.1 measured on `2025_05_26_5-20.mp4` at the same sampling as every other arm
- [ ] All arms scored under a single, named metric version, reported as a guardrail
- [ ] The decision is recorded with its basis, including the precision/recall trade, the
      general prompt's precision lead on `2024_6_24`, and what reading the output showed
- [ ] **No claim of the form "prompt B is more accurate than prompt A" appears anywhere in the
      output of this PR**, per `docs/ARCHITECTURE.md` § Review; the arms' fact-count and length
      spread is reported alongside any figure quoted
- [ ] `prompts/vision-chart.txt` and the `market-research` profile reflect the decision
- [ ] PR-026's "Shipping decision is OPEN" section is closed and points here
- [ ] `cargo test --workspace` passes

## Research backing

**Tier-1.** PR-026's Tier-2 round settled the comparison design, the primary measure and the
tuning/held-out split before any cycle was run; those are not re-opened. Phase 1 must confirm the
metric version, that the arms are still paired, and that PR-029 has not invalidated the recorded
figures.

**Scope amended 2026-08-26 by user decision**, on PR-029's Phase 1 finding that this PR's original
deliverable — a ranking of prompt variants on F0.5 — is forbidden by `docs/ARCHITECTURE.md` § Review
until κ is reported. The alternative considered and rejected was re-opening κ calibration, which is
blocked on the review-sheet interaction and would resurrect the deleted PR-027 measurement programme.
**What is given up:** no statistical claim that any prompt is more accurate than another; the choice
rests on the precision/recall trade and on reading the output. This is the same bargain PR-026 struck
and is recorded there as the sharpest thing given up.

## Notes

- **v2 and v3 prompt files no longer exist.** Only v1 (`923b869a`, in `38fb3c8`) and v3.1
  (`cfab896e`, in `c2d11ae`) were ever committed; v2 and v3 survive only as hashes in PR-026's table,
  and the desktop's `~/vtt-exp/prompt-ab*` directories retain manifests but no prompt text. **If this
  PR decides to revert to v2, there is nothing to revert to.** Reconstructing or re-deriving v2 is a
  cost that must be counted before choosing it, and committing every prompt version that is measured
  is a process fix worth making regardless.
- `2025_05_26` was never started under v3; `2024_6_24` was cancelled mid-flight. Only the missing
  v3.1 cell is needed.
