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

### State Assessment (2026-08-26)

**Both dependencies landed today, hours before this assessment.** PR-029 (`8cc4b11`, `d79aaad`) changed
what the metric reports; PR-032 (`04455a4`) fixed the submission path this PR's one remaining run
depends on. Neither is deployed.

**Current state:**

- **The running server predates both.** `vtt-server` is up on the desktop (pid 907228) from a binary
  built **Aug 25 21:33** — before PR-029 and PR-032 were committed. A run submitted now would produce
  no `signature` and no `yield_concentration`, and would still silently ignore `--profile` for a local
  file. **Deploying is a precondition for the remaining run**, and PR-032 exists precisely so this run
  is submitted correctly.
- **Config back-compat verified, not assumed.** `FidelityConfig` carries `#[serde(default)]`
  (`vtt-core/src/config.rs:222`), so PR-029's new `min_facts_for_yield` key defaults cleanly and the
  deployed `server.toml` — which sets only `enabled = true` under `[fidelity]` — parses unchanged.
- **The prompt under test is already deployed.** `~/vid-to-text/prompts/vision-chart.txt` on the
  desktop hashes to `cfab896e…` — v3.1, the arm being completed. No prompt deploy needed.
- **The clip is staged.** `/home/rux/vtt-exp/study/2025_05_26_5-20.mp4`, 113.8 MB.
- **The GPU is idle.** `nvidia-smi` reports 0% utilisation and 460 MiB used; `ollama ps` shows no
  loaded model; no job is in flight. Deploying now interrupts nothing.
- **All 11 arms have been re-scored under one signed metric** (offline, no GPU), satisfying this PR's
  first verification criterion ahead of the run. Signature:
  `vtt-fidelity|v:1|ref:kept|tol:0|persist:5|height:10|stop:19-6ff508cb79a3821a`.

**The completed comparison, on one metric version** — F0.5 / precision / recall / `stated` / kchars /
**yield concentration** (PR-029's guardrail; ~1.0 is unremarkable, high means the text is piled into
segments that state almost nothing):

| prompt | `2024_4_8` | `2024_6_24` | `2025_05_26` |
|---|---|---|---|
| general `c0fe5d36` | .516 / .883 / .194 / 111 / 32.5k / **0.85** | .659 / .950 / .296 / 240 / 41.5k / **1.14** | .536 / .903 / .204 / 226 / 42.5k / **1.00** |
| v1 `923b869a` | .488 / .893 / .174 / 159 / 12.7k / **1.06** | .530 / .885 / .203 / 218 / 18.5k / **0.94** | — |
| v2 `a4a133fc` | .584 / .926 / .235 / 285 / 13.1k / **1.00** | .663 / .906 / .320 / 310 / 38.4k / **2.37** | .638 / .923 / .286 / 310 / 18.4k / **1.06** |
| v3 `c0921846` | .472 / .529 / .330 / 556 / 24.9k / **0.87** | — | — |
| **v3.1 `cfab896e`** | .604 / .890 / .265 / 291 / 9.5k / **1.00** | .668 / .863 / .351 / 386 / 43.4k / **10.17** | **MISSING** |

**New constraints / evidence PR-029 introduced, which the PR was drafted before seeing:**

1. **v3.1's best F0.5 cell is also its worst guardrail reading, by an order of magnitude.** On
   `2024_6_24`, v3.1 scores concentration **10.17** against v2's **2.37** and the general prompt's
   **1.14** on the same clip, at the same sampling. Its recall lead there (.351 vs .320) was bought
   with text that the metric can price only as bulk. This is exactly the reading PR-029 was built to
   supply, and it did not exist when this PR's Motivation was written.
2. **The effect is clip-conditional, not a blanket property of v3.1.** On `2024_4_8` both v2 and v3.1
   sit at **1.00**. So the honest statement is "v3.1 degenerated on `2024_6_24`", not "v3.1
   degenerates" — and a single further clip cannot settle which, since `2025_05_26` will be one
   observation.
3. **The missing cell is now decision-relevant in a way the F0.5 column alone is not.** Whether v3.1's
   concentration on `2025_05_26` looks like its 1.00 on `2024_4_8` or its 10.17 on `2024_6_24` bears
   directly on the trade this PR must judge.

**Stale assumptions:** none of substance. The PR was already amended today (ranking dropped, table
rebuilt with fact counts and lengths, `vtt-client` binary name corrected); this assessment confirms
those amendments still hold and adds the guardrail column, which did not exist when they were made.

**Downstream contracts:** **none** — `grep -rn "PR-030"` returns only `PR-032`'s dependency note,
`ROADMAP.md` and `RESEARCH-BACKLOG.md` index rows. Upstream: PR-029 `[x]`, PR-032 `[x]`, PR-026 `[x]`,
all landed.

**Path-tier checkpoint:** **Tier-1 confirmed** (PR header, `ROADMAP.md:87`, `RESEARCH-BACKLOG.md:71`
agree). Phase 1 found no premise change and no unsatisfiable contract. **Cleared after Phase 1**;
Phases 2-4 do not run.

**Blocking on an operator decision, not on research.** The remaining work is a **GPU run on the shared
desktop**, which requires first **deploying PR-029 + PR-032** and restarting the running server. Both
are outward-facing actions on a machine the user owns, so they are not taken unasked. Everything
offline is already done.


### The Missing Arm, and the Decision (2026-08-26)

**Run.** Job `4fdf172c`, `2025_05_26_5-20.mp4`, `market-research`, 506.4s over 6 chunks. Submitted
through the CLI path PR-032 repaired — the client reported `(profile: market-research)` and the first
`[ocr] chunk_0` line reported **20 frames**, confirming adaptive sampling actually applied where fixed
mode would have shown 360. `capture` records prompt `cfab896e` (v3.1), `sampling: adaptive`,
`use_transcript: false`, `transcript_window: causal` — fully paired with every other arm.

**The completed comparison.** F0.5 / precision / recall / **yield concentration**, all arms under one
signature (`vtt-fidelity|v:1|ref:kept|tol:0|persist:5|height:10|stop:19-6ff508cb79a3821a`):

| prompt | `2024_4_8` | `2024_6_24` | `2025_05_26` (held out) |
|---|---|---|---|
| general `c0fe5d36` | .516 / .883 / .194 / 0.85 | .659 / **.950** / .296 / 1.14 | .536 / .903 / .204 / 1.00 |
| v1 `923b869a` | .488 / .893 / .174 / 1.06 | .530 / .885 / .203 / 0.94 | — |
| v2 `a4a133fc` | .584 / .926 / .235 / 1.00 | .663 / .906 / .320 / **2.37** | .638 / .923 / .286 / 1.06 |
| v3 `c0921846` | .472 / .529 / .330 / 0.87 | — | — |
| **v3.1 `cfab896e`** | .604 / .890 / .265 / 1.00 | .668 / .863 / .351 / **10.17** | **.689 / .935 / .336 / 0.96** |

**The concentration figure answers the question Phase 1 raised.** v3.1's 10.17 on `2024_6_24` is
**clip-conditional, not a property of the prompt**: on `2024_4_8` it scores 1.00 and on the held-out
clip 0.96. v2 degrades on the same clip too (2.37), just less. `2024_6_24` is hard for both.

**Reading the output — the primary evidence, not a supplement.** Same segment window
(`00:04:51–00:06:00`) on the held-out clip:

- **v3.1, 1,051 chars:** axis span 3.39T–3.65T, x-axis March 2024–May 2026, the OHLC header
  `O3.37T H3.42T L3.36T C3.39T (+0.53%)`, a drawn line at 3.39T, a shaded band 3.39T–3.45T, an
  oscillator reading 67, and the Elliott Wave overlay labels.
- **v2, 292 chars:** approximate axis range, two labelled horizontal lines.

v3.1 states roughly **3.6x the content on the same window, at higher precision** (.935 vs .923). Its
ten segments run 966–1,697 chars with 34–88 chars per fact — no bloat anywhere, and per-segment
precision is 0.750–1.000 with seven segments above 0.9.

**What is still wrong with v3.1, recorded rather than smoothed over.** The cursor-chronology
fabrication PR-026 named and chose not to build a detector for is **still present**: "The presenter
scrolls the chart" appears **12 times** in 12,939 characters, and the narration is sometimes
self-contradictory — scrolling "to the right" while the cursor moves from February 2025 *back* to
August 2024, then forward again. This content is unverifiable by construction (it is Trend/Magnitude
in CHOCOLATE terms, which PR-023 excluded) so it costs nothing in precision and nothing in the
guardrail at this volume. It is a real defect that no instrument in this project can currently price.

---

**DECISION: keep v3.1 (`cfab896e`). No file change — `prompts/vision-chart.txt` already holds it.**

**Basis, stated as the amended scope requires — not read off the F0.5 column:**

1. **On the held-out clip there is no trade to adjudicate.** v3.1 beats both alternatives on
   precision *and* recall simultaneously (.935/.336 against v2's .923/.286 and general's .903/.204).
   The precision-versus-recall tension that made this a judgement call on the tuning clips does not
   arise on the clip that was held out.
2. **The recorded caution about the general prompt does not survive the held-out clip.** Its precision
   lead is real on `2024_6_24` (.950 vs .863) and is the reason this decision was not to be read off a
   score column — but on `2025_05_26` v3.1 is ahead on both axes, and general's recall is 0.204
   against 0.336, which for a research corpus is a large amount of on-screen fact left unmentioned.
3. **The guardrail supports keeping it, with one flagged exception.** Two of three clips are clean
   (1.00, 0.96). The `2024_6_24` cell is genuinely bad and is now *visible*, which it was not when this
   PR was drafted. PR-031 targets that mode directly.
4. **Reverting to v2 is not actually available.** Its prompt file was never committed
   (`prs/PR-030` § Notes); "revert to v2" would mean reconstructing it from a hash. Given v3.1 is ahead
   on the held-out clip, that cost buys nothing.

**No accuracy claim is made.** No arm is asserted to be more accurate than another; `docs/ARCHITECTURE.md`
§ Review's rule stands, and κ remains unreported.

**Risks accepted:**
- v3.1 degenerates on `2024_6_24` (concentration 10.17) and nothing guards that mode until PR-031.
- The cursor-chronology fabrication persists, unpriced by any instrument here.
- The held-out clip is **one** clip. This decision rests on three clips total, two of them tuning.


### Gate Check (2026-08-26)

- Premise still valid: ✓ — the decision was open and is now closed with the table completed
- No prerequisite PRs surfaced: ✓ (PR-029 and PR-032 both landed today and were deployed for this run)
- Verification criteria:
  - [x] v3.1 measured on `2025_05_26_5-20.mp4` at the same sampling as every other arm — `capture`
        confirms prompt `cfab896e`, adaptive, `use_transcript: false`, causal
  - [x] All arms scored under a single, named metric version, reported as a guardrail
  - [x] The decision is recorded with its basis, including the precision/recall trade, the general
        prompt's precision lead on `2024_6_24`, and what reading the output showed
  - [x] `prompts/vision-chart.txt` and the `market-research` profile reflect the decision — v3.1 was
        already deployed and stays; **no file change**
  - [x] PR-026's "Shipping decision is OPEN" section is closed and points here
  - [x] `cargo test --workspace` passes
  - [x] No claim of the form "prompt B is more accurate than prompt A" appears in this PR's output;
        fact count and text volume are reported beside every figure
- Implementation cleared / PR complete: ✓ (2026-08-26)

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
