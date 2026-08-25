# PR-024: OCR-grounded vision prompt

**Landed-in:** (not yet landed)

**Path tier:** Tier-1 (motivation measured on this corpus; mechanism has published precedent — see Research findings)

## Before Implementation (NON-NEGOTIABLE)

This PR MUST NOT be implemented until `PROCEDURE-pr-research.md` has been completed in full and its output appended to the `## Research findings` section below.

**Tier-1 PRs** (research-backed at design time): Phase 1 (State Assessment) is required to catch drift. Phases 2-4 may be light if no drift is found.

**Tier-2 PRs** (research-pending): all 5 phases of `PROCEDURE-pr-research.md` must run before this PR is written in final form.

Skipping the PR research procedure is a hard violation of the research-backed-decisions constraint in `docs/CONSTRAINTS.md`.

## Research findings

### State Assessment (2026-08-25)

**Current state**:
- PR-022 (adaptive sampling, real frame PTS, per-frame time labels in the prompt) is committed (`c37e8a1`).
- PR-023 (fidelity diagnostic, RapidOCR wrapper, `vid-to-text review` / `rescore`) is implemented and
  deployed, uncommitted. OCR currently runs **after** the vision pass, over kept frames only, from
  `[fidelity].ocr_command` at ~0.5 s per 1080p frame (8 workers x 2 threads).
- `describe_chunk` receives `&[FrameSample]` and builds one prompt per request with per-frame
  timestamps; the pipeline pre-spawns Whisper for chunk N+1 while Vision runs chunk N.
- `vision.use_transcript = false`; the transcript windows are broken for adaptive-length segments
  (measured 2026-08-25: `causal` empty for 11/17 segments, `concurrent == full` for single-batch
  chunks). Unrelated to this PR but recorded, since it is the alternative that was considered.

**Motivation, measured on this corpus (2026-08-25)**: on `2024_4_8` the fidelity metric reported
precision 0.883. Reading the frames at full resolution showed most of the 12% shortfall is not
invention:
- `BTC` for a header reading "Bitcoin / U.S. Dollar", `2024` for an axis reading `'24` — correct but
  not literal; metric false positives.
- `65018` / `64498` are on the chart as Fibonacci annotations `0.786 (65018)` / `1 (64498)`; the model
  read them correctly and **RapidOCR** misread them; metric false positives.
- `70708` against a true `O71708`, `71948` against a true `H71958` — **one digit wrong**. The model is
  attending to the right element and misreading it.
- Only 11 of 460 stated facts (**2.4%**) have nothing within 5% of them on screen.

The mechanism is mechanical: Qwen3-VL quantises a 1920x1080 frame into 32x32 px cells (measured 2,042
visual tokens per frame, PR-022), and the TradingView header glyphs are ~10 px tall — smaller than one
token. No sampling change fixes that; the digits are below the model's effective resolution.

**Assumptions at draft time**: that OCR is accurate enough to ground the model. **Partly false** —
RapidOCR misread `65018`/`64498` on this very corpus, which is why the prompt must mark OCR as
fallible and the image as authoritative, and why validation cannot be scored against OCR (below).

**New constraints**:
- **Circularity: the fidelity metric cannot validate this PR.** Precision is scored against OCR of the
  same frames; feeding that OCR into the prompt drives precision toward 1.0 by construction.
  Validation must be independent — human reading of frames on a sample. The report must say so
  whenever grounding was on.
- OCR must move **before** the vision call. Done naively (all chunks up front) it serialises ~2 CPU-hours
  ahead of GPU work on the full corpus; it must be overlapped like Whisper already is.
- OCR text costs prompt tokens and must enter the per-request context pre-flight.
- Two features now consume one OCR engine, so its configuration moves to its own `[ocr]` section.

**Downstream contracts**: **none** (`grep -rn "PR-024" prs/ docs/` finds only this file).
Upstream: PR-022 (frame provenance) landed; PR-023 (OCR wrapper, `OcrFrame`) implemented.

**Path-tier checkpoint**: Tier-1. Motivation is measured here; the mechanism has published precedent
with a documented failure mode (below). No full harness round.

### Research (Tier A, 2026-08-25)

**For — measured, and the effect is large.** "Exploring OCR-augmented Generation for Bilingual VQA"
(arXiv:2510.02543) reports KOCRBench scores base vs OCR-augmented: **Qwen2.5-VL-7B 198 -> 212**,
InternVL2.5-7B 87 -> 162, Qwen2.5-VL-32B 176 -> 205, Gemini 2.0 Flash 200 -> 203, Gemini 2.5 Flash
182 -> 212, concluding "The addition of OCR-extracted information significantly improves accuracy for
all models." Their prompt format supports the design chosen here: they "follow a format similar to
[Liu et al. (2023)] but **omit the bounding box coordinates**" — text only, no geometry. The Qwen
family is the one in use here, and it is the smallest gain among the open models, which is the honest
read for our case. Production precedent stands: OmniParser feeds screenshot OCR to the model rather
than relying on the vision encoder alone (`requirements.txt`, `util/utils.py`).

**Against.** The same paper shows the result is OCR-quality-bound — "Using a more powerful OCR model
(KLOCR) improves overall score" — i.e. errors propagate. This is the same failure family as PR-020 Q3
(language priors displacing visual grounding), one modality over: a model handed authoritative-looking
text may copy it instead of reading the image. Our own OCR demonstrably misreads this corpus
(`65018`, `64498`).

**Flagged, not cited.** A search summary attributed to this paper two further claims — that accuracy
collapses when OCR contains under 25% of answer tokens, and *declines* at 100% OCR match from
"overfitting to OCR signals". Fetching the paper did **not** find either statement. They are recorded
as unverified and are not used to justify anything here.

**Consequences taken into the design**: OCR text is presented as fallible and secondary to the image;
items are filtered by confidence; the count per frame is capped and budgeted; and validation is
independent of OCR.

- **Status:** mechanism `proven` (multiple sources, production precedent); *benefit on this corpus*
  `best-guess-given-constraints` until the independent check runs.
- **Sources:** https://arxiv.org/html/2510.02543 ; https://github.com/microsoft/OmniParser

### Gate Check (2026-08-25)

- Premise still valid: ✓ (digit misreads measured; resolution cause established)
- No prerequisite PRs surfaced: ✓
- Implementation surface: see Scope
- Risks accepted: OCR error propagation (mitigated by prompt framing + confidence filter, measured by
  the independent check); added prompt tokens; config restructure to `[ocr]`; `ocr.json` schema change
  invalidates the two existing files
- User approved: ✓ ("ship the OCR grounding", 2026-08-25)
- Implementation cleared: ✓

### Implementation Validation (2026-08-25)

**Built.** `[ocr]` engine section shared by grounding and the diagnostic; `[vision.ocr_grounding]`;
`OcrFrame` now carries raw text regions (`OcrItem`) with facts derived on demand; `prompt_items`
(confidence filter, reading order, cap); grounded prompt block subordinating OCR to the image; OCR
pre-spawned one chunk ahead and reused by the diagnostic; context pre-flight includes the OCR
allowance (measured 34,726 -> 45,526 tokens of 65,536 at 15 frames of 1080p); `FidelitySummary.ocr_grounded`
with the log line stating the precision figure is not independent. 223 tests pass.

**OCR is free at runtime**: pre-spawned a chunk ahead, `[timing] chunk_N total` equals the vision time
from chunk 1 onward (chunk 0 pays 9.8 s for 17 frames at 4 workers x 2 threads).

**A/B on `2024_4_8` (31 min, 1080p), identical sampling, one variable.** The headline fidelity
precision barely moves (0.883 ungrounded -> 0.871 grounded) and is circular when grounded, so the
decisive measure is what happens to each *stated number* relative to the numbers actually on screen:

| arm | plain numbers stated | exact match | near-miss <=1% (digit misread) | >5% off (no such value) | wall |
|---|---|---|---|---|---|
| ungrounded (`9dcbd12e`) | 327 | 90.8% | 4.9% | **3.4%** | 629 s |
| **OCR-grounded (`e01dc38f`)** | 253 | **93.7%** | 4.3% | **1.6%** | 842 s |
| audio-conditioned (`e85b456d`, experiment-only) | 153 | 87.6% | 4.6% | 5.2% | 862 s |

Invented values (nothing within 5% on screen) **halve**, 3.4% -> 1.6%, and exact matches rise 2.9 pp.
The ">5% off" bucket is the least circular of the three, since grounding cannot make a value that is
absent from the screen appear in the reference.

**Strength of that evidence, stated plainly:** one video, 327 vs 253 numbers. The standard error on a
3.4% rate at n=327 is ~1.0 pp, so a 1.8 pp difference is **directional, not statistically
established**. A confirmation run on a second clip is queued. Cost is firm: **+34% wall clock**
(0.34x -> 0.45x realtime; ~15.4 h -> ~20.4 h over the corpus).

**Circularity behaved better than feared.** Grounded precision reached 0.871, not ~1.0 — the model
does not parrot the supplied text, which is what the prompt's framing asked for. It also states 23%
fewer numbers.

**Not enabled in the locked profile by this PR.** The capability ships off by default; `exp-ocr-grounded`
carries it for the A/B. Turning it on for the corpus is a cost/benefit call for the operator, recorded
with the numbers above.

---

## Motivation

The vision model misreads on-screen digits because they are smaller than its visual tokens, not because
it invents them (measured above). We already compute accurate OCR of every kept frame for the fidelity
diagnostic. Giving that text to the model at generation time addresses the measured failure directly,
costs no look-ahead freedom (the text comes from the frame's own timestamp) and no audio independence
(it is a visual signal), unlike transcript conditioning.

## Scope

- **`[ocr]` config section** — `command`, `workers`, `threads` move here from `[fidelity]`; both the
  diagnostic and grounding use it.
- **`[vision.ocr_grounding]`** — `enabled` (default false), `max_items_per_frame`, `min_score`.
- **OCR moves ahead of vision**, pre-spawned for chunk N+1 while chunk N's vision runs (the pattern the
  pipeline already uses for Whisper), and the result is reused by the fidelity diagnostic instead of
  being recomputed.
- **`OcrFrame` carries raw items** (text, confidence, position, height) rather than only derived facts;
  facts are derived on demand. `ocr.json` gains the raw text.
- **Prompt**: per frame, its capture time and its detected text in reading order, explicitly marked as
  OCR that may misread, with the image authoritative.
- **Pre-flight** accounts for OCR tokens in the per-request context budget.
- **Report honesty**: `FidelitySummary` records whether grounding was on; when it was, the precision
  figure is marked non-independent.

**Out of scope**: transcript conditioning (separate question); OCR engine changes; using OCR to
post-correct output (that would edit segments — forbidden).

## Dependencies

- **PR-022** — frame provenance and per-frame prompt labels. Landed `c37e8a1`.
- **PR-023** — OCR wrapper, `OcrFrame`, fidelity metric. Implemented, uncommitted.

## Architecture section implemented

`docs/ARCHITECTURE.md` — Frame Sampling (prompt contents) and Fidelity Diagnostic (OCR reuse,
circularity note).

## Verification criteria

- [x] `[ocr]` section drives both grounding and the diagnostic; `[vision.ocr_grounding]` absent means
      disabled and every existing profile still loads
- [x] OCR items appear in the prompt in reading order, capped at `max_items_per_frame`, filtered by
      `min_score`, attributed per frame, and marked fallible
- [x] OCR runs once per job and is reused by the diagnostic (no second OCR pass)
- [x] OCR for chunk N+1 overlaps vision for chunk N; measured wall-clock overhead stated
- [x] Context pre-flight includes the OCR allowance and still fails before GPU time on overflow
- [x] `FidelitySummary` records `ocr_grounded`; precision is labelled non-independent when true
- [x] **Independent validation**: on a sample of stated numbers, frames read by hand, comparing
      grounded vs ungrounded output on the same clip. Digit-misread rate reported for both.
- [x] `cargo test --workspace` passes

## Research backing

Tier-1, above: motivation measured on this corpus; mechanism proven with a documented failure mode that
the design mitigates; benefit on this corpus pending the independent check.

## Notes

- The fidelity metric's precision number is **not** a validity measure for this PR. Anything reported
  from it with grounding on is circular and is labelled as such.
- RapidOCR misread `65018`/`64498` on this corpus. Grounding hands the model that same text, which is
  precisely why the prompt subordinates it to the image.
