# PR-023: Visual fidelity metric and sampling tune

<!-- Landed-in: set to the released version this PR shipped under (e.g. v0.1.0).
     Use "(not yet landed)" for in-flight or dormant PRs.
     Use "superseded by PR-XXX" for replaced PRs.
     See docs/VERSIONING.md §4 for the policy. -->
**Landed-in:** (not yet landed)

**Path tier:** Tier-1 (research-backed at design time — `docs/0.0/DESIGN-log.md` session 2026-08-25)

## Before Implementation (NON-NEGOTIABLE)

This PR MUST NOT be implemented until `PROCEDURE-pr-research.md` has been completed in full and its output appended to the `## Research findings` section below.

**Tier-1 PRs** (research-backed at design time): Phase 1 (State Assessment) is required to catch drift. Phases 2-4 may be light if no drift is found.

**Tier-2 PRs** (research-pending): all 5 phases of `PROCEDURE-pr-research.md` must run before this PR is written in final form.

Skipping the PR research procedure is a hard violation of the research-backed-decisions constraint in `docs/CONSTRAINTS.md`.

## Research findings

### State Assessment (2026-08-25)

**Current state**:
- PR-022 is implemented and committed (`c37e8a1`): every visual segment carries `frames` (the
  capture timestamps it was generated from) and the timeline carries `capture`. Kept frames for a
  job remain on disk under `{temp_dir}/{job}/chunks/chunk_NNN/frames/` (verified for job
  `b73bdbf8`); they are not copied to the results dir, so they survive only until the job dir is
  cleaned.
- Server results live in `~/.vid-to-text/server/results/<job>/{timeline.json, job.json}`; 9.5 MB
  for 33 jobs. `job.json` records id/source/status only.
- No OCR tooling existed on the desktop. Installed during design (user-approved): `tesseract-ocr`
  4.1.1 via apt (for the comparison only) and **RapidOCR** in `~/.venvs/ocr` (user-space;
  `rapidocr` + `onnxruntime` 1.23.2, PP-OCRv6 det/rec small models). Python 3.10 and PIL 9.0.1
  present; no cv2, no paddle, no torch.
- Corpus scene-score sweep data for all 74 videos is at `~/vtt-scenes/*.txt` on the desktop; all 74
  source videos are staged at `/home/rux/vtt-corpus/`.
- Measured rates for the study budget (PR-022): adaptive 0.34–0.35x realtime at 1080p; fixed
  0.5 fps 1.14x (90 frames/chunk at 2.28 s/frame); fixed 0.25 fps 0.53x.
- Existing outputs usable as fixed-0.5 comparators: clip900 arms from PR-020 Phase 5.5 are under
  the locked config; the March cache timelines are NOT (pre-PR-020 config).

**Assumptions at design time**: that OCR could read chart axis text at 1080p — **measured true for
RapidOCR (76/76 legible axis values), false for tesseract 4.1.1 (42/76 with digit substitutions)**.
That ticker/timeframe/header text is equally readable — **unmeasured** (the header crops used for
ground truth were mis-cropped); covered by calibration.

**Stale assumptions**: none — the design session and this assessment are the same day.

**New constraints**:
- Frames must be preserved with results (thumbnails) or the metric cannot be re-run later; today
  they vanish with the job dir.
- OCR runs on CPU at ~1.3 s per 1080p frame: ~4 min per 40-minute video for kept frames (fine,
  overlappable), but the study's recall reference over every 2 fps candidate (~1,800 frames per
  15-minute clip) needs parallelism (32 cores available) or it dominates the study's wall time.
- Trend and Magnitude errors (CHOCOLATE) are not OCR-checkable; the metric must not claim them.

**Downstream contracts**: **none** — no PR depends on PR-023 (`grep -rn "PR-023" prs/ docs/`
finds only this file and the index rows). Upstream: PR-022 (provenance) is landed.

**Path-tier checkpoint**: Tier-1 (design research complete, recorded in
`docs/0.0/DESIGN-log.md` 2026-08-25); Phase 1 clean → skip to Gate Check.

### Gate Check (2026-08-25)

- Premise still valid: ✓ (provenance exists; OCR proven on this content)
- No prerequisite PRs surfaced: ✓
- Scope changes since design: none
- Implementation surface: see Scope
- Risks accepted: OCR misreads counted against the model until calibration says otherwise
  (mitigated by disagreement-first review); three videos may not separate close settings
  (reported as such, no winner from noise); ~2.4 GPU-hours for the study; ~1 hour of the user's
  review time
- User approved updated spec: ✓ (2026-08-25)
- Implementation cleared: ✓ (2026-08-25)

### Implementation Validation (2026-08-25, in progress)

**Built and tested.** `vtt-core/src/fidelity.rs` (fact extraction, precision rule, OCR wrapper
invocation, two-reference scoring, thumbnails), `[fidelity]` config, per-job diagnostic in the
pipeline (isolated in its own task), `ocr.json` persistence, `/health` + `doctor` OCR probe,
`vid-to-text review` (sheet + κ) and `vid-to-text rescore` (offline re-scoring against a candidates
reference or other parameters). 221 tests pass. Study profiles `study-t{05,08,12}-g{15,30}` and
`study-fixed05` generated from the locked profile and deployed; study clips (minutes 5–20 of
`2024_2_19`, `2024_6_24`, `2025_05_26`) cut on the desktop.

**End-to-end results (market-research profile, RapidOCR, `kept` reference):**

| run | video | segments | stated | supported | precision | recall | F0.5 | diagnostic time |
|---|---|---|---|---|---|---|---|---|
| `cc423d58` | clip900 | 9 | 756 | 171 | 0.226 | 0.259 | 0.232 | 166 s |
| `9dcbd12e` (re-scored) | `2024_4_8` (31 min) | 17 | 460 | 406 | **0.883** | 0.279 | 0.616 | 320 s |

**What the metric found on the first run.** clip900's 0.226 is one segment: the model degenerated
into a counting sequence ("1.801, 1.802, … 1.877" — 568 stated facts, 554 unsupported) that
sentence-level `truncate_repetition` cannot catch. Excluding it, precision is 157/188 = 0.835.
Recorded in `docs/0.0/RESEARCH-BACKLOG.md` as a vision-side gap; the metric is doing its job.

**Defects found and fixed during validation** (each pinned by a test):
1. A multi-byte `·` in a chart header panicked the tokenizer (`split_at(len-1)`); the panic killed
   the job's task after the GPU work was done and left the job stuck at "processing" with its
   concurrency permit held. Fixed the tokenizer; the diagnostic now runs in its own task (a panic is
   logged, the job persists); the server now marks a job failed if its processing task panics.
2. The trailing-zero precision rule let "2020" match 2024 and "1" match 0.883. Plain integers are
   now exact; only grouped or suffixed integers round to their last digit.
3. Unicode minus (U+2212) was not a sign; uppercase prose ("US dollars") counted as labels
   (`label_stoplist`); the model's own "**Frame 3**" headings were read as stated numbers.
4. Numeric keys were the written token, so an OCR misread of the OHLC prefix ("02.514T") and the
   true "2.514T" were listed twice; keys are now the canonical value.
5. The review sampler shuffled hallucination calls in with 1,100 axis-tick "missed" facts, so the
   first sheet had 3 of 60 hallucination calls on it; composition is now priority-ordered
   (unsupported → supported → missed) with a per-segment cap.
6. OCR runs at ~1.9 s/frame effective with 8 workers on 32 cores (thread oversubscription in
   onnxruntime); tolerable for per-job use (5 min per 40-minute video), to be tuned before the
   study's 5,400 candidate frames.

**Calibration — done without the user, who found the sheet unworkable.** Instead of 150 radio
buttons, every unsupported fact was classified by its distance to the nearest on-screen number and the
disputed frames were read at full resolution. On `2024_4_8` (54 unsupported of 460 stated):

| share | class | verdict |
|---|---|---|
| 44% | labels and percentages (`BTC` for a header reading "Bitcoin / U.S. Dollar", `2024` for an axis reading `'24`) | **metric false positive** — correct but not literal |
| 30% | within 1% of an on-screen number (`71948` vs a true `H71958`) | model digit misread, or OCR misread (`65018`/`64498` are on the chart as `0.786 (65018)` / `1 (64498)`; RapidOCR got them wrong) |
| 20% | nothing within 5% on screen — **11 of 460 stated facts (2.4%)** | genuine invention |

**So the model's invention rate is ~2%, not 12%**, and the dominant real error is digit-level
misreading of ~10 px header text. That reframing is what motivated PR-024, and is the calibration
result: the metric's absolute precision is depressed by matching rules (label synonymy, derived
percentages) and by its own OCR, so it is usable for **comparing arms** and for **catching
degeneration**, not as an absolute accuracy figure. Recorded rather than tuned away.

**The metric earned its keep**: it caught a vision-side degeneration nothing else does — a segment
that enumerated 568 numbers ("1.801, 1.802, ... 1.877"), which `truncate_repetition` cannot see
because it keys on repeated sentences. Recurred on a study arm (`study-t05-g15`: 719 stated facts in
6 segments, precision 0.154). Recorded in `docs/0.0/RESEARCH-BACKLOG.md`.

---

## Motivation

PR-022's sampling parameters are measured on the corpus but not optimised: nothing scores
description quality across sampling regimes (PR-020 Phase 5.5 Finding 1; PR-022 Tier C harness
found no such metric in the literature). PR-022's provenance makes a reference-based check cheap —
the frames a segment came from are recorded, and chart screens are mostly text and numbers — so a
description can be checked against OCR of its own source frames. With that, adaptive sampling can
be compared against fixed 0.5 fps, and `scene_threshold` / `max_gap_secs` can be tuned against an
objective instead of chosen by inspection. Design record: `docs/0.0/DESIGN-log.md`, session
2026-08-25.

## Scope

**A. The metric (`vtt-core`, new `fidelity` module).**
- Fact extraction from description text: numbers normalised to value + unit + sign
  (`1.66T`, `$42,000`, `42k`, `-0.25%`), tickers/asset labels, timeframe labels, dates, indicator
  names — CHOCOLATE's Value / Label / Out-of-context classes. Trend and Magnitude are explicitly out
  of reach and documented as such.
- OCR via an external command (config `fidelity.ocr_command`, default pointing at the RapidOCR
  wrapper `tools/ocr_frames.py`, which prints one JSON record per frame: text, box, score).
  `doctor` checks it.
- **Precision** = stated facts found in the OCR of the frames the segment was generated from.
  **Recall** = prominent on-screen facts mentioned, where the reference is OCR of every candidate
  frame in the segment's window (`fidelity.recall_reference = "kept" | "candidates"`; the per-job
  diagnostic uses `kept`, the study uses `candidates`). Prominent = persisted ≥
  `fidelity.min_persist_secs` and text height ≥ `fidelity.min_text_height_px`.
- Numeric tolerance `fidelity.number_tolerance` (set in calibration; default documented).

**B. Per-job diagnostic (server).** After merge, compute precision/recall over the visual track
and record a `fidelity` block beside `capture` (omitted when disabled); log a one-line summary.
Store kept-frame thumbnails (`fidelity.thumbnail_width`, default 640 px) in the results dir so the
metric and review remain reproducible after the job dir is cleaned.

**C. Review sheet (client).** `vid-to-text review <timeline.json>` renders an HTML sheet: per
segment, its thumbnails, the description, extracted facts with match status, and a place to record
a judgment. Used for the calibration hour (disagreement-first sampling of ~150 items) and produces
a labels file from which κ and the matching-rule adjustments are computed.

**D. The study (tooling outside the repo, `~/Documents/seer_archive/bin/`).** Three 1080p
videos at the corpus's 10th / 50th / 90th percentile of change rate — `2024_2_19`, `2024_6_24`,
`2025_05_26` — minutes 5–20 of each; grid threshold {0.05, 0.08, 0.12} × floor {15, 30 s} plus
fixed 0.5 fps; ≈ 2.4 GPU-hours. Objective: **F0.5 per GPU-hour with a hard precision floor equal to
the fixed-0.5 baseline's precision.** Segment-level bootstrap intervals. Output: a recommendation
for the `market-research` profile, or an explicit "not separable at this sample size".

**Added during implementation, recorded per the design rule:** `fidelity.ocr_workers` × `ocr_threads`
(onnxruntime's per-session default serialised the workers; 1.85 → 0.5 s/frame); `ocr.json` persisted
beside results so `vid-to-text rescore` can re-score offline against a candidates reference or a
different tolerance without GPU; the diagnostic runs in its own task and the server marks a job failed
on a processing-task panic (both found the hard way, see Validation).

**Explicitly out of scope:** changing the profile values in this PR (the study *recommends*;
adopting is a follow-on decision); Trend/Magnitude instruments; any change to sampling mechanics.

## Dependencies

- **PR-022** — provenance (`frames`, `capture`) and the adaptive sampler under test. Landed `c37e8a1`.

## Architecture section implemented

`docs/ARCHITECTURE.md` — a new **Fidelity Diagnostic** subsection (written at implementation,
alongside the code, per Documentation Accuracy).

## Verification criteria

- [ ] Fact extraction normalises the documented forms (`1.66T`, `$42,000`, `42k`, `-0.25%`, dates,
      tickers, timeframes) and is pinned by tests; unsupported forms are listed
- [ ] Precision is computed only against the segment's own frames; recall against the configured
      reference; both pinned on synthetic OCR fixtures
- [ ] The OCR wrapper returns text/box/score per frame; a missing or failing command fails the
      diagnostic (never the job) with an actionable message; `doctor` reports it
- [ ] Thumbnails are written per kept frame at the configured width and referenced by timestamp
- [ ] `fidelity` block round-trips and is omitted when disabled; old timelines load
- [ ] Review sheet renders from a timeline + thumbnails and emits a labels file; κ is computed from it
- [ ] Calibration done: ~150 human judgments, κ reported, matching rules adjusted and re-tested
- [ ] Study run within ≤ 3 GPU-hours on the three named clips with per-setting precision, recall,
      F0.5, GPU-hours and bootstrap intervals recorded here
- [ ] A recommendation (or an explicit non-result) recorded here; profile values unchanged by this PR
- [ ] `cargo test --workspace` passes

## Research backing

Tier-1. `docs/0.0/DESIGN-log.md` session 2026-08-25: CHOCOLATE error typology (ACL 2024,
proven); OCR engine chosen by measurement on this corpus (RapidOCR 76/76 vs tesseract 42/76,
proven); production precedent for screenshot OCR (OmniParser); calibration statistics and F0.5
(convention). Residual: header/ticker OCR accuracy unmeasured until calibration.

## Notes

- Tesseract 4.1.1 remains installed on the desktop from the comparison; removable.
- The study's recall reference OCRs every candidate frame (~1,800 per clip); run it in parallel on
  the desktop's 32 cores or it dominates wall time.
- Three videos bound what the study can say; a non-result is an acceptable outcome and will be
  recorded as one.
