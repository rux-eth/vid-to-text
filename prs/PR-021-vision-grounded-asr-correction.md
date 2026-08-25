# PR-021: Vision-grounded ASR correction

<!-- Landed-in: set to the released version this PR shipped under (e.g. v0.1.0).
     Use "(not yet landed)" for in-flight or dormant PRs.
     Use "superseded by PR-XXX" for replaced PRs.
     See docs/VERSIONING.md §4 for the policy. -->
**Landed-in:** (not yet landed)

**Path tier:** Tier-2 (full path — all five phases of `PROCEDURE-pr-research.md` run)

## Before Implementation (NON-NEGOTIABLE)

This PR MUST NOT be implemented until `PROCEDURE-pr-research.md` has been completed in full and its output appended to the `## Research findings` section below.

**Tier-1 PRs** (research-backed at design time): Phase 1 (State Assessment) is required to catch drift. Phases 2-4 may be light if no drift is found.

**Tier-2 PRs** (research-pending): all 5 phases of `PROCEDURE-pr-research.md` must run before this PR is written in final form.

Skipping the PR research procedure is a hard violation of the research-backed-decisions constraint in `docs/CONSTRAINTS.md`.

## Research findings

_To be populated by `PROCEDURE-pr-research.md`. Partial design-time basis is recorded under Research backing below — it does not substitute for the procedure._

---

## Motivation

PR-020's research established that this pipeline conditions across modalities in the **documented-harmful direction**, and does not implement the documented-beneficial one.

**Currently implemented — audio to vision.** `extract_transcript` (`pipeline.rs:221`) feeds the chunk's speech into the vision prompt (`vision.rs`). The VLM hallucination literature identifies language-prior dominance as a primary failure mode: models *"rely heavily on linguistic priors and insufficiently leverage the visual embeddings"*, producing output *"guided more by the language priors inherent in the LLM backbone, rather than grounded in the actual visual content."* Supplying a transcript feeds that failure mode. PR-020 locks `use_transcript = false` to stop it.

**Not implemented — vision to audio.** The reverse is documented as beneficial. Video-guided post-ASR correction *"consistently improves transcription accuracy in complex multimedia environments"*, and visual context disambiguates homophone-adjacent errors: *"a basketball court is more likely to include the term 'lay-up' whereas an office place is more likely to include 'layoff'."*

For this corpus the speech track is the primary signal — it is byte-identical at every fps setting, whisper-derived, and carries the analyst's actual reasoning. The charts on screen carry objective text (ticker symbols, exchange names, timeframes, indicator labels) that could ground exactly the terms most likely to be mis-transcribed.

The user independently proposed this direction before the research was run: unclear speech should benefit from visual context.

## Scope

Investigate and, if the research supports it, implement a correction pass that uses on-screen text to repair ASR output.

**In scope:**
- Whether to correct the transcript in place or emit a separate corrected track (the Segments Are Immutable After Merge constraint bears on this and must be respected or explicitly amended)
- Which visual signal to use: the existing free-text visual descriptions, or a dedicated OCR pass over frames
- Where the pass runs: server-side in the pipeline, or as a post-processing step over a finished timeline
- Confidence gating: which whisper segments are candidates for correction, and on what signal
- Evaluation: how a correction is shown to be an improvement rather than a plausible-sounding rewrite

**Explicitly out of scope:**
- Reinstating `use_transcript = true`. This PR is the other direction, not a reversal of PR-020.
- Deriving tradeable price levels from vision output. PR-020 records that extracted chart numbers are unvalidated.

## Dependencies

- **PR-020** (capture config) — locks `use_transcript = false`, establishing that the audio-to-vision path is closed before the reverse path is opened. Prevents a bidirectional loop where each modality is conditioned on the other's output.

## Architecture section implemented

`docs/ARCHITECTURE.md` — the processing pipeline. This PR would add a stage, so it is a structural change, not a configuration change.

## Verification criteria

_Populated after research. Shape expected below._

- [ ] A correction is demonstrably an improvement against something other than its own plausibility
- [ ] The Segments Are Immutable After Merge constraint is respected, or explicitly amended with rationale
- [ ] Corrections are attributable — it is recoverable which segments were altered and on what evidence
- [ ] No correction pass runs on segments the evidence does not support (no blanket rewriting)
- [ ] `cargo test --workspace` passes

## Research backing

Tier-2. Design-time basis exists from PR-020's Q3 but is **not sufficient** — it establishes the
direction is beneficial in general, not that it works for screencast content with on-screen chart text.

**Candidate must-answer questions** (formalised in this PR's own Phase 2):

1. Does video-guided ASR correction generalise from its studied domain (TV series, natural video) to screen-recorded content where the visual signal is rendered text rather than scenes?
2. Is OCR over chart frames a better grounding signal than the VLM's free-text descriptions, given PR-020 found those descriptions carry unvalidated numbers?
3. How is a correction validated without a reference transcript? PR-020's Q1 established there is no validated reference-free measure of faithfulness — this PR faces the same wall.
4. What is the false-correction rate, and what happens when the visual context is itself wrong?

**Known hazard, carried from PR-020:** the same research that motivates this PR also found that visual
descriptions are contaminated when generated with audio context, and that chart numbers extracted by
the VLM are internally inconsistent (0.618 mapped to three different prices in one transcript). A
correction pass grounded in an unreliable visual track could make the transcript worse while appearing
more confident. That risk is the central thing this PR's research must resolve.

## Notes

- This is a **follow-on**, not a prerequisite. PR-020 does not depend on it.
- If the research shows the visual track is too unreliable to ground ASR, the correct outcome is to close this PR with that finding recorded — a negative result is a valid conclusion here.
