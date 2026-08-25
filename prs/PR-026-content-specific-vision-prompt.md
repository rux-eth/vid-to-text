# PR-026: Content-specific vision prompt

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

### Procedure extension for this PR: Phase 6 (Iterative Empirical Tuning)

This PR adds one phase beyond the standard five, by user direction (2026-08-25):

> **Phase 6 — Iterative Empirical Tuning.** A prompt cannot be settled by a single
> measurement the way a threshold can: each revision changes the output distribution, so the
> next revision must be measured against the new one. After the Gate Check clears, run
> **repeated** capture/measure/revise cycles on a fixed sample of corpus videos until the
> stopping rule below is met. Every cycle is recorded — prompt version, what changed, why,
> and the measured result — so the final prompt arrives with its derivation, not just its text.

**Stopping rule (must be fixed BEFORE the first cycle, so iteration cannot chase noise):**
no cycle may continue past the point where the primary measure stops improving by more than
its own measurement error on the held-out videos. The primary measure, the error estimate and
the number of cycles are set in Phase 2 and are not revisable mid-iteration.

**Overfitting guard:** tune on a **tuning set**, confirm on a **held-out set** the prompt was
never measured against. A prompt that wins on the tuning set and not the held-out set is
rejected. Sets are named in Phase 2 before any cycle runs.

## Research findings

_To be populated by `PROCEDURE-pr-research.md`. Scoping evidence gathered 2026-08-25 is recorded
under Research backing below; it is a Tier-A probe, not the research round, and does not
substitute for the procedure._

---

## Motivation

**The shipped prompt is written for a different kind of video than this corpus contains.**
`prompts/vision.txt` (PR-015, CRISPE framework) was authored and tuned against TED talks and film
clips. Nine of its ten numbered instructions concern humans, faces, camera work and staging; its
Role claims *"deep expertise in cinematography, animation, body language, visual storytelling, and
semiotics"*; its worked examples are about runners and a Mister Rogers interview. The corpus is
screen-recorded trading charts with no people in them.

**Measured on the 13 corpus videos captured 2026-08-25** (151 visual segments, 90,231 words,
median 588 words per segment):

| pattern in the model's own output | share of segments |
|---|---|
| "no characters / people / human figures" | **76%** |
| "remains unchanged / consistent / in place" | **61%** |
| "no scene transitions / camera angles" | **61%** |
| "no expressions / body language / gestures" | **33%** |
| "In summary…" filler | 16% |

3,783 words are spent purely on the absence of humans. Far more go on restating that nothing
changed between frames — which is itself ironic after PR-022, since adaptive sampling exists so
that frames only appear *when* content changes.

**This is not merely wasted volume; the literature says it is an active hallucination driver.**
"When Prompts Override Vision: Prompt-Induced Hallucinations in LVLMs" (arXiv:2604.21911) measures
what happens when an instruction presupposes something absent from the image: on **Qwen2-VL-7B**,
the same family this pipeline runs, object recognition falls from 94.1% to **56.7%** under
adversarial presupposition. The stated mechanism is *"reliance on presuppositions introduced by the
instruction itself"*, even where visual recognition is accurate. Our prompt instructs the model to
track characters, read facial expressions and identify speakers on charts that contain none of them.

**A second, worse failure was found by reading output** (2026-08-25, `2024_4_8` segment 4). The
model reports the chart header's OHLC values as a time series: `$71,708 -> $71,850 -> $69,365 ->
$71,708 -> $69,797`, narrated as *"The price has moved slightly higher, now at $71,850"* and *"the
price now at $69,365"*. No such movement occurred. TradingView's header shows the **hovered
candle's** values, so as the analyst sweeps the cursor across history the header reports historical
prices — and the model infers a chronology that does not exist. Supporting tells: `+142 (+0.20%)`
appears verbatim at two different prices, and `$69,365 with a +6 (+0.01%) change` is arithmetically
incoherent for a chart trading near 71,700.

**The fidelity diagnostic cannot see this.** Every one of those numbers is genuinely on screen, so
each scores as supported: precision 0.883 on a segment containing a fabricated price narrative. For
a corpus whose purpose is generating trading hypotheses, a false account of price action is more
damaging than a misread digit, and nothing currently built detects it.

**Whatever the fix, provenance must close first.** `CaptureInfo` records model, fps, sampling
parameters, transcript settings and temperature — but **no prompt identifier**. The prompt is also
not part of a profile, so `run-corpus.sh`'s "already captured under this profile" check would not
notice a prompt change either. Changing `prompts/vision.txt` today would silently produce a mixed
corpus: the same defect class that stranded 36 timelines and was archived on 2026-08-25.

## Scope

**In scope:**
- **Prompt provenance** in `CaptureInfo` — an identifier and content hash of the prompt actually
  used, so two captures made under different prompts are distinguishable in the data, and so the
  runner can treat a prompt change as a reason to reprocess. This lands regardless of what the new
  prompt says, and is a prerequisite for Phase 6 being measurable at all.
- **A chart/screencast vision prompt**, selected by the research and the Phase 6 cycles. Candidate
  directions to evaluate, not decisions: removing human/cinematography presuppositions; naming the
  content class explicitly; describing the interface's conventions (that a header reflects the
  hovered candle, that the right edge is unfilled future space); constraining the model to report
  observation separately from inference; structured or semi-structured output.
- **Per-content-class prompt selection** — whichever mechanism the research supports for pairing a
  prompt with the config it was built for (a `prompts/` file named by profile, a config key, or a
  prompt bundled into the profile). The general prompt must remain usable for non-chart video.
- **A detector for the cursor-hover chronology error**, or an explicit finding that one cannot be
  built and the error must be handled in the prompt alone. Without this, Phase 6 cannot tell whether
  a revision fixed the most important defect.

**Explicitly out of scope:**
- Changing sampling, whisper, or any PR-020/PR-022 locked dimension.
- Re-opening `use_transcript` (measured worse, PR-024) or OCR grounding (measured null, PR-024).
- Fine-tuning the model. The cited mitigation for prompt-induced hallucination is preference
  optimisation, which is a different project.
- Running the corpus. This PR produces the prompt; the run is separate.

## Dependencies

- **PR-022** — provenance and the adaptive sampler whose segment shape the prompt must suit. Landed `c37e8a1`.
- **PR-023** — the fidelity diagnostic and `vid-to-text review`, which Phase 6 measures with. Landed `f570aec`.
- **PR-025** — the degeneration guard, so prompt cycles are not confounded by enumeration collapse. Landed `f45ce72`.

## Architecture section implemented

`docs/ARCHITECTURE.md` — the vision prompt surface and the Capture Configuration section (prompt
provenance is part of the operating point).

## Verification criteria

_Populated after the research round. The following are structural and hold regardless of what the
research concludes:_

- [ ] `CaptureInfo` records the prompt identifier and content hash; a timeline made with a different
      prompt is distinguishable, and `run-corpus.sh` treats a prompt change as pending work
- [ ] The general prompt still applies to non-chart video; selection is explicit, not implicit
- [ ] Phase 6 recorded: every cycle's prompt version, change, rationale and measurement
- [ ] The stopping rule and the tuning/held-out split were fixed before the first cycle and honoured
- [ ] The final prompt wins on the held-out set, not only on the tuning set
- [ ] Absent-human boilerplate is measurably reduced, reported as a rate not an anecdote
- [ ] The cursor-hover chronology error is measured before and after, or its unmeasurability recorded
- [ ] `cargo test --workspace` passes

## Research backing

Tier-2. The *motivation* is measured on this corpus and supported by a directly relevant primary
source; the *fix* is not researched.

**Scoping probe (Tier A, 2026-08-25) — status, not conclusions:**
- Prompt-induced hallucination from instruction presupposition is **proven** on the Qwen family
  (arXiv:2604.21911): 94.1% -> 56.7% on Qwen2-VL-7B. **But that paper's mitigation is fine-tuning,
  not prompting** — it establishes the problem here, not the remedy.
- Domain-specific prompting is reported to beat generic prompting for chart and diagram
  understanding, including supplying explicit descriptions of the visual conventions in use. Sources
  found so far are secondary or general-purpose; primary evidence has not been verified.
- Structured/schema-constrained output is reported to improve factual extraction from screenshots
  and slides. Applicability to open-ended description is unestablished.

**Candidate must-answer questions** (formalised, ranked and tier-assigned in Phase 2 — listed so the
scope is legible, not to pre-empt it):

1. What prompt structure most improves factual grounding for dense information displays, and is
   there measured evidence, or only convention? A finding that free description is the wrong frame
   entirely (structured extraction plus a short narrative) satisfies this and would be the more
   valuable answer.
2. Does explicitly describing an interface's conventions — that a chart header shows the hovered
   candle, that the right edge is unfilled future space — measurably reduce false inference about
   it? This is the cursor-hover defect stated as a research question.
3. Does removing presuppositional instructions improve accuracy, or only reduce verbosity? The
   presupposition literature predicts accuracy gains; that must be measured here, not assumed.
4. How should observation be separated from inference in the output so a downstream reader can tell
   them apart, given the corpus is read by humans and models rather than scored by a benchmark?
5. What is the right primary measure for Phase 6, given PR-020 Phase 5.5 established that the
   diversity metrics cannot compare a generator against itself, and PR-023's calibration established
   that fidelity precision is depressed by its own matching rules? **This blocks Phase 6** — without
   it, iteration has no objective and will chase noise.

**Known hazard.** PR-015 chose the current prompt using the CRISPE framework and no measurement, and
it has been in place ever since. This PR must not replace one unmeasured prompt with another; the
Phase 6 stopping rule and held-out split exist for that reason.

## Notes

- The 13 videos captured under the current prompt were **deleted** by user direction (2026-08-25)
  rather than kept as a control arm, so Phase 6 must regenerate its own baseline. That costs roughly
  20 minutes of GPU on two videos and should be the first cycle's zero point.
- Prompt work is open-ended by nature. The stopping rule is the guard against that, and it is fixed
  in Phase 2 rather than discovered during iteration.
- `prompts/format.txt` (the GPT format prompt) has the same content-mismatch question and is **not**
  addressed here — one PR, one thing. Worth its own work item.
