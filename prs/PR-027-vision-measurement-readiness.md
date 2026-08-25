# PR-027: Vision measurement readiness

<!-- Landed-in: set to the released version this PR shipped under (e.g. v0.1.0).
     Use "(not yet landed)" for in-flight or dormant PRs.
     Use "superseded by PR-XXX" for replaced PRs.
     See docs/VERSIONING.md §4 for the policy. -->
**Landed-in:** (not yet landed)

**Path tier:** Tier-1 (the research that created this PR is already done — see Research findings; a
Phase 1 state assessment is still required before implementation)

**Numbering note.** This PR carries a higher number than PR-026 but **lands before it**. The repo
expresses order through the `Depends on` column of `docs/0.0/ROADMAP.md`, not through PR numbers —
PR-021 is likewise unimplemented while PR-022..PR-026 came after it. PR-026 was split on 2026-08-25
after its research round; the measurement half became this file so that neither half violates
**One PR, One Thing**.

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

**This PR was created by PR-026's research round (2026-08-25) and inherits it.** The full record —
Phase 1 state assessment, Phase 2 scoping, Phase 3 Tier-A and Tier-C rounds, and the Phase 4 Amend
decision that produced this split — lives in
`prs/PR-026-content-specific-vision-prompt.md` § Research findings. The findings that created **this**
PR are summarised under `## Research backing` below.

**Still required before implementation:** a Phase 1 state assessment of its own, per the
time-decay rule and because this PR touches code the inherited assessment only surveyed
(`fidelity.rs` scoring internals, `review.rs`, the deploy scripts).

---

## Motivation

**PR-026 cannot tune a prompt with an instrument that has not been shown to measure the right thing.**
Its research round established four defects in the current measurement path, each with a cited basis:

1. **F0.5 systematically prefers the terser arm.** `fidelity.rs` uses
   `f05(p,r) = 1.25pr/(0.25p+r)`. Against that very formula: an arm at precision 0.80 / recall 0.40
   scores **F0.5 0.6667 but F1 0.5333**, while an arm at 0.60 / 0.60 scores **0.6000 under both** —
   a rank reversal produced purely by β, on exactly the precision/recall trade that a change in
   prompt verbosity induces. PR-023 chose F0.5 for a **sampling** study, where verbosity was not the
   manipulated variable. Both published metrics that solved the length-comparison problem — OVFact
   (`arXiv:2507.19262`) and CAPTURE (`arXiv:2405.19092`) — chose **β = 1**.

2. **Scoring is micro-averaged, so there is no variance estimate.** `score_segments` accumulates
   `stated`/`supported` across every segment and returns one pooled precision and recall, so a long
   or segment-dense video dominates the figure and a single number per arm cannot support a
   confidence interval. CAPTURE §5.1.1 treats per-sample rank agreement as *"the most important
   metric for consistency evaluation"*; with two variants that degenerates to a paired sign test over
   videos — which is exactly the instrument PR-026's stopping rule assumes and currently has no
   source for.

3. **The metric has never been validated against human judgment.** `docs/ARCHITECTURE.md` already
   says *"The metric is not trusted for tuning until that κ has been reported"* and
   `docs/0.0/DESIGN-log.md` decision 4 says *"nothing ships without the κ"* — and PR-023's calibration
   was done without it. The research round found this is **the field's standard precondition**, not a
   local rule: every metric surveyed ships a human meta-evaluation as its central claim.

4. **The most damaging known defect is invisible to the metric.** In `2024_4_8` the model narrated
   TradingView's hovered-candle header as a price time series. Every number in that fabrication is
   genuinely on screen, so the segment scored **precision 0.883**. A measure that cannot see this
   cannot be the objective of a tuning loop aimed at it.

Two further gaps block measurement for a different reason — a prompt cycle currently cannot be shown
to have measured the prompt it claims to have measured. `CaptureInfo` records no prompt identifier,
and **nothing deploys `prompts/` to the GPU host**: `config/deploy-profiles.sh` copies `*.toml` only,
and `~/vid-to-text` on the desktop is an unversioned file copy (verified 2026-08-25:
`fatal: not a git repository`).

## Scope

**A. Prompt provenance.** A prompt identifier and content hash in `CaptureInfo`, so two captures made
under different prompts are distinguishable in the data. Must be `Option` or `#[serde(default)]` —
timelines carrying a `capture` block already exist on the desktop and are read by `review`/`rescore`.

**B. Prompt deployment.** A checksum-compare / `--apply` / verify-after path for `prompts/`, modelled
on `config/deploy-profiles.sh`, so what runs on the GPU is known to be what is in the repo.

**C. Configurable β, defaulting unchanged.** Expose the F-score β under `[fidelity]`. **PR-023's
F0.5 remains the default** so no existing behaviour or recorded figure changes; PR-026's Phase 6 sets
β = 1. Both `f05` and the general form pinned by tests.

**D. Per-video paired scoring.** Report per-video (and per-segment) precision/recall/F alongside the
existing pooled summary, and a paired-difference test with a bootstrap CI over the same segments in
both arms. Pairing is valid because arms are segment-aligned by construction — verified: four
independent `clip900` runs produced bit-identical segment spans and frame lists, and `prominent`
counts are bit-identical across arms while `stated` moved 13×.

**E. The κ calibration study.** ~150 disagreement-first human judgments, using CHOCOLATE's
per-sentence error typology as the annotation template (Value / Label / Trend / Magnitude /
Out-of-context / Nonsense / Grammatical, of which this metric grounds three). Report Cohen's κ via the
existing `cohen_kappa` (`vtt-client/src/review.rs:127`), the matching-rule adjustments it implies, and
the resulting resolvable effect size. **κ ≈ 0.6 is the realistic target, not a disappointment** —
CHOCOLATE reaches Fleiss κ 0.63 with seven trained annotators, CAPTURE's best Sample τ is 0.6018, and
FaithScore's best human correlation is Pearson r 0.482.

**F. The cursor-hover chronology detector.** Flag a segment that asserts price movement between values
drawn from the chart header across frames, since the header reflects the *hovered* candle rather than
the passage of time. Buildable from artifacts already persisted — verified on this corpus:
`ocr.json` stores per-item `text`, `score`, `x`, `y`, `height_px`, and the header is captured as the
single topmost item (`y=135, x=26, h=26`), with the price axis 25 px lower at `x≈1855`.

**Explicitly out of scope:**
- Changing the vision prompt — that is PR-026, and it must not move while the instrument is being
  fixed.
- Changing sampling, whisper, or any PR-020/PR-022 locked dimension.
- Re-opening `use_transcript` or OCR grounding (measured worse and null respectively, PR-024).
- Re-tuning the numeric tolerance or prominence thresholds beyond what the κ study's matching-rule
  adjustments require.

## Dependencies

- **PR-022** — `Segment.frames` and `CaptureInfo`. Landed `c37e8a1`.
- **PR-023** — the fidelity diagnostic, `review`, `rescore`, and `cohen_kappa`. Landed `f570aec`.
- **PR-025** — the numeric-degeneration guard, so calibration is not contaminated by enumeration
  collapse. Landed `f45ce72`.

## Architecture section implemented

`docs/ARCHITECTURE.md` § **Fidelity Diagnostic** (β, paired scoring, κ status, the chronology
detector) and § **Capture Configuration** (prompt provenance as part of the operating point).

## Verification criteria

- [ ] `CaptureInfo` records a prompt identifier and content hash; a timeline made under a different
      prompt is distinguishable, and existing timelines carrying a `capture` block still deserialise
- [ ] `prompts/` deploys to the GPU host by checksum-compare with verify-after, and a mismatch is
      reported by name
- [ ] β is configurable under `[fidelity]`, defaults to 0.5 so no recorded figure changes, and both
      β = 0.5 and β = 1 are pinned by tests
- [ ] Per-video and per-segment precision/recall/F are reported alongside the pooled summary
- [ ] A paired-difference test with a bootstrap CI over segments is implemented and pinned by a test
      with a known-answer fixture
- [ ] `prominent` counts are asserted equal across two arms of the same video at identical sampling
      and fidelity config — the property the pairing relies on
- [ ] κ reported from ~150 human judgments; matching-rule adjustments applied and re-tested; the
      resolvable effect size recorded
- [ ] The chronology detector flags the known `2024_4_8` case, tolerates the `O`→`0` header misread,
      and degrades to "no header found" rather than mis-parsing; its false-positive rate is measured
      on a stated sample
- [ ] `cargo test --workspace` passes

## Research backing

**Tier-1, inherited from PR-026's round (2026-08-25), which included two Tier-C harness runs.**
Sources and epistemic status for each scope item:

- **C (β):** *proven* — the rank-reversal arithmetic is computed against the repo's own `f05`; β = 1
  precedent from OVFact (`arXiv:2507.19262`) and CAPTURE (`arXiv:2405.19092`).
- **D (paired scoring):** *proven* that the current code micro-averages and that arms are
  segment-aligned on this corpus; *convention* for extrapolating CAPTURE's metric-validation protocol
  to a prompt A/B.
- **E (κ):** *proven* — every metric surveyed ships a human meta-evaluation; ceilings from CHOCOLATE
  (Fleiss κ 0.63), CAPTURE (Sample τ 0.6018), FaithScore (Pearson r 0.482), ALOHa (20.30%).
- **F (detector):** *proven* that the required data exists and the header is isolable (verified on
  this corpus); *best-guess-given-constraints* for the false-positive rate, which is unmeasured until
  built. The literature names the defect class (VIDHALLUC's temporal sequence hallucination; ARGUS's
  ordering penalty) but every automatic detector found needs human reference captions or a constructed
  multiple-choice task, so nothing could be lifted.
- **A, B (provenance, deployment):** *internal* — established by Phase 1 state assessment against the
  live system, not by external research.

**Group D was waived** for the round that produced this PR (user direction, 2026-08-25). The
consequence is carried here: the scope-F detector combines an OCR-item schema, a header-parse rule and
a claim-extraction rule that **no cited source uses in combination**, and it therefore stays labelled
`best-guess-given-constraints` until it is built and measured.

## Notes

- Scope E is the expensive item and the one most likely to be cut. If it is, PR-026's Phase 6 must
  either adopt a primary measure that needs no κ or record an explicit amendment to
  `docs/ARCHITECTURE.md`'s tuning rule — the rule cannot simply be ignored.
- A hard operating rule discovered while verifying pairing: **`fidelity.rs` and its config must be
  frozen across arms, or every arm re-scored with one binary.** A third `clip900` job predating
  PR-023's tokenizer fixes has slightly different `prominent` counts, so the invariance that makes
  pairing valid holds only at a fixed metric version.
- Scope D's per-video reporting also gives the corpus runner something it lacks: a per-video quality
  figure that is comparable across runs.
