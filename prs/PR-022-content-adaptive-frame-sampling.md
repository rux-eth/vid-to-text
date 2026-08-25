# PR-022: Content-adaptive frame sampling

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

### Design-planning required first

PR-020 Phase 5.5 **escalated** here, which under Phase 4's Outcome Branch means the premise is a
design question, not a parameter choice. Run `PROCEDURE-design-planning.md` (Idea → Decisions →
Convergence → Docs) **before** this PR is written in final form. The open questions in Scope below are
inputs to that session, not decisions already taken.

## Research findings

_To be populated by `PROCEDURE-pr-research.md` after the design session._

---

## Motivation

`vision.fps` samples on a fixed clock. PR-020 Phase 5.5 measured what the content actually does, and
the clock is badly matched to it.

**Measured on two full corpus videos** (ffmpeg scene detection, no GPU, ~2 min):

| video | major changes (>0.30) | moderate (>0.10) | mean gap | median gap |
|---|---|---|---|---|
| 2024_2_12 (41 min) | 5 | 33 | 72.1s | 31s |
| 2024_4_8 (31 min) | 1 | 36 | 48.0s | 11s |

On a 15-minute sample, **97.3% of detected transitions score below 0.02** — cursor movement and
crosshair redraw, not content change.

Against a ~45s meaningful-change interval, every fps tested oversamples by an order of magnitude:

| fps | frames per 45s interval | corpus cost |
|---|---|---|
| 2.0 | 90 | 106.2 h |
| 1.0 | 45 | 53.1 h |
| 0.5 | 22 | 26.5 h |
| 0.25 | 11 | 13.3 h |

**And the rate is not constant.** Median gap differs ~3x between the two videos measured, so any
single fixed value is a compromise across the corpus. That is the case for sampling on content rather
than on a clock.

Two supporting observations from PR-020:
- The diversity metrics **cannot** choose an fps — raw scores are length-confounded, and controlling
  length introduces a time-coverage confound. Both cannot be held constant when fps is the variable.
- 68% of numbers extracted at fps 2.0 appeared in exactly one 7.5s segment. At 90 frames per
  meaningful change, the marginal frames capture cursor telemetry rather than chart state.

## Scope

Design and implement content-adaptive frame selection, exposed as a config switch.

**Proposed config surface** (user's suggestion, to be confirmed in design):

```toml
[vision]
fps = "auto"     # instead of a numeric rate
```

**Open design questions — inputs to the design session, NOT decisions:**

1. **Timestamp derivation (breaks a current invariant).** `vision.rs:161-165` computes every visual
   segment's time range arithmetically from `frame_offset * seconds_per_frame`, assuming uniform
   spacing. Under adaptive sampling there is no `seconds_per_frame`, so **every visual timestamp
   would be wrong** — and wrong in a way that looks plausible. Real frame PTS must be threaded through
   `describe_chunk` (e.g. ffmpeg `showinfo` metadata or `-frame_pts`) instead of computed. This is the
   largest piece of work in the PR.
2. **Floor.** A chunk with no detected change yields **zero frames** and therefore no visual track at
   all. `2024_2_12` has gaps up to 577s between major changes — three consecutive silent chunks. What
   is the minimum-frames guarantee, and does a static chunk get one representative frame?
3. **Ceiling.** A busy chunk could emit hundreds of frames. Q8 established the real constraint is a
   **visual-token budget** (256–16384 per video for Qwen3-VL), not a frame count, and token cost also
   scales with resolution. How is the ceiling enforced, and in which unit?
4. **Threshold selection.** `"auto"` is not parameter-free — it replaces `fps` with a scene threshold
   `T`, and the measured data shows `T` dominates: 0.02 yields 22–44 changes/min, 0.10 yields ~1. Is
   `T` fixed, per-corpus, or itself derived from the video's score distribution?
5. **Config typing.** `vision.fps` is `f32`. Accepting `"auto"` needs a tagged representation
   (e.g. `FrameRate::Fixed(f32) | FrameRate::Auto { threshold }`) with a custom deserializer, and
   every existing profile must keep working.
6. **Cost predictability.** Fixed fps makes corpus runtime computable in advance (1.949 s/frame,
   verified to ~2%). Adaptive sampling makes it content-dependent. Is a predictable upper bound
   required before committing to a 13–26 h unattended run?

**Explicitly out of scope:**
- Changing `chunk_duration_secs`. Its rationale was corrected in PR-020; the value stands.
- Any whisper-side change. The speech track is byte-identical across fps arms and is unaffected.
- Re-opening the locked dimensions from PR-020.

## Dependencies

- **PR-020** — locks every other capture dimension and ships `vision.fps` unset behind a validation
  sentinel, so this PR fills a hole that is already explicitly marked rather than overriding a value.

## Architecture section implemented

`docs/ARCHITECTURE.md` — the frame-extraction stage. This changes the pipeline's sampling mechanism
and breaks the uniform-spacing invariant that visual timestamps currently rely on, so it is a
structural change.

## Verification criteria

_Populated after the design session and research._

- [ ] Visual segment timestamps derive from real frame PTS, not arithmetic; a test pins this against
      non-uniform spacing
- [ ] A chunk with zero detected changes still produces a visual track (floor honoured)
- [ ] A pathologically busy chunk cannot exceed the token budget (ceiling honoured)
- [ ] Existing numeric-fps profiles continue to work unchanged
- [ ] Corpus runtime remains predictable within a stated bound, or the loss of predictability is
      explicitly accepted
- [ ] `cargo test --workspace` passes

## Research backing

Tier-2. The *motivation* is measured on this corpus (above); the *mechanism* is not researched.

**Candidate must-answer questions** (formalised in Phase 2 after design):

1. What scene-detection threshold is defensible for screencast content, and should it be absolute or
   derived per-video from the score distribution?
2. Does shot-boundary or adaptive sampling actually improve VLM description quality on static
   screen-recorded content — or only on the natural-video benchmarks where PR-020 Q2 found all the
   evidence lived?
3. How do production video-understanding systems bound cost under content-adaptive sampling?
4. Is scene-change score the right signal for chart content, where the meaningful change may be a
   small numeric region rather than a large pixel delta? A cursor sweep and a symbol change may score
   similarly.

**Known hazard.** PR-020 Q2 found that adaptive selectors **lost** to uniform fps at equal frame
budget on VideoMME across all four models tested. The counter-evidence favouring adaptive was
task-shaped (temporal reasoning) and, for the strongest recall figures, rated low-confidence because
the benchmark was synthetic and authored by the same team whose method won. This PR must not assume
adaptive is better; the corpus measurement justifies *investigating* it, not adopting it.

## Notes

- The measurement that motivates this PR came from a user domain observation ("charts don't change
  often while the narrator talks about them"), which was then tested rather than accepted. It held,
  and more strongly than stated.
- Scene detection is essentially free — ffmpeg computed it over two full videos in ~2 minutes with no
  GPU. Whatever the outcome, the *measurement* is cheap enough to run over the whole corpus to
  characterise the change-rate distribution beyond n=2, which should probably happen during design.
