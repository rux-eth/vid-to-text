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

### State Assessment (2026-08-26)

**Method note.** Every figure below was re-derived on 2026-08-26 from the artefacts on the GPU
desktop, not read from a prior PR. A release build of `HEAD` (`edb8bce`) was used to re-run the
scorer; the 13 scorable job directories were pulled to a scratch dir and scored locally.

**Current state**:

- `vtt-core/src/fidelity.rs` is 864 lines and has **exactly one commit** — `f570aec` ("Visual fidelity
  diagnostic (PR-023) and OCR-grounded vision prompt (PR-024)"). It has never been amended. There is
  no accumulated fix history in this file to carry forward; PR-029 would be the first amendment to
  the extraction rules.
- Three fact kinds exist: `Fact::Number` / `Fact::Label` / `Fact::Timeframe` (`fidelity.rs:39-49`).
  `score_segments` (`fidelity.rs:503`) micro-averages `stated`/`supported` and `prominent`/`mentioned`
  across all visual segments.
- `FidelitySummary` (`vtt-core/src/types.rs:75-90`) has ten fields and **no metric-version field**.
  Its one existing "how to read this figure" flag is `ocr_grounded`, which is *not* set by
  `score_segments` (hardcoded `false` at `fidelity.rs:606`) but patched in afterwards at
  `vtt-core/src/pipeline.rs:387`.
- **The built binary is `vtt-client`, not `vid-to-text`.** `CLAUDE.md`, this PR's Scope and PR-030's
  Scope all say `vid-to-text rescore`; `cargo build --release` produces `target/release/vtt-client`
  (`vtt-client/Cargo.toml:2`). Any workflow written against the documented name fails with exit 127.
- Server results on the desktop: **45 directories, 44 with `timeline.json`, 14 with `fidelity.json`,
  13 with `ocr.json`.** One job has `fidelity.json` but no `ocr.json` and is therefore permanently
  un-rescorable.
- No profile in `config/profiles/` overrides `number_tolerance`, `min_persist_secs`,
  `min_text_height_px` or `label_stoplist`; the desktop's `server.toml` `[fidelity]` block sets only
  `enabled = true`. `run_rescore` (`vtt-client/src/review.rs:307`) builds a `FidelityConfig::default()`,
  which is therefore the same configuration every stored figure was produced under.

**Verified, not assumed — the Motivation table reproduces exactly:**

| claim | stated in PR | re-derived 2026-08-26 | verdict |
|---|---|---|---|
| `84149f3b` seg 109 | 15,905 chars / 25 facts / 0.840 | 15,905 / 25 / 21÷25 = 0.840 | ✓ |
| `046cd326` seg 6 | 16,644 chars / 22 facts / 0.909 | 16,644 / 22 / 20÷22 = 0.909 | ✓ |
| `046cd326` seg 8 | 13,701 chars / 28 facts / 0.643 | 13,701 / 28 / 18÷28 = 0.643 | ✓ |
| share of visual text in the two segments | 70% (30,345 of 43,411) | 30,345 ÷ 43,411 = **69.9%** | ✓ |
| whole-video precision, both removed | 0.863 → 0.878, "**1.6** points" | 0.8627 → 0.8780 = **+1.53 points** | endpoints ✓, **delta wrong** |

**The re-scoring mechanism is sound — verified, not assumed.** `vtt-client rescore` was run against
all **11** prompt-A/B arms and reproduces the stored `timeline.fidelity` block **bit-exact** on every
one of `stated`, `supported`, `prominent`, `mentioned`, `precision`, `recall`, `f05`. The offline
re-scoring path this PR depends on is therefore trustworthy, and the pre-change baseline is pinned.

**Stale assumptions** (where current state disagrees with what this PR was drafted against):

1. **"44 stored timelines" overstates the re-scoring surface and understates the arm coverage.** Only
   **13 of 44** have the `ocr.json` that `rescore` requires. But **all 11 prompt-A/B arms have it** —
   every cell of PR-030's table is re-scorable offline with no GPU time. The other 31 are pre-fidelity
   dev runs (`clip900`, `det30`) and unrelated YouTube videos, not arms. The scope statement should say
   "the 11 arms, all of which have `ocr.json`" rather than implying 44 timelines are in play.
2. **The "1.6 points" figure is 1.53.** Endpoints 0.863 → 0.878 are right; the delta is 1.5 points, not
   1.6. This number is quoted as load-bearing in five places: this PR, `PR-030` § Motivation,
   `docs/0.0/ROADMAP.md` § Why this order, `docs/0.0/RESEARCH-BACKLOG.md` row PR-029, and the session
   memory. All five need the correction.
3. **The Motivation table mixes two segment-indexing conventions.** `84149f3b` "seg 109" is an index
   into the **merged timeline** (144 segments; it is visual segment #7 of 10). `046cd326` "seg 6" and
   "seg 8" are indices into the **visual-only list** (timeline indices 97 and 121). PR-031 inherits the
   same ambiguity for the same two segments. Whichever convention is chosen must be stated.
4. **`vid-to-text` is not the binary name** (see Current state). PR-030's Scope depends on this command
   too.

**New constraints** (learned from prior PRs and from the codebase, that restrict this PR's options):

1. **PR-023 deliberately excluded exactly this territory, on the record.** Its State Assessment
   concluded "Trend and Magnitude errors (CHOCOLATE) are not OCR-checkable; **the metric must not claim
   them**", and "Trend/Magnitude instruments" are named in its explicit out-of-scope list. The
   fabricated text this PR wants priced ("the presenter draws a line from point ④ to point ③") is
   largely Trend/Magnitude. **This PR must therefore either (a) price fabrication by a route that makes
   no truth claim about trend or magnitude — e.g. a redundancy or volume statistic — or (b) overturn a
   recorded research-backed decision with new evidence.** Route (a) is available and is compatible with
   the PR's own framing; route (b) is a bigger change than the PR currently describes. This choice is
   not acknowledged in the PR as drafted and belongs at the top of Phase 2.
2. **`docs/ARCHITECTURE.md:252` — "The metric is not trusted for tuning until that κ has been
   reported."** That κ has never been reported. Per `docs/0.0/RESEARCH-BACKLOG.md`, PR-023's
   calibration stalled on the **review-sheet interaction**, not the code: `cohen_kappa` works and
   `review --labels` scores a completed sheet, but `render_html` emits a table row per fact with three
   radio buttons and ~150 of those was rejected as unworkable. **Any option requiring human ground
   truth inherits a known-blocked UI problem**, and fixing it is not in this PR's scope.
3. **`docs/ARCHITECTURE.md:241-244` — "Precision alone cannot rank length-differing arms … a score
   difference accompanied by a large fact-count difference is uninterpretable, not a win."** Measured
   across the arms today: on `2024_4_8` they span **3.4x in visual text volume** (9,507 → 32,508 chars)
   and **5.0x in fact count** (111 → 556 stated). This is precisely the case the rule names.
4. **`docs/ARCHITECTURE.md:236-240` — "β is not neutral."** F0.5 is recorded as right for a *sampling*
   comparison and **wrong for a *verbosity* comparison**, because it retains a brevity reward. This
   PR's whole subject is text volume, so any new term interacts with β directly; a volume-sensitive
   term added under F0.5 is not β-neutral and the PR must say what it does about that.
5. **A measurement programme was already judged disproportionate and deleted.** PR-027 ("Vision
   measurement readiness" — κ calibration, paired bootstrap, configurable β, chronology detector) was
   collapsed and deleted on 2026-08-25 **by explicit user decision**, recorded in
   `prs/PR-026-…md:1436-1441`. PR-029 is a Tier-2 measurement PR touching the same surface. The prior
   decision is a live constraint on how much apparatus this PR may propose, and must be surfaced rather
   than rediscovered.
6. **`run_rescore` silently drops `ocr_grounded`.** `score_segments` hardcodes `false` and only the
   pipeline patches the real value in (`pipeline.rs:387`), so any re-score of an OCR-grounded run
   reports `ocr_grounded: false` — dropping the circularity warning. Latent today (no stored run has
   grounding on), but **a metric-version field added to the summary inherits this exact defect** unless
   it is set inside `score_segments`. This PR's verification criterion "a metric version is recorded in
   the timeline" must therefore also hold through the `rescore` path.
7. **Per-segment statistics need no change to `score_segments`.** `fidelity.json` already persists
   `SegmentFidelity.stated[].supported` and `.prominent[].mentioned`; only the summary micro-averages.
   Recorded in `RESEARCH-BACKLOG.md` as a finding rescued from the deleted PR-027. Per-segment or
   per-video precision is computable offline from files already on disk.
8. **Chars-per-scored-fact cannot itself be the instrument — measured.** Across all 127 scored visual
   segments: p50 **96**, p90 **318**, p99 **784**, max **1,019**. The three segments this PR names sit
   at 757, 636 and 489 — but the **top two** are `95f4bc52` vseg 5 (1,019) and vseg 4 (784), which are
   the *general* prompt writing legitimate prose. Inspected directly:

   | segment | chars | type-token ratio | longest consecutive word run | top token |
   |---|---|---|---|---|
   | `95f4bc52` vseg 5 — legitimate prose | 3,058 | **0.399** | 1 | `the` ×66 |
   | `046cd326` vseg 6 — known degenerate | 16,644 | **0.029** | 15 | `light` ×1,140 |

   Volume-per-fact does **not** separate the classes; lexical redundancy separates them by 13.7x on the
   same pair. That is an observation handed to Phase 2, **not** a recommendation: redundancy statistics
   are the same family as the **compression ratio that PR-025 measured and rejected** on this corpus
   (flagged 12.5% of clean segments at 2.4 while still missing 3 of 18 degenerate ones). Phase 3 must
   establish whether that rejection extends to this statistic before treating it as available.

**Downstream contracts** (from the bidirectional sweep — `grep -rn "PR-029" prs/ docs/` returns
`PR-030`, `PR-031`, `ROADMAP.md`, `RESEARCH-BACKLOG.md`):

- **PR-031 (word-run degeneration guard)** → contract: *"a threshold validated against a metric that
  cannot see the failure is not validated … PR-028's residue trade-off was argued in precision terms;
  the same argument is unavailable here until the metric prices this text."* Its verification criterion
  "a legitimate segment at the measured p99 is untouched" needs an instrument that assigns different
  values to `046cd326` vseg 6 and to `95f4bc52` vseg 5. **Satisfiable by the current scope, conditional
  on Q2** — it holds if the chosen instrument prices repetitive non-numeric filler at segment
  granularity, and fails if the answer is a corpus-level statistic only.
- **PR-030 (close the vision-prompt shipping decision)** → originally two contracts, now one.
  The mechanical one — *"all arms scored under a single, named metric version"* — **is satisfiable**:
  all 11 arms have `ocr.json` and re-score bit-exact today. The substantive one was **NOT satisfiable
  by any scope this PR could take**: PR-030's deliverable was a **ranking of prompt variants**, while
  `docs/ARCHITECTURE.md` § Review records that the metric **is not trusted for tuning until κ has been
  reported**, and PR-026 states the consequence explicitly: *"no claim of the form 'prompt B is more
  accurate than prompt A' may be made."* Fixing what the metric *counts* does not lift a restriction
  about whether the metric *agrees with a human*.
  → **RESOLVED 2026-08-26 by user decision: the ranking was dropped from PR-030.** That PR now decides
  the prompt on the precision/recall trade and by reading the output, with fidelity as a guardrail
  only; its Motivation, Scope, Dependencies, verification criteria and Research backing were amended
  the same day. The alternative — re-opening κ calibration — was rejected because it is blocked on the
  review-sheet interaction and would resurrect the deleted PR-027 programme.
  **Contract on PR-029 is now: a guardrail that can see majority-by-volume fabrication, reported per
  arm on one metric version.** Satisfiable by the current scope.

**Path-tier checkpoint**: **Tier-2 confirmed.** The PR header, `docs/0.0/ROADMAP.md:86` and
`docs/0.0/RESEARCH-BACKLOG.md:70` all agree on Tier-2, and no cut-plan disagrees. Phase 1 surfaced an
unsatisfiable downstream contract and an unacknowledged locked decision (constraint 1), so Phases 2-5
run regardless.

**Time-decay**: drafted 2026-08-26, assessed 2026-08-26 — 0 days against a 30-day threshold. No
staleness re-run required. `RESEARCH-BACKLOG.md` § Drift Watch lists nothing for this PR.

**Halt (Phase 1 exit criteria), and its resolution.**

1. **PR-030's contract could not be met by this PR as written. — RESOLVED 2026-08-26 by user
   decision:** the ranking was dropped from PR-030 rather than κ calibration being re-opened. See
   Downstream contracts above. PR-029's deliverable is consequently a **guardrail**, not a ranking
   function — which lowers the bar the instrument must clear: it must make majority-fabricated output
   *visible*, not adjudicate which of two arms is more accurate. Phase 2 scopes against that.
2. **Constraint 1 (PR-023's Trend/Magnitude exclusion) narrows the option space** before the research
   questions are written. Q1 as drafted ("what is the unit of a fabricated non-numeric claim, and how
   is it scored without a reference?") presumes a truth-scoring route that PR-023 closed. The
   reference-free redundancy route stays open, subject to constraint 8. **Carried into Phase 2 as a
   scoping constraint, not a blocker** — it changes which questions get asked, not whether the PR
   proceeds.

### Research Questions (2026-08-26)

**What Phase 1 changed about this section.** The questions drafted in § Research backing assumed this
PR would supply a *ranking-grade* metric and would score fabricated claims for *truth*. Neither holds:
the ranking was dropped from PR-030 by user decision, so the deliverable is a **guardrail** that makes
majority-fabricated output visible; and PR-023's recorded exclusion of Trend/Magnitude closes the
truth-scoring route. Q1 is reframed accordingly, from "what is the unit of a fabricated claim" to
"what observable property makes fabricated bulk visible without a reference".

**A substrate constraint that binds every question below.** PR-028's measurement pool is **11,108
visual segments across 90 guard-era timelines** (113 files in three locations — server results, laptop
cache, `~/Documents/seer_archive` — deduplicated by content SHA-256, 4 pre-guard excluded). But only
**127 segments across 13 jobs have OCR** (Phase 1). So a statistic computable **from segment text
alone** can have its threshold measured at pool scale the way PR-025's and PR-028's were; an
**OCR-dependent** statistic is measurable on **1.1%** of the pool and cannot. This does not decide Q1,
but any OCR-dependent candidate must carry an explicit answer for how its threshold is set.

**Must-answer:**

1. **What reference-free property of a visual segment makes majority-by-volume fabrication visible?**
   — success criteria: a *named* statistic, with its tokenizer stated explicitly; a separation measured
   over the guard-era pool in PR-025/PR-028 form (where legitimate output tops out, where degenerate
   begins, and every segment that sits between, inspected rather than assumed); clearly separated
   values for the four confirmed degenerate segments (`046cd326` vseg 6 and vseg 8, `84149f3b` vseg 7,
   `37a3242c` seg 130 — the 878x `"You're not."` loop) against the confirmed-legitimate prose segment
   (`95f4bc52` vseg 5, TTR 0.399, longest word run 1); and **no truth claim about trend or magnitude**,
   per PR-023's exclusion.

2. **Where does the signal live — inside `precision`, as a separate field on `FidelitySummary`, or
   outside the summary entirely?** — success criteria: for **each** of the three options, a stated
   consequence for all three consumers: (a) the 11 historical arm figures, (b) PR-030's guardrail
   reading, (c) PR-031's threshold validation. Plus a statement of which consumer is prioritised when
   they conflict. Constraint: PR-030 needs the historical figures to remain *readable*, so an option
   that silently redefines `precision` must show how that is preserved.

3. **How do published or production systems report free-text hallucination when the reference covers
   only part of the output?** — success criteria: **≥2 independent** systems with cited primary
   evidence; for each, how the unreferenced remainder is handled (ignored / reported on a separate
   axis / penalised by volume) and whether any folds it into a precision figure; **plus at least one
   counter-case** — a system that deliberately does *not* price the remainder, with its stated reason.
   This is the question that made the PR Tier-2 and is the most likely to have external prior art.

4. **What is the migration and comparability rule for a metric change over an already-scored corpus?**
   — success criteria: a concrete rule covering (i) whether the metric is versioned and where the
   version is stored; (ii) whether the 11 arms are re-scored or marked as old-metric; (iii) what
   mechanically prevents a reader comparing across versions; and (iv) **how the version survives the
   `rescore` path**. (iv) is not hypothetical: `score_segments` hardcodes `ocr_grounded: false`
   (`fidelity.rs:606`) and only `pipeline.rs:387` patches the real value in, so **any summary field set
   outside the scorer is silently dropped on every re-score**. Must also decide whether repairing that
   existing `ocr_grounded` drop is in this PR or recorded as a separate defect.

5. **Does PR-025's compression-ratio rejection extend to the statistic Q1 selects?** — success
   criteria: an explicit yes/no with numbers on the same axes PR-025 used — the false-positive rate on
   clean segments at the chosen operating point, against compression ratio's recorded **12.5% at 2.4**,
   and the miss rate against its **3 of 18** degenerate segments. If the rejection extends, Q1's answer
   is disqualified and Q1 re-runs; this question exists to stop a rejected family being re-adopted
   under a new name.

**Dependencies:**

- **Q3 → Q1, Q2** — prior art informs both the candidate set and where the signal belongs; Q3 runs
  first or alongside.
- **Q1 → Q5** — strictly sequential; Q5 tests whatever Q1 selects.
- **Q1, Q3 → Q2** — what may be reported depends on what can be computed and on how others report it.
- **Q4 independent** of Q1–Q3, Q5: it depends only on the fact that *something* changes, not on what.

**Research plan (depth tiers):**

- **Q1 — Tier A** (probe, external candidate discovery) **→ B if inconclusive**; rationale: the
  candidate set (repetition and lexical-diversity statistics used in long-form generation eval —
  `rep-n`, `distinct-n`, `seq-rep`, TTR, MTLD, self-BLEU) is well-trodden and a probe should surface it
  cheaply. **Escalation trigger, pre-registered:** if Tier A returns no statistic with a *published
  operating point on long-form generation*, escalate to B rather than picking one by intuition — that
  is exactly the "settle on a best-guess" failure the method forbids.
- **Q2 — Tier A**; rationale: mostly an internal design decision against three known consumers, with
  Q3 supplying the external input. No escalation planned.
- **Q3 — Tier A (probe) → B if inconclusive**; rationale: **load-bearing and the reason this PR is
  Tier-2.** **Escalation trigger, pre-registered:** escalate if Tier A finds only secondary sources, or
  finds no system that addresses partial reference coverage at all.
- **Q4 — Tier A**; rationale: metric/schema versioning has ample primary prior art; sub-question (iv)
  is internal and answered from this repo.
- **Q5 — internal — no web round**; resolved by measurement on this corpus in Phase 3.

**Rounds:**

- **Round 1 — parallel Tier-A probes: Q1a (candidate discovery), Q3, Q4.** Independent; no ordering
  between them.
- **Round 2 — internal, no web: Q1b then Q5.** Q1b measures the surviving candidates over the 11,108-
  segment guard-era pool with a faithful port of the implementation's own tokenizer (PR-028's method —
  and PR-025's recorded near-miss, where a different tokenizer nearly shipped a cap of 24 against a
  true 40, is the reason this is not optional). Q5 follows on Q1b's selection.
- **Round 3 — Q2**, synthesising Q1b, Q3 and Q5 against the three consumers.

**Default tier: A.** Escalations above A: **none committed** — two pre-registered triggers (Q1, Q3).
Recorded reason for not pre-assigning B despite the Tier-2 path: PR-027, a measurement programme on
this same surface, was deleted on 2026-08-25 as disproportionate by explicit user decision, and the
deliverable is now a guardrail rather than a ranking function. Probing first and escalating on a
stated trigger respects both without weakening the method. **No Tier E.**

**Explicitly excluded from this round** (nice-to-have, each with what it costs):

- **κ calibration, or any human ground truth.** Blocked on the review-sheet interaction (~150 rows of
  three radio buttons, rejected as unworkable) and no longer needed now that PR-030 does not rank.
  *Cost:* the metric still may not be used to rank prompt variants — already accepted, recorded in
  `docs/ARCHITECTURE.md` § Review.
- **Configurable β, or any change to the F-score.** Deleted with PR-027; a guardrail does not need it.
  *Cost:* F0.5's brevity reward remains wrong for verbosity comparisons — mitigated by PR-030 now
  reporting fact count and length beside every figure.
- **Guarding or truncating the fabricated text.** PR-031 owns it. *Cost:* none to this PR; measuring
  before guarding is the agreed order.
- **Scoring trend/magnitude claims for truth.** PR-023's recorded exclusion. *Cost:* the largest class
  of fabricated content stays unscored *for truth*; this PR prices its **bulk**, not its falsity, and
  must say so plainly wherever the new figure is reported.
- **Re-scoring the 31 non-arm timelines.** They have no `ocr.json` and are permanently un-rescorable.
  *Cost:* none — they are pre-fidelity dev runs and unrelated YouTube videos, not arms.
- **Fixing the review-sheet interaction.** Real, and it blocks κ. *Cost:* κ stays blocked; needs its
  own work item.
- **Per-video scoring, bootstrap intervals, variance estimates.** Computable offline from
  `fidelity.json`, which already carries per-segment detail. *Cost:* none; no change to
  `score_segments` is required for anyone who wants them later.

### Findings — Phase 3, Round 1 (2026-08-26)

Three Tier-A probes, run inline in the main loop (`WebSearch` + `WebFetch` of primary sources), per the
Phase 2 plan. Round 1 covers **Q1a** (candidate discovery), **Q3** and **Q4**. Q1b and Q5 are Round 2;
Q2 is Round 3.

---

**Q1 (part a): What reference-free property of a visual segment makes majority-by-volume fabrication
visible?**

*Options considered:*

- **Option A: `seq-rep-n`** — the fraction of duplicate n-grams in one generated sequence.
  - Source: Welleck et al., *Neural Text Generation with Unlikelihood Training*, ICLR 2020
    (`arxiv.org/abs/1908.04319`).
  - Definition, verbatim: `seq-rep-n = 1.0 − |unique n-grams(x)| / |n-grams(x)|`, range 0 → 1.0.
  - **Computed per generated continuation**, then averaged — the granularity PR-031 needs.
  - Published reference values on `seq-rep-4`: **human 0.006**, degenerate MLE-greedy **0.442**,
    repaired model 0.013. A ~74x separation between human text and degenerate text, published.
  - Pros: standard and widely reported, so a figure is legible outside this repo; text-only, so it is
    measurable over the **full 11,108-segment pool**, not the 127 OCR'd ones; per-sequence by
    construction; no reference required, so PR-023's Trend/Magnitude exclusion is untouched.
  - Cons: **the paper proposes no per-output threshold** — it treats these as continuous improvement
    metrics, so a cap must still be measured on this corpus. Its length sensitivity is **not
    documented** in what the probe found (see disconfirming evidence).

- **Option B: type-token ratio (TTR)** — the Phase 1 observation (0.399 legitimate vs 0.029 degenerate).
  - **DISQUALIFIED by this probe.** TTR is length-dependent: "as texts increase in length, the TTR
    begins to level out or even drop", and "after reaching a point of stabilization, TTR tends to
    monotonically decrease as the number of tokens increases" (Koizumi & In'nami, *System* 40(4), 2012,
    `sciencedirect.com/science/article/abs/pii/S0346251X12000887`).
  - **Phase 1's own comparison is confounded**: it set a **3,058-char** legitimate segment against a
    **16,644-char** degenerate one — a 5.4x length difference, on a statistic that falls with length.
    The observed 13.7x gap cannot be attributed to redundancy on that evidence. This is exactly why
    Phase 1 recorded it as an observation and not a recommendation.

- **Option C: MTLD** — the length-robust lexical-diversity measure, designed to remove TTR's bias by
  measuring the mean length of a token run that keeps TTR above a fixed 0.72.
  - Pros: purpose-built for the confound that disqualifies Option B; reported effective on texts as
    short as ~100 tokens.
  - Cons: carries its own threshold parameter (the 0.72 factor), i.e. it trades the first length
    problem for a parameter-sensitivity problem; less standard in generation eval than `seq-rep-n`;
    the supporting detail here came from a **secondary** source (an encyclopedic topic page), so it is
    labelled accordingly below.

*Disconfirming evidence sought:* I searched specifically for length-sensitivity of `rep-n`/`seq-rep-n`
("rep-n repetition metric sensitive to sequence length confound longer text more n-gram repetition").
**No source was found that characterises it either way.** Longer text has more opportunity for n-gram
collision, so a length effect is plausible and **unconfirmed** — this is a real gap, not a clean bill
of health. It is cheap to settle empirically on our own 11,108 segments (regress the statistic on
segment length over known-clean segments), and Round 2 must do so before any cap is set.

*Recommendation:* **Option A (`seq-rep-n`)**, carried into Round 2 for corpus measurement, with
Option C retained as the fallback if Round 2 finds a length effect that cannot be normalised.
- **Status: convention** — cited in one primary source with published baselines, but the per-output
  operating point this PR needs is not published anywhere the probe reached.
- **Why:** it is per-sequence, text-only, reference-free, standard, and comes with published human vs
  degenerate values that bound the expected separation. Option B is disqualified on a documented
  confound that Phase 1's evidence cannot rule out.
- **Risks accepted:** the threshold is ours to measure, not to adopt; and the length-sensitivity gap
  above is open until Round 2 closes it.

---

**Q3: How do published or production systems report free-text hallucination when the reference covers
only part of the output?**

*Options considered — three independent systems, and they converge:*

- **VeriScore** (`arxiv.org/abs/2406.19276`) — **ignores** the unverifiable remainder, by design.
  Verbatim: "Unverifiable content such as advice, fictional stories, or subjective opinions are
  ignored." Its extractor returns the literal string `"No verifiable claim."` for such sentences, and
  those passages "contribute zero to the numerator but don't inflate claim counts — they're simply
  absent from evaluation."
- **SAFE / F1@K** (`arxiv.org/abs/2403.18802`, Google DeepMind) — **excludes** them: "discarding
  irrelevant facts better isolates measuring factuality", on the grounds that relevance measures
  instruction-following rather than factuality.
- **RefChecker** (`arxiv.org/abs/2405.14486`, Amazon) — **the counter-case.** It runs 3-way
  classification precisely because binary labelling "can only distinguish factual and non-factual
  claims", defines *Neutral* as "cases where the reference is insufficient to verify the
  claim-triplets", and **folds Neutral into the reported hallucination rate** ("the ratio of
  Contradiction and Neutral claims"), while still breaking the three classes out separately.

*The finding that matters most, and it cuts against this PR's premise:* **the field standard is to
exclude unverifiable content from the factuality score, deliberately — which is exactly what
`score_segments` already does.** This PR is therefore proposing a *departure from convention*, and must
justify it on the difference in purpose: VeriScore and SAFE are **benchmarking factuality** across
models, where excluding unverifiable text is correct; this metric is a **guardrail against
degeneration** on a single corpus, where the excluded text is the failure being guarded against.

*And the counter-case does not transfer.* RefChecker's Neutral requires the text to **yield a
claim-triplet** that the reference then fails to verify. Our degenerate text yields **no extractable
claim at all** — "the presenter draws a light light light … line" produces zero facts — so it would be
invisible to RefChecker too, for the same structural reason it is invisible to us. **No system found
prices text that yields no claims.**

*A directly transferable mechanism was found, on a different axis.* Both VeriScore and SAFE price
**volume without a reference**, via `R_K = min(S/K, 1)` where K is "the median number of extracted
facts from each response in each domain" — a corpus-derived target, not a ground truth. And the known
failure of precision-only metrics is documented in the same literature: FActScore "does not penalize a
model that abstains from responding too frequently or generates fewer facts", so precision can be
gamed by **saying less**. **Our failure is the exact inverse — precision is gamed by saying more
unextractable text — which is the same axis in the opposite direction, and argues that the fix belongs
on a volume term rather than inside precision.** That is a direct input to Q2 (Round 3).

*Recommendation:* price the fabricated bulk as a **separate, volume-oriented statistic**, not as a new
fact kind and not inside `precision`.
- **Status: convention** — three independent cited systems agree on excluding unverifiable content
  from precision; the volume-term pattern is cited in two of them (VeriScore, SAFE).
- **Why:** every system found keeps precision to what it can verify; the one system that folds
  unverifiable content in still requires an extractable claim, which our failure mode does not produce.
  Departing from convention *inside* precision would break both external legibility and the 11
  historical arm figures at once.
- **Risks accepted:** a separate statistic can be ignored by a reader, which is the present failure
  mode restated. Q2 must decide what makes it unignorable.

---

**Q4: What is the migration and comparability rule for a metric change over an already-scored corpus?**

*Options considered:*

- **Option A: an integer metric version** on the summary. Pros: trivial. Cons: records *that* the
  metric changed, not *what settings produced a number* — two runs at the same version but different
  `number_tolerance` still compare falsely, which is a live risk here (`rescore` accepts `--tolerance`,
  `--min-persist`, `--min-height` overrides).
- **Option B: a sacreBLEU-style signature string.** Post, *A Call for Clarity in Reporting BLEU
  Scores*, WMT 2018 (`arxiv.org/abs/1804.08771`), and the reference implementation
  (`github.com/mjpost/sacrebleu`). The signature is a compact parameter string —
  `BLEU|nrefs:1|case:mixed|eff:no|tok:13a|smooth:exp|version:2.0.0` — and the governing rule is that
  **"two scores computed through SacreBLEU with an identical signature are comparable."** It exists
  because "papers vary in the hidden parameters and schemes they use, yet often do not report them."
  Pros: solves exactly Q4(iii) — a reader cannot silently compare across versions, because the
  mismatch is visible in the score's own signature; proven and widely adopted across a whole field.
  Cons: more to implement and to keep honest than an integer.

*Disconfirming evidence sought:* I looked for criticism of signature-based comparability. The
substantive critique found is not of the mechanism but of adoption — a meta-evaluation of 769 MT papers
(`arxiv.org/abs/2106.15195`) reports that comparability fails when authors do not use the standard tool
at all. That is an argument for **emitting the signature automatically inside the scorer** rather than
relying on a caller to record it, which aligns with Phase 1 constraint 6.

*Recommendation:* **Option B**, with the signature emitted **inside `score_segments`**, not patched in
by the caller.
- **Status: proven** — deployed field-wide in MT for eight years, with a cited primary source for the
  rule and a reference implementation.
- **Why:** it answers all four parts of Q4 with one mechanism, and it is the only option that survives
  Phase 1 constraint 6: `score_segments` hardcodes `ocr_grounded: false` (`fidelity.rs:606`) and only
  `pipeline.rs:387` patches the true value in, so **any field set outside the scorer is silently
  dropped on every re-score.** A signature built where the score is built cannot be dropped.
- **Consumer cross-check:** PR-030's criterion "all arms scored under a single, named metric version"
  is satisfiable — all 11 arms re-score bit-exact today (Phase 1), so re-scoring them under a signed
  metric is offline and mechanical. PR-031's criterion is unaffected by Q4.
- **Risks accepted:** the existing `ocr_grounded` drop is a defect of the same shape; whether this PR
  repairs it or records it separately is still Q4(iv)'s open sub-decision, carried to Round 3.

---

**Escalation check against the Phase 2 pre-registered triggers:**

- **Q3 — trigger did NOT fire. Resolved at Tier A.** The trigger was "only secondary sources, or
  nothing addressing partial reference coverage". Three primary sources address it directly, they
  converge, and a counter-case was found and shown not to transfer.
- **Q4 — no trigger; resolved at Tier A** with a proven mechanism and a primary source.
- **Q1 — the trigger FIRED, on a technicality that may not be worth paying for.** The trigger was "no
  statistic with a **published operating point on long-form generation**". `seq-rep-n` has published
  *reference values* (human 0.006 vs degenerate 0.442) but **no published threshold for flagging a
  single output**. Flagged rather than silently resolved, per the escalation rule. **Recommendation:
  do NOT escalate to Tier B** — the threshold was always going to be measured on this corpus
  (PR-025 and PR-028 both set theirs that way, and `docs/ARCHITECTURE.md` records both separations as
  in-repo measurements), so a deeper web round would be buying a number this project does not use.
  **Operator decision required before Round 2.**

### Findings — Phase 3, Round 2 (2026-08-26)

Internal measurement, no web round, per the Phase 2 plan: **Q1b** (threshold measurement) and **Q5**
(does PR-025's compression-ratio rejection extend). **The Round 1 recommendation did not survive.**

**Pool reproduced exactly, as a correctness check.** 151 candidate files across the three locations →
114 timeline-shaped → **95 distinct by content SHA-256** (19 appear in more than one pool) → 4 excluded
by the Phase-1 pre-guard marker (`>=3 verbatim repeats of a >=15-char sentence`) → **91 timelines /
11,118 visual segments**. Removing the single job that did not exist when PR-028 swept —
`046cd326`, created by PR-028's own deployment, 10 visual segments — gives **90 timelines / 11,108
visual segments, matching PR-028 exactly.** The pool is therefore PR-028's, plus the one job that
contains two of the four confirmed degenerate segments. All measurement below is on the full 11,118.

**Tokenizer, stated explicitly** (per PR-025's and PR-028's recorded lesson that the tokenizer decides
the answer): **no word tokenizer exists in the codebase** — `vision.rs` defines only a numeric token
(`number_end`) and a sentence split. The word tokenizer used here mirrors `fidelity.rs`'s own
`extract_facts_with`: split on whitespace, `/` and `|`, then trim the `strip_punct` character set,
dropping empties; lowercased. This is the closest thing to a shipped word tokenizer in this repo and is
the one PR-031 will have to adopt or explicitly diverge from.

---

**Q1b: measured — and `seq-rep-n` fails on this corpus.**

*Sanity check first, and it passes:* median `seq-rep-4` over 11,118 segments is **0.0067**, against
Welleck et al.'s published **human baseline of 0.006**. Our ordinary output sits where human text sits,
which is independent corroboration that the port is sane.

*The confirmed degenerate segments do score high:*

| segment | chars | tokens | seq-rep-4 |
|---|---|---|---|
| `046cd326` vseg 6 (`light` x1,140) | 16,644 | 3,141 | **0.9226** |
| `84149f3b` vseg 7 (144 trend lines) | 15,905 | 3,101 | **0.7579** |
| `046cd326` vseg 8 (`near the end of` x215) | 13,701 | 2,749 | **0.7287** |
| `1ea5ab3a` vseg 21 (`"You're not."` x878) | 13,194 | 2,207 | **0.8017** |

All four found, all far above p99 (0.213). **But that is not enough, and three measurements kill it:**

1. **There is no gap.** **113 of 11,118 segments** sit continuously across `[0.20, 0.75)` with no
   discontinuity anywhere. PR-025 had legitimate topping at **38** against a degenerate floor of
   **166**; PR-028 had **13** against **143**. Both were clean. This is a continuum, and every cap in
   the band cuts legitimate content or misses degenerate content.
2. **It does not track fabrication where fabrication is visible.** Over the **125** segments that have
   OCR ground truth and >=5 stated facts, the Pearson correlation between `seq-rep-4` and
   OCR-verified precision is **+0.011** — no relationship. Two cases show why:
   - `84149f3b` vseg 8: `seq-rep-4` **0.4736**, precision **0.960**. Highly repetitive *and* highly
     accurate. Any cap that catches the degenerate band flags this segment.
   - `2fc10c93` vseg 6: precision **0.158** — the worst measured fabricator in the corpus, 284 stated
     facts of which 239 are unsupported — scores only **0.3988**, mid-band.
3. **Word redundancy runs the wrong way.** `1 − unique/total` correlates with precision at **+0.268** —
   *more* redundant output is *more* accurate on this corpus, not less.

*The Round 1 open gap is now closed with a measurement.* `seq-rep-4` **is** weakly length-sensitive:
among ordinary segments the median rises **0.0043 → 0.0084 → 0.0240** across the 250–500, 500–1,000 and
1,000–2,000 token bands, with Pearson **r = 0.194** against `log(tokens)`. Weak, but real, and in the
direction that inflates long segments — which is the population of interest. (Segments under ~100
tokens are also unstable: only 6 exist and their p50 is 0.264, so any use would need a minimum-token
gate, as `min_skeleton_chars` does for PR-028.)

*A structural distinction emerged from the measurement, and it reframes the PR.* There are **two**
fabrication modes, not one:

- **Mode A — fabrication the metric already sees.** Many extractable facts, most unsupported.
  `2fc10c93` vseg 6: 284 facts, precision 0.158. **The metric prices this correctly today.** It is not
  PR-029's problem.
- **Mode B — fabrication the metric cannot see.** Enormous text yielding almost no extractable facts.
  The three confirmed segments, at 489–757 chars per fact. **This is the whole of PR-029's problem.**

Crucially, `046cd326` vseg 6 — the worst degenerate segment in the corpus — has precision **0.909** on
the 22 facts it does yield. Its *facts are right*; its **bulk** is fabricated. Any instrument aimed at
the wrongness of extracted facts will therefore miss it by construction, which is a sharper statement
of the PR's premise than the Motivation currently makes.

*Recommendation:* **`seq-rep-n` is REJECTED for PR-029.** Status: **proven unsuitable — by direct
measurement on 11,118 segments and 125 ground-truth segments.**
- **Why:** it measures repetition; repetition and fabrication are near-orthogonal here (r = 0.011). It
  would flag a 0.960-precision segment and under-flag a 0.158-precision one.
- **Carried to PR-031, not discarded.** As a *repetition* detector it is exactly what PR-031 is for,
  and this round supplies PR-031 with a measured distribution, a stated tokenizer, and a length caveat.
  PR-031 should be told the p99 is **0.213**, not the p99-of-8 word-run figure quoted from a Python
  probe in its Motivation.

---

**A candidate emerged from the measurement, and it is the one Phase 1 rejected.**

**Chars per stated fact**, gated on a minimum fact count, ranks the confirmed cases correctly:

| rank | segment | facts | chars | chars/fact | label |
|---|---|---|---|---|---|
| 1 | `046cd326` vseg 6 | 22 | 16,644 | **757** | confirmed degenerate |
| 2 | `84149f3b` vseg 7 | 25 | 15,905 | **636** | confirmed degenerate |
| 3 | `046cd326` vseg 8 | 28 | 13,701 | **489** | confirmed degenerate |
| 4 | `95f4bc52` vseg 1 | 11 | 3,934 | 358 | precision **1.000** |
| 5 | `fd9533a2` vseg 8 | 16 | 5,561 | 348 | precision **1.000** |

With a `>=10` fact gate the three confirmed degenerate segments are **the top three, in order**, above
a legitimate ceiling of 358 — a gap of 489 → 358.

**Phase 1 rejected this statistic on evidence that does not survive inspection.** The objection was
that `95f4bc52` vseg 5 (1,019 chars/fact) and vseg 4 (784) outrank the degenerate segments. Those
segments have **3 and 5 stated facts** respectively — the ratio is division by a near-zero denominator,
not a signal. A minimum-fact gate, exactly analogous to PR-028's `min_skeleton_chars`, removes both.

**This is a candidate, not an answer, and three things block it from being one:**

1. **The gap is 1.37x** (489 → 358), against PR-025's 4.4x and PR-028's 11x. Narrow enough that three
   more labelled points could close it.
2. **The positive class is 3 segments.** A threshold fitted to three points is not measured, it is
   memorised.
3. **It is OCR-dependent, so it is measurable on 127 of 11,118 segments — 1.1% of the pool.** This is
   precisely the substrate constraint Phase 2 flagged in advance. **Its threshold cannot be set the way
   PR-025's and PR-028's were**, because 98.9% of the pool has no denominator.

---

**Q5: Does PR-025's compression-ratio rejection extend to the statistic Q1 selects?**

*Answer: it extends to `seq-rep-n`, and it does not reach `chars per fact`.* Status: **proven for the
first, not applicable to the second.**
- `seq-rep-n` is a redundancy statistic of the same family as compression ratio, and it fails the same
  way and worse: PR-025 recorded compression ratio flagging **12.5% of clean segments at 2.4 while
  missing 3 of 18** degenerate ones. `seq-rep-4` has **no operating point at all** that separates the
  classes on this corpus — 113 segments spread continuously through the band, and a 0.960-precision
  segment sitting inside it. **The rejection extends; the family is closed for this purpose.**
- `chars per stated fact` is **not** a redundancy statistic — it is a yield ratio between output volume
  and verifiable content, and it does not measure repetition at all. PR-025's rejection does not reach
  it, so adopting it is not re-opening a closed decision.

---

**Round 2 outcome: Q1 is NOT resolved, and the blocker is not research depth.**

Flagged rather than settled, per the escalation rule. The Tier-A candidate failed under measurement; a
better candidate emerged *from* the measurement but cannot be validated, because **the corpus has 3
labelled positives and no ground truth outside the 127 OCR'd segments.** Escalating Q1 to Tier B or C
would buy more literature, and literature is not what is missing — **labels on this corpus are.**

That is the same wall PR-023 hit: its κ calibration stalled on the review-sheet interaction, the κ half
moved to PR-027, and PR-027 was deleted. **The blocker has circled back to the deleted PR.** Naming
that plainly is this round's most useful output, and the decision it forces belongs to the operator,
not to a deeper web round.

### Findings — Phase 3, Round 3 (2026-08-26)

**Q2: Where does the signal live — inside `precision`, as a separate field on `FidelitySummary`, or
outside the summary entirely?**

**Operator decision, 2026-08-26:** ship the yield statistic as a **reported diagnostic with no
threshold**. Rationale accepted from Round 2: three labelled positives cannot support a cutoff, and
making Mode B *visible* is what the resolved PR-030 contract actually asks for. Round 3 answers where
it goes and what stops it being ignored.

*Options considered:*

- **Option A — fold into `precision`.** Rejected. Round 1 found three independent systems (VeriScore,
  SAFE, RefChecker) that keep precision to what can be verified, and it would silently redefine all 11
  historical arm figures — the one thing PR-030 needs preserved.
- **Option B — a separate field on `FidelitySummary`.** Selected.
- **Option C — outside the summary.** Rejected: the summary is what gets quoted; a number outside it
  reproduces the present failure, where the diagnostic exists and nobody reads it.

*The statistic must be a concentration measure, not a mean — measured, not assumed.* A mean yield ratio
fails at job level for the same reason it failed at segment level in Phase 1. Over the 14 scored jobs:

| arm | median chars/fact | text-weighted median | **ratio** | text in worst segment |
|---|---|---|---|---|
| **v3.1 `2024_6_24`** (`046cd326`) | 48 | 489 | **10.17** | 38.3% |
| **v2 `2024_6_24`** (`84149f3b`) | 73 | 172 | **2.37** | 41.5% |
| general `2024_6_24` | 193 | 219 | 1.14 | 13.4% |
| general `2025_05_26` | 230 | 229 | 1.00 | 7.5% |
| **general `2024_4_8`** | **310** | 263 | **0.85** | 9.4% |
| *(nine further arms)* | 33–188 | 33–188 | 0.87–1.06 | 5–18% |

**The two arms carrying a confirmed degenerate segment are the only two above 2.0**, at 10.17 and 2.37,
against 0.85–1.14 for every other arm. And the arm with the **worst absolute** chars-per-fact — the
general prompt on `2024_4_8`, at 310 — has a ratio of **0.85**, because it is uniformly prose-heavy
rather than concentrated. That is Phase 1's objection resolved by construction rather than by a gate:
**a mean flags verbose-but-honest output; a concentration ratio does not.**

The statistic is **parameter-free** — a text-weighted median against an unweighted one, with no
threshold, no top-N and no cutoff — and computable entirely from data `fidelity.json` already stores
(`SegmentFidelity.stated` plus segment length), which is the finding rescued from the deleted PR-027.

*Verification criterion 2 is satisfied, demonstrated numerically.* This PR's own criterion — "Removing
PR-028's 12,236 fabricated characters from `84149f3b` now moves the score" — tested directly:

| | precision | concentration ratio |
|---|---|---|
| before the guard | 0.9065 | **2.37** |
| after removing 12,236 fabricated chars from vseg 7 | **0.9065** (bit-identical) | **1.50** |

**The exact edit that moved not one fidelity number moves this statistic by 37%.**

*What stops it being ignored* — the risk Round 1 flagged explicitly. **`ocr_grounded` is the precedent,
in this very struct:** a `FidelitySummary` field whose whole job is to tell a reader how to read the
other numbers, backed by a log line that says so. The yield statistic takes the same shape and the same
log line as `precision`, so a precision figure never travels without the number that says how much text
produced it. This is a convention already accepted in this codebase, not a new mechanism.

*Recommendation:* **Option B — a separate, parameter-free concentration field on `FidelitySummary`,
reported beside precision, with no threshold.**
- **Status: convention** for the placement (three cited systems in Round 1 keep precision clean);
  **proven** for the statistic's behaviour on this corpus (14 arms measured, and the PR's own
  verification criterion demonstrated above).
- **Risks accepted:** two labelled positives drive the separation, so the ratio is a *pointer for a
  reader*, not a classifier — nothing may be auto-rejected on it, and no threshold may be quietly
  introduced later without the labels Round 2 showed are missing.

*Consumer cross-check (mandatory before marking resolved):*

- **PR-030** — criterion "all arms scored under a single, named metric version, reported as a
  guardrail". **Satisfied**: precision, recall and F0.5 are untouched, so all 11 historical figures
  stay readable, and the new field flags that v3.1's 0.863 precision on `2024_6_24` was computed over
  text 38.3% concentrated in one low-yield segment. That is exactly the guardrail reading PR-030 needs
  and could not previously get.
- **PR-031** — criterion "a threshold validated against a metric that cannot see the failure is not
  validated". **Satisfied**: the 37% move above is the demonstration that a guard truncating fabricated
  bulk now shows up in a reported number. PR-031 can argue its residue trade-off in a term that
  responds, which PR-028 explicitly could not.
- **No other PR depends on this one** (Phase 1 sweep).

---

**Phase 3 complete.** Q1 resolved by rejection and replacement (`seq-rep-n` rejected on measurement;
yield-concentration adopted as a no-threshold diagnostic); Q2 resolved above; Q3 and Q4 resolved in
Round 1; Q5 resolved in Round 2. **No escalation above Tier A was spent.** The one question that could
not be closed by research — a validated *threshold* — was closed by a scope decision instead, on the
finding that the missing input is labels on this corpus rather than literature.

### Group D: MCP Verification (2026-08-26)

Run against the load-bearing, identifier-specific claims before locking Phase 4. Pure-methodology
claims (e.g. "concentration beats a mean") are out of scope and were settled by measurement in Round 3.

**Probe 1 — Schema-Integrity.**

| Claim | Identifier | Canonical documenter | Verified? |
|---|---|---|---|
| new field lands on the summary | `FidelitySummary` | `vtt-core/src/types.rs:75-90` | yes — 10 fields, no version/signature field exists |
| precedent for a "how to read this" field | `ocr_grounded` | `vtt-core/src/types.rs:88-89`, set at `pipeline.rs:387` | yes |
| statistic computable from stored data | `SegmentFidelity.stated` | `vtt-core/src/fidelity.rs:468-476` | yes — per-segment, already persisted to `fidelity.json` |
| the offline re-score entry point | `run_rescore` | `vtt-client/src/review.rs:282-318` | yes |
| CLI binary name | `vtt-client` (**not** `vid-to-text`) | `vtt-client/Cargo.toml:2` | yes — corrected; the documented name exits 127 |

**Probe 2 — Synthesis-Verification.** The Round 1 recommendation combines two elements: *a
sacreBLEU-style signature* **and** *emitting it inside the scorer rather than in the caller*. Round 1
cited each separately, which is the synthesis failure mode this probe exists to catch. Verified
against the reference implementation: `sacrebleu/metrics/bleu.py` declares `_SIGNATURE_TYPE =
BLEUSignature` on the metric class itself, and the metric constructs `self.tokenizer_signature =
self.tokenizer.signature()` during initialisation — **the metric owns its signature; no caller builds
it.** The exact combination is therefore cited, not synthesised. Status upgraded to **proven**.

**Probe 3 — Binding-at-creation (live-state).** Phase 1 *inferred* from reading that a summary field
set outside `score_segments` is dropped on re-score. Demonstrated live rather than argued: a copy of
`046cd326`'s timeline was edited to claim `ocr_grounded: true`, then re-scored with the release binary.

```
stored timeline claims ocr_grounded = True
rescored summary   ocr_grounded = False      <-- silently dropped
precision          = 0.8627                   <-- bit-identical, so the score itself round-trips
```

**Confirmed: the drop is real and silent.** This is why the signature must be constructed inside
`score_segments`, and it converts Q4(iv) from a design preference into a demonstrated requirement.

---

### Synthesis (2026-08-26)

**Outcome: Amend** — the PR's drafted approach (score fabricated non-numeric content as facts, then set
a threshold) is replaced: research closed the fact-scoring route and measurement rejected the
threshold. **Both amendments were approved by explicit user decision on 2026-08-26** — first to drop
PR-030's ranking rather than re-open κ, then to ship a reported diagnostic with no threshold.

**Changes to this PR** from research:

- **No new fact kind.** Q3 found three independent systems (VeriScore, SAFE, RefChecker) that keep
  precision to what can be verified; PR-023's Trend/Magnitude exclusion stands unamended. The Scope's
  "an additional fact kind" option is removed.
- **`seq-rep-n` rejected**, by measurement over 11,118 segments: no gap (113 segments spread
  continuously through `[0.20, 0.75)`), r = +0.011 against OCR-verified precision, and it flags a
  0.960-precision segment while under-flagging the 0.158-precision one. Carried to PR-031 as a
  *repetition* detector, which is what it actually measures.
- **The statistic is a parameter-free yield *concentration*, not a mean** — text-weighted median
  chars-per-fact against the unweighted median. A mean was rejected on measurement: the general prompt
  on `2024_4_8` has the worst absolute chars-per-fact (310) and a concentration ratio of 0.85.
- **No threshold, by decision.** Two labelled positives cannot support a cutoff. The field is a
  pointer for a reader; nothing may be auto-rejected on it.
- **Migration is a signature, not a version integer**, emitted inside `score_segments` (Group D
  Probe 2 + 3). `rescore`'s `--tolerance` / `--min-persist` / `--min-height` overrides mean a bare
  version number cannot establish comparability.
- **Two factual corrections** carried into the PR body: the headline delta is **1.53 points**, not 1.6
  (re-derived; also corrected in `ROADMAP.md`, `RESEARCH-BACKLOG.md` and `PR-030`); and the re-scoring
  surface is **11 arms that all have `ocr.json`**, not "44 stored timelines" — 13 of 44 are scorable at
  all, but every arm is among them.

**Changes to `docs/ARCHITECTURE.md`:** **none in this commit, deliberately** — following PR-028's
recorded precedent. § Fidelity Diagnostic must gain the new summary field and the signature, but that
documents behaviour which does not exist yet, and `docs/CONSTRAINTS.md` forbids phantom
implementations. The edit is carried as a verification criterion so it lands in the implementation
commit, alongside the code it describes.

**Changes to `docs/CONSTRAINTS.md`:** none. No new hard rule. The "no threshold without labels"
condition is a risk recorded against this PR, not a project-wide invariant, and the rule that already
governs the surface — § Review's "the metric is not trusted for tuning until that κ has been reported"
— is unchanged and was the constraint that reshaped PR-030.

**New PRs that must come first:** **none.** One follow-up is *recorded, not opened*, matching PR-028's
handling: **the review-sheet interaction blocks κ, κ blocks any future threshold on this statistic, and
that chain now has no owner.** It does not block this PR, which ships without a threshold by design.

**Research-backed details now locked in this PR:**

- Precision, recall and F0.5 are **not** redefined — three cited systems, and PR-030's need for the 11
  historical figures to stay readable.
- The reported statistic: text-weighted median chars-per-stated-fact ÷ unweighted median, per job,
  parameter-free, computed from data `fidelity.json` already persists.
- Placement: a new field on `FidelitySummary`, reported in the same log line as precision, modelled on
  `ocr_grounded` — the existing in-struct precedent for a field that tells a reader how to read the
  others.
- Comparability: a sacreBLEU-style signature constructed **inside `score_segments`**, verified against
  the reference implementation and against a live demonstration of the drop it prevents.
- Migration: all **11** arms re-scored offline with `vtt-client rescore`; the 31 non-arm timelines have
  no `ocr.json` and are marked un-rescorable rather than silently omitted.


### Gate Check (2026-08-26)

- **Premise still valid: ✓ — and sharpened by the research.** The PR's claim was that the metric cannot
  see majority-by-volume fabrication. That held under measurement, and Round 2 made it more precise:
  `046cd326` vseg 6 scores **precision 0.909** on the 22 facts it yields while 16,644 characters of its
  output are fabricated. The facts are right; the bulk is not. No instrument aimed at the wrongness of
  extracted facts can see that, which is why this PR reports a yield term instead.
- **No prerequisite PRs surfaced: ✓.** One follow-up recorded, not opened: the review-sheet interaction
  blocks κ, κ blocks any future threshold on this statistic, and that chain has no owner. It does not
  block this PR, which ships without a threshold by design.
- **Scope changes since drafting:** substantial, and both approved by explicit user decision on
  2026-08-26 — PR-030's ranking dropped (rather than re-opening κ), and this PR shipping a reported
  diagnostic with no threshold. See § Synthesis.
- **Downstream contracts re-checked after the amendment: ✓.** PR-030 gets a guardrail that flags
  v3.1's `2024_6_24` concentration at 10.17; PR-031 gets a term that moves 37% on the exact edit that
  moved no fidelity number. Both verified numerically in Round 3.
- **Risks accepted:** the separation is driven by **two** labelled positives, so the ratio is a pointer
  for a reader and not a classifier; and the 31 non-arm timelines can never be re-scored (no
  `ocr.json`), so they are reported as such rather than silently dropped.
- **Research spend:** Tier A throughout. No escalation to B/C/D/E; the one question research could not
  close (a validated threshold) was closed by a scope decision, on the finding that the missing input
  is labels on this corpus rather than literature.
- **User approved updated spec: ✓ (2026-08-26)**
- **Implementation cleared: ✓ (2026-08-26)**

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
1.53 points**, from 0.8627 to 0.8780. (Re-derived 2026-08-26; the PR originally said 1.6.)

**Why this blocks the project's objective.** The stated goal is quality per hour of compute, and
`docs/0.0/RESEARCH-BACKLOG.md` records that PR-023's F0.5-per-GPU-hour objective was never exercised
and has no owner. Neither that objective nor any prompt A/B can be trusted while the numerator is
blind to majority-by-volume fabrication. Every figure in PR-026's prompt table carries this
contamination to an unknown degree.

## Scope

**Settled by the research round (2026-08-26); this section states outcomes, not options.**

**In scope:**
- **A yield-concentration field on `FidelitySummary`**, making Mode-B fabrication — enormous text
  yielding almost no extractable facts — visible in the summary that gets quoted. Defined as the
  **text-weighted median chars-per-stated-fact divided by the unweighted median**, per job.
  Parameter-free: no threshold, no top-N, no cutoff. Computed from data `fidelity.json` already
  persists (`SegmentFidelity.stated` plus segment length).
- **Reported beside `precision` in the same log line**, modelled on `ocr_grounded` — the existing
  field in this struct whose job is telling a reader how to read the other numbers.
- **A sacreBLEU-style comparability signature, constructed inside `score_segments`.** Not a version
  integer: `rescore` accepts `--tolerance` / `--min-persist` / `--min-height`, so same-version scores
  can still differ. Building it inside the scorer is a demonstrated requirement, not a preference —
  Group D Probe 3 shows a field set outside is silently dropped on every re-score.
- **Re-scoring all 11 prompt-A/B arms** offline with `vtt-client rescore`. Verified 2026-08-26: all 11
  have `ocr.json` and re-score bit-exact, so this costs no GPU time.

**Explicitly out of scope** (each closed by the research, not deferred by preference):
- **A new fact kind for non-numeric claims.** Q3: three independent systems keep precision to what can
  be verified; PR-023's Trend/Magnitude exclusion stands.
- **Any threshold on the new statistic.** Two labelled positives cannot support a cutoff. The field is
  a pointer for a reader; nothing may be auto-rejected on it, and no threshold may be added later
  without the labels Round 2 showed are missing.
- **Redefining `precision`, `recall` or `f05`.** PR-030 needs the 11 historical figures readable.
- **`seq-rep-n` and the redundancy family.** Rejected on measurement (Round 2); carried to PR-031 as a
  repetition detector, which is what it measures.
- Guarding or truncating the fabricated text. This PR measures; PR-031 guards.
- Changing frame sampling, prompts, or any capture parameter.
- Re-running any job on the GPU. Re-scoring is offline.

## Dependencies

- **PR-023** — the fidelity diagnostic this extends. Landed `f570aec`.
- **PR-028** — supplied the measurement that proves the blindness. Landed `82df218`.

## Architecture section implemented

`docs/ARCHITECTURE.md` — § Fidelity Diagnostic.

## Verification criteria

Rewritten 2026-08-26 from the research round; the originals assumed a fact-scoring metric with a
threshold, and both were closed by the findings.

- [ ] The two arms carrying a confirmed degenerate segment report a concentration ratio **above 2.0**
      (measured: 10.17 and 2.37) while every other arm reports **below 1.2** (measured: 0.85–1.14) —
      pinned by a test over the stored arm data
- [ ] The **general prompt on `2024_4_8`**, which has the worst absolute chars-per-fact (310) and is
      verbose-but-honest, is **not** flagged — its ratio is 0.85. A mean-based statistic must not be
      substituted for the concentration ratio without re-checking this case
- [ ] Removing PR-028's 12,236 fabricated characters from `84149f3b` moves the reported statistic
      (measured: 2.37 → 1.50, a 37% drop) while `precision` stays bit-identical at 0.9065
- [ ] Segments with too few stated facts do not produce a spurious ratio — the denominator guard is
      explicit and tested (`95f4bc52` vseg 5 has **3** facts and 1,019 chars/fact; it is not a signal)
- [ ] A comparability signature is emitted **by `score_segments` itself**, and survives a `rescore`
      round-trip — pinned by a test that would fail under the `ocr_grounded` pattern
- [ ] All 11 arms re-scored under one signature; the 31 timelines without `ocr.json` are reported as
      un-rescorable rather than silently omitted
- [ ] `docs/ARCHITECTURE.md` § Fidelity Diagnostic documents the new field and the signature, **in the
      implementation commit** (not before — documenting unimplemented behaviour is a phantom
      implementation)
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
