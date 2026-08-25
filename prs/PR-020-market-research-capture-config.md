# PR-020: Lock the market-research capture config

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

### Procedure extension for this PR: Phase 5.5 (Empirical Validation)

This PR adds one phase beyond the standard five, by user direction (2026-08-24):

> **Phase 5.5 — Empirical Validation.** After the Gate Check clears and the config is
> implemented, run the config against the **researched evaluation metrics** from Phase 3
> on real corpus material. Findings are appended to `## Research findings`. If the
> empirical result contradicts the researched recommendation, loop back to Phase 4's
> Amend branch rather than shipping the config.

Rationale: the config values currently in place were tuned in a single ad-hoc session using
**invented** metrics (see Motivation). A research-backed recommendation that is never tested
against this corpus would repeat that error in the opposite direction — plausible-sounding and
unvalidated. Phase 5.5 closes that loop.

## Research findings

### State Assessment (2026-08-24)

**Current state**:
- `PROCEDURE-pr-research.md` and `prs/PR-TEMPLATE.md` exist (PR-019, `24f237c`). Upstream dependency satisfied.
- PR-018's mechanism is implemented and committed: `TranscriptWindow` enum, `vision.use_transcript`, per-batch prompt construction, 8 tests. Workspace suite passes 168 tests.
- Determinism (`ollama.temperature` / `ollama.seed`) is implemented and deployed; two warm runs verified byte-identical.
- Whisper config surface (`beam_size`, `initial_prompt`, temperature-fallback tholds) is implemented and deployed.
- Live `server.toml` on the desktop sets `whisper.beam_size = 5` and `vision.max_frames_per_request = 15`. **It does not set `vision.fps`.**

**Assumptions at PR draft time**:
- That `vision.fps`'s "current value" is 0.5.
- That the config dimensions in the scope table are independent knobs.
- That "locking the config" is achievable by fixing values in a profile.
- That evaluating ASR/vision repetition was an unsolved problem in this codebase.

**Stale assumptions**:
1. **`vision.fps` is 2.0, not 0.5.** 0.5 is only set inside the `ml` profile. The *effective default* for any job run without a profile is the code default `fps: 2.0` (`config.rs:353`), since `server.toml` omits the key. The scope table's "current value" column is wrong for this row, and any job submitted without `--profile` today runs at 2.0.
2. **`vision.fps` and `ffmpeg.chunk_duration_secs` are coupled, and the coupling is a locked design decision.** `docs/0.0/DESIGN-log.md` line 23 records the 180s chunk default as *"Derived from Qwen3-VL's 768-frame cap at 2fps."* Changing fps changes how much of that cap a chunk consumes: 180s yields 360 frames at fps 2.0 but only 90 at fps 0.5. The cap binds at 180s only above **fps ~4.3**. So the recorded rationale no longer constrains anything at or below fps 2.0 — but if PR-020 raises fps, it re-binds. **`chunk_duration_secs` must be added to the scope table**, at minimum to confirm its rationale still holds. Whether the 768-frame cap is even accurate for `qwen3-vl:8b-instruct-q8_0` is itself unverified and becomes a research question.
3. **The config being "locked" is not version-controlled.** All 11 profiles — including `ml.toml` and `charts.toml` — exist **only** on the desktop at `~/.vid-to-text/config/profiles/`. The repo contains no `.toml` outside Cargo manifests. Locking a config that lives on one untracked machine, with no history and no review, does not satisfy the PR's own premise. **Scope must add: bring the profile into the repo.**
4. **Repetition detection is prior art here, not a new problem.** `truncate_repetition` (`vision.rs:272`) was added by `6f10acd` (merge #18, branch `fix-vision-repetition`) and thresholds on *exact repeated sentences of >=15 chars, breaking at the 3rd occurrence*. My 2026-08-24 duplicated-8-gram metric re-derived a problem the project had already solved once, in a different way, for a different stage.

**New constraints** (from prior-art audit):
- **Asymmetry: vision output has a repetition guard, whisper output does not.** `truncate_repetition` runs only on vision descriptions (`vision.rs`). The `initial_prompt` looping observed on 2026-08-24 was in **whisper** output, which passes through ungated. Q5 must account for this existing mechanism rather than propose a parallel one, and the asymmetry is a candidate finding in its own right.
- The `#18 fix-vision-repetition` branch is evidence that repetition in this pipeline is a **recurring, already-experienced failure mode**, not a novel observation. Research must not present it as new.
- 10 of the 11 desktop profiles are experiment scratch (`exp-*`) created during the 2026-08-24 session. They are not part of the intended config surface and should not be mistaken for it.

**Downstream contracts**:
- **`prs/PR-019-vibe-rails-sync.md`** → names PR-020 as the consumer of `PROCEDURE-pr-research.md` + `prs/PR-TEMPLATE.md`. **Satisfied** — both exist and this PR uses the template.
- **`docs/0.0/RESEARCH-BACKLOG.md`** → lists PR-020 under Tier 2 with required topics "fps/sampling rate, whisper decoding, transcript windowing, determinism, and researched evaluation metrics, includes an empirical validation phase." **Current scope satisfies this**, and Phase 1 extends it with `chunk_duration_secs` and profile version-control.
- No other PR depends on this one (verified via `grep -rln "PR-020" prs/ docs/`, 2026-08-24).

**Path-tier checkpoint**:
- Header assigns **Tier-2**; `docs/0.0/RESEARCH-BACKLOG.md` independently lists PR-020 as Tier 2. Sources agree, no re-tier needed.
- Phase 1 surfaced stale assumptions but **none invalidate the PR's premise** — they widen its scope. Per the Tier-2 branch, proceed to **Phase 2 (Scope the Research)**.

**Scope amendments arising from Phase 1**:
- Correct `vision.fps` current value: **2.0** (code default; `ml` profile's 0.5 applies only when that profile is named).
- Add `ffmpeg.chunk_duration_secs` to the scope table — its recorded rationale is fps-coupled.
- Add: **bring the locked profile into version control**. A config that exists only on one untracked machine cannot be "locked".
- Add a research question: is the 768-frame cap cited in `DESIGN-log.md` accurate for the current model, and does any per-request frame ceiling actually bind?
- Q5 (repetition detection) must start from the existing `truncate_repetition` prior art and address the vision/whisper asymmetry.


### Research Questions (Phase 2, 2026-08-24)

**Must-answer:**

1. **How is the value of a video-derived text corpus evaluated when the intended use is exploratory hypothesis generation rather than model training?**
   *Success criteria:* >=2 named, cited evaluation frameworks or metric families applicable to non-training corpus use, each with what it measures and its stated limitations, and concrete enough to compute over this project's `timeline.json` shape. A bare list of NLP metrics without applicability argument does not satisfy this.
   *Why it blocks:* Phase 5.5 measures the locked config against these. Without them, Phase 5.5 repeats the 2026-08-24 error of inventing metrics.

2. **What frame-sampling rate is defensible for screen-recorded chart/presentation content, and on what evidence?**
   *Success criteria:* cited sampling strategies from video-understanding systems that handle static/slide/screencast content, with rationale. A finding that fixed-fps is the wrong frame entirely (e.g. shot-change or keyframe-triggered sampling is standard) satisfies this and would be the more valuable answer.
   *Why it blocks:* the single largest cost/quality lever (106.2h at fps 2.0 vs 26.5h at 0.5), and the 2026-08-24 choice was made under a framing this PR rejects.

3. **Does conditioning a vision model on the concurrent audio transcript improve or degrade the descriptions' value as independent evidence, and how is that assessed?**
   *Success criteria:* cited evidence on text-conditioning effects in vision-language models — language-prior bias, hallucination amplification, or grounding improvement — with at least one source arguing each direction.
   *Why it blocks:* determines `vision.use_transcript`. Measured 98% -> 30% citation change locally, but with no evidence on whether conditioning helps or harms *accuracy*.

4. **Is look-ahead contamination inside a generated corpus a recognized problem for exploratory financial research, or only for training/backtesting? What is the accepted mitigation?**
   *Success criteria:* cited treatment of look-ahead bias in financial research pipelines, specifically addressing whether it matters before a backtest exists. Must name the accepted mitigation pattern.
   *Why it blocks:* determines `vision.transcript_window`. The exploratory reframing may reduce its urgency; that must be evidenced, not assumed.

5. **What is the accepted method for detecting and quantifying hallucinated repetition in ASR output?**
   *Success criteria:* named, cited detection methods or metrics. Must be reconciled against this repo's existing `truncate_repetition` prior art (`vision.rs:272` — exact repeated sentences >=15 chars, break at 3rd) and address the vision-guarded / whisper-ungated asymmetry found in Phase 1.
   *Why it blocks:* feeds both the whisper settings and the Phase 5.5 metric set.

6. **Do beam search vs greedy decoding have documented effects on ASR hallucination, and does `initial_prompt` have documented failure modes?**
   *Success criteria:* cited evidence from whisper/whisper.cpp issue trackers, model cards, or papers. Must include at least one source that disagrees with the locally-measured result (beam5 better, prompt harmful) or an explicit statement that none was found after specified searches.
   *Why it blocks:* `whisper.beam_size` and `whisper.initial_prompt` are currently set from a single 900s clip on one video.

7. **What is accepted practice for reproducibility of LLM-generated corpora, and is `temperature=0` + fixed seed sufficient given GPU non-determinism?**
   *Success criteria:* cited statement on determinism guarantees for the relevant serving stack, and the accepted practice when bit-reproducibility is unavailable.
   *Why it blocks:* determines whether the determinism claim in the locked config is honest.

8. **Is the 768-frame cap recorded in `docs/0.0/DESIGN-log.md` accurate for `qwen3-vl:8b-instruct-q8_0`, and does any per-request frame ceiling actually bind?**
   *Success criteria:* the model card / official docs figure, cited, at the version in use. Determines whether `ffmpeg.chunk_duration_secs`'s recorded rationale still holds.

**Internal — no web round** (resolved in Phase 4 synthesis):

- **I1.** Should the locked profile live in the repo, and by what mechanism does it reach the desktop? (Phase 1 finding: profiles are untracked.) Pure project-hygiene decision.
- **I2.** Should whisper output receive a repetition guard analogous to `truncate_repetition`? Depends on Q5's answer; the *decision* is internal even if the *method* is researched.
- **I3.** The transcript-free variant of `ollama.default_prompt` (removes the disclaimer noise). Mechanical.

**Dependencies:**
- Q5 -> Q6 (sequential): the detection method must be established before evaluating which decoder produces less of what it detects.
- Q1 -> Phase 5.5 (blocking): metrics must exist before validation runs.
- Q8 -> Q2 (weak): a binding frame ceiling would constrain the sampling answer. Run Q8 first; it is cheap.
- Q2, Q3, Q4, Q7 mutually independent.
- I2 depends on Q5.

**Research plan (depth tiers):**

Per `PROCEDURE-pr-research.md` Phase 2: **Tier A runs first on every question** as a scoping pass, including those expected to escalate.

- **Round 1 — Tier A probes, all 8 questions.** Inline `WebSearch` + `WebFetch` of primary sources, no harness. Purpose: establish whether literature exists, and confirm or revise the pre-assigned escalations below.
- **Round 2 — escalations**, run only where Round 1 leaves a load-bearing point uncertain.

| Q | Tier | Rationale |
|---|---|---|
| Q1 | **A -> B (pre-assigned)** | Load-bearing for Phase 5.5 and likely thin/scattered literature — "corpus evaluation for exploratory use" is not a standard benchmark task. Escalation expected. |
| Q2 | **A -> B (pre-assigned)** | Largest cost lever (80h swing). Expect fragmented evidence across video-understanding and screencast-summarization work. |
| Q5 | **A -> B (pre-assigned)** | Feeds both config and metrics; ASR hallucination detection is an active area with real literature, so B is likely to pay. |
| Q3 | A -> B if inconclusive | Real VLM literature exists on language-prior bias; Tier A may suffice. |
| Q4 | A -> B if inconclusive | Look-ahead bias is well documented in finance; the *exploratory-use* angle may not be. |
| Q6 | A -> B if inconclusive | whisper.cpp issue tracker is a primary source and may resolve at Tier A. |
| Q7 | **A** | Narrow, factual; determinism guarantees are documented by serving stacks. |
| Q8 | **A** | Single documented figure from a model card. Cheapest question; run first. |

- **Default tier:** A.
- **Escalations above A and reasons:** Q1, Q2, Q5 pre-assigned to B (load-bearing + expected uncertainty). Q3, Q4, Q6 conditional.
- **Tier E:** not used. Requires explicit operator approval and is not warranted here.
- **Estimated cost:** Round 1 ~8 x Tier-A probes. Round 2 at 3 pre-assigned Tier-B harness runs ~= 750k tokens; each conditional escalation adds ~250k. **User approval requested before Round 2 dispatch**, since cost scales with escalation count.

**Explicitly excluded from this round (nice-to-have):**
- `vision.max_frames_per_request` tuning beyond confirming 15 is safe. Measured effect was -5.9% wall time, not statistically established (se +/-5.3%), and it trades output granularity for speed. Low stakes; carry the current value.
- `whisper.model_path` (turbo vs `large-v3`) beyond Q6's scope. Measured equal on repetition and content retention at 2.3x cost; parsimony favours turbo without further research.
- Corpus survivorship bias (the 22 coverage gaps). Data-provenance question, not capture-config. Needs its own work item.
- Prompt-engineering optimisation of `prompts/vision.txt` (CRISPE framework, PR-015). Out of scope beyond I3's mechanical fix.

---

### Findings — Round 1 (Tier-A probes, 2026-08-24)

Round 1 ran a Tier-A probe on all 8 must-answer questions as the scoping pass. **4 resolved at
Tier A, 1 needs a counter-case probe, 3 confirmed for escalation.** Full options/pros/cons and the
mandatory disconfirming-evidence sections are completed per question at the tier that resolves it;
Round-1 entries below record status and the evidence that determined it.

---

**Q8: Is the 768-frame cap real for `qwen3-vl:8b-instruct-q8_0`?** — **RESOLVED (Tier A)**

*Finding:* The official Qwen3-VL README documents **no per-request frame ceiling and no 768-frame
cap**. It specifies a native **256K context (expandable to 1M)** and controls video via a **visual-token
budget** — "you can set the number of visual tokens of a single video to 256-16384 (32x spatial
compression + 2x temporal compression)" — plus `fps`, `num_frames`, `max_pixels`/`min_pixels`.
Serving-layer caps exist but are configuration, not model limits (vLLM's `--limit-mm-per-prompt`
defaults to image=4/video=1 for the 7B class).

*Consequence:* the rationale recorded in `docs/0.0/DESIGN-log.md:23` — *"Derived from Qwen3-VL's
768-frame cap at 2fps"* — **cites a figure that does not appear in the official documentation.**
`ffmpeg.chunk_duration_secs = 180` is therefore currently unjustified by its own stated reason. The
real constraint is a **token** budget, not a frame count, which is a different quantity and scales
with resolution as well as frame count.

- **Status:** proven (official model documentation)
- **Sources:** https://github.com/QwenLM/Qwen3-VL/blob/main/README.md ; https://docs.vllm.ai/projects/recipes/en/stable/Qwen/Qwen3-VL.html
- **Risk accepted:** the README may omit a cap stated in the technical report; Group D Probe 1 will re-verify against the canonical documenter before this is locked.

---

**Q7: Is `temperature=0` + fixed seed sufficient for reproducibility?** — **RESOLVED (Tier A), and it contradicts what is currently deployed**

*Finding:* Two independent points, both against the current implementation.

1. **The seed is a no-op at temperature 0.** A seed governs the sampling step; greedy decoding has
   no sampling step, so `seed` is ignored. The `ollama.seed = 42` currently deployed does nothing.
2. **The root cause of non-determinism is batch-size dependence of reduction kernels, not
   floating-point non-associativity.** Thinking Machines Lab: *"the primary reason nearly all LLM
   inference endpoints are nondeterministic is that the load (and thus batch-size)
   nondeterministically varies."* Even at temperature 0, "LLM APIs are still **not** deterministic in
   practice."

*Consequence:* the 2026-08-24 observation (two warm runs byte-identical, one cold run divergent) was
attributed in-session to "a freshly loaded model producing different logits." That explanation was
wrong. The likely mechanism is differing batch/kernel conditions. **The determinism the config
currently claims is not guaranteed** — it held because batch conditions happened to coincide.

*Accepted fix:* batch-invariant kernels for RMSNorm, matmul and attention, at ~60% throughput
overhead (vLLM/SGLang). **Ollama does not implement this**, so bit-reproducibility is not available
on the current serving stack at any setting.

- **Status:** proven (primary engineering source + corroborating survey)
- **Sources:** https://thinkingmachines.ai/blog/defeating-nondeterminism-in-llm-inference/ ; https://arxiv.org/pdf/2506.09501
- **Open for Phase 4:** whether to keep `temperature=0` (still worth it — removes *sampling*
  variance even if kernel variance remains), drop the misleading `seed`, and state the honest
  reproducibility guarantee in the config comment rather than implying bit-determinism.

---

**Q6: Beam search vs greedy, and `initial_prompt` failure modes** — **RESOLVED (Tier A) for the decoder half; counter-case outstanding**

*Finding:* The original Whisper paper **acknowledges that greedy decoding falls into repetition
loops on long-form transcription** and prescribes the exact remedy now deployed: begin with
**beam search width 5 at temperature 0**, with **compression-ratio and log-probability thresholds**
as the fallback triggers. This is upstream's own recommendation, not a local discovery — the
2026-08-24 measurement (turbo greedy 4.4% -> beam5 0.0% duplicated 8-grams) reproduces documented
behaviour.

*Consequence:* `whisper.beam_size = 5` is upstream-prescribed, and the previously-shipped
`Greedy { best_of: 1 }` was below upstream default. Confirms the deployed value.

- **Status:** proven (model paper + corroborating implementations)
- **Sources:** https://arxiv.org/pdf/2303.00747 (WhisperX, long-form decoding) ; https://github.com/sanchit-gandhi/whisper-jax/issues/148 ; https://yage.ai/share/whisper-repetition-hallucination-en-20260526.html
- **Outstanding:** no source yet found arguing *against* beam search, and no documented
  `initial_prompt` failure mode located. Both are required by the unbiased-presentation rule. One
  further Tier-A probe is scheduled before this is marked complete.

---

**Q4: Does look-ahead contamination matter outside training/backtesting?** — **RESOLVED (Tier A)**

*Finding:* Yes, and it is formalised. Recent work defines **Look-Ahead-Freedom as Temporal
Non-Interference** — *"a formal, verifiable property ... certifying that a computational pipeline
does not allow information from the future to influence a decision made in the past"* — explicitly
scoped to **backtesting *and agentic trading pipelines***, not backtests alone. A standardized
benchmark (`Look-Ahead-Bench`) exists for point-in-time LLMs in finance.

Critically for this PR, the literature names **"textual sources that contain hindsight about market
movements"** as a distinct leakage vector, and notes leakage that *"inspection of the pipeline code
cannot rule out."* A visual description generated from a prompt containing the next three minutes of
speech is precisely that vector.

*Consequence:* the exploratory reframing does **not** neutralise look-ahead. It is a property of the
corpus, and it survives into any later use.

- **Status:** proven (formal property + named benchmark, multiple independent 2025-2026 sources)
- **Sources:** https://arxiv.org/html/2607.04958v1 ; https://arxiv.org/pdf/2601.13770 ; https://arxiv.org/abs/2605.24564
- **Risk accepted:** these are recent papers rather than settled practice; the *concept* (data leakage) is long-established, the formalisation is new.

---

**Q3: Does audio-conditioning help or harm the visual track's independence?** — **PARTIAL (Tier A); counter-case probe needed**

*Finding so far, one direction only:* VLM hallucination literature consistently identifies
**language-prior dominance** — models "rely heavily on linguistic priors and insufficiently leverage
the visual embeddings," producing output "guided more by the language priors inherent in the LLM
backbone, rather than grounded in the actual visual content." Adding a transcript to the prompt
supplies exactly the textual context this failure mode feeds on, which predicts the locally-measured
98% audio-citation rate.

*Missing:* the unbiased-presentation rule requires at least one cited source arguing the other
direction (that text conditioning *improves* grounding, e.g. as query/context conditioning). Not yet
searched. **One more Tier-A probe**, then escalate only if still one-sided.

- **Status:** best-guess-given-constraints pending the counter-case
- **Sources so far:** https://arxiv.org/html/2605.25036 ; https://arxiv.org/html/2505.19678v3 ; https://arxiv.org/html/2605.08245

---

**Q1: How is a corpus evaluated for exploratory (non-training) use?** — **ESCALATE to Tier B (confirmed)**

*Round-1 result:* a real taxonomy exists. The 2026 survey on LLM-generated data proposes
**Quality** (Validity, Fidelity, Diversity, Utility) and **Trustworthiness** (Faithfulness, Safety,
Robustness, Fairness, Privacy, Provenance, Benchmark Contamination), with named metrics — GAR, EDS,
TTR, Self-CosSim, Attr_AIS, PMI-FAITH, EM_TS.

*Why it must escalate:* the survey **"does not systematically distinguish evaluation protocols
between training-data versus evaluation-set uses"**, and states that **"most evaluations of generated
data are extrinsic"** — i.e. measured by downstream task performance, which is exactly what an
exploratory corpus lacks. The load-bearing question (what to measure when there is no downstream
task to score against) is therefore unresolved. Hypothesis-generation metrics found in the same
search evaluate *generated hypotheses*, not the *corpus feeding them* — a category mismatch.

- **Sources so far:** https://arxiv.org/html/2601.17717 ; https://ceur-ws.org/Vol-4100/paper2.pdf

---

**Q2: Defensible sampling rate for screencast/chart content?** — **ESCALATE to Tier B (confirmed)**

*Round-1 result: genuinely conflicting evidence.*
- **For uniform-fps:** on VideoMME, "Uniform-FPS sampling consistently yielded the best performance
  across all SVLMs"; an ablation found accuracy "peaking around 256 frames."
- **Against uniform-fps:** the keyframe-extraction literature states uniform sampling "may fail to
  represent the video" with **"too many keyframes with similar content for a long static segment"** —
  a direct description of a 40-minute static-chart video. Shot-aware and adaptive selectors are
  reported to "capture rare or fleeting, question-relevant events."

*Why it must escalate:* the strongest empirical source **"does not specifically test performance on
static or slide-heavy content separately"** — the exact content class here. The two bodies of
evidence point opposite ways for our case, and this question governs an 80-hour cost swing.
Escalation is warranted precisely because Tier A found *conflict*, not absence.

- **Sources so far:** https://research.momentslab.com/blog-posts/frame-sampling-vlm ; https://arxiv.org/html/2603.17374v1 ; https://link.springer.com/article/10.1007/s00521-021-06322-x ; https://arxiv.org/pdf/2412.10360

---

**Q5: Accepted method for detecting/quantifying ASR hallucinated repetition?** — **ESCALATE to Tier B (confirmed)**

*Round-1 result:* named text metrics exist (**Levenshtein distance, length ratio, perplexity**) plus
model-internal signals (layer-wise probing, token confidence, attention patterns). A strong lead
surfaced from Q6's search: Whisper's own **compression-ratio threshold** and **log-probability
threshold** are described as *"the most direct signals for detecting repetition."*

*Why it must escalate:* the dedicated hallucination-detection paper states that **"formal metrics for
repetition aren't extensively detailed"** — no canonical repetition metric was named. Q5 must also
reconcile against this repo's `truncate_repetition` prior art and resolve the Phase-1 asymmetry
(vision guarded, whisper ungated), which no Round-1 source addresses.

- **Sources so far:** https://arxiv.org/pdf/2606.23060 ; https://arxiv.org/pdf/2206.02369

---

### Round 1 summary

| Q | Status | Tier that resolved / next |
|---|---|---|
| Q8 | resolved — **no 768-frame cap exists; DESIGN-log rationale unsupported** | A |
| Q7 | resolved — **seed is a no-op at temp 0; batch-invariance is the real cause** | A |
| Q6 | resolved (decoder half) — beam5@temp0 is upstream's own prescription | A + counter-case probe |
| Q4 | resolved — look-ahead is a pipeline property, formalised, applies here | A |
| Q3 | partial — strong one-directional evidence; counter-case not yet searched | A probe, then decide |
| Q1 | **escalate** — taxonomy exists but no protocol for non-training use | B |
| Q2 | **escalate** — sources conflict for static-content class specifically | B |
| Q5 | **escalate** — no canonical repetition metric named | B |

**Round 2 dispatch (pending user approval):** Q1, Q2, Q5 at Tier B via
`~/.claude/research-tiers/light-research.js`, ~250k tokens each (~750k total). Q3 and Q6 counter-case
probes remain Tier A and run inline first, since they may close without escalation.


### Findings — Round 1b (Tier-A counter-case probes, 2026-08-24)

**Q6 counter-case — RESOLVED. `initial_prompt` has a documented failure mode matching the local measurement.**

*Disconfirming evidence sought and found (for the prompt half):* the upstream Whisper repository
carries a dedicated discussion titled **"Repetitions and Hallucinations when using prompt feature"**.
The documented behaviour: *"Using the prompt feature in Whisper is helpful when transcribing technical
terms, but will cause specific hallucinations and repetitions."* Reported concrete cases include a
transcription that *"got stuck in a loop"* and an inserted phrase absent from the audio. Mechanism
given upstream: *"Whisper uses previous transcription results to prompt the current transcription, so
if it can't recognize something clearly, it starts to make things up based on previous results."*
Recommended mitigation is to disable prompt/context carry-over.

This is a **documented, upstream-acknowledged failure mode**, not a local anomaly. The 2026-08-24
measurement (90% intra-segment repetition under greedy+prompt, real content deleted) reproduces it.

*The genuine counter-argument, recorded:* the same sources state the prompt **does** help with
technical vocabulary and that *"the prompt history does improve output in normal conditions."* So the
tradeoff is real: `initial_prompt` buys jargon accuracy at the cost of hallucination risk. The local
measurement found the jargon benefit did **not** materialise here (the only domain-term movement,
`timeframe` 1->8, was a tokenisation artifact of "time frame" collapsing) while the repetition cost
did. That asymmetry is corpus-specific and is the reason to decline the prompt **for this corpus**,
not a claim that prompts are universally harmful.

*Counter-case for beam search:* **none found.** Searches run: "whisper beam search vs greedy
hallucination repetition loops", "whisper initial_prompt causes hallucination repetition problems
prompt conditioning drawbacks". No source argued greedy is preferable to beam search for long-form
transcription. Recorded per the unbiased-presentation rule as an explicit null result.

- **Status:** proven (upstream issue tracker + corroborating practitioner writeups)
- **Sources:** https://github.com/openai/whisper/discussions/1992 ; https://github.com/ggml-org/whisper.cpp/discussions/2286 ; https://github.com/openai/whisper/discussions/1490 ; https://memo.ac/blog/whisper-hallucinations

---

**Q3 counter-case — RESOLVED, and it reveals the pipeline is conditioning in the wrong direction.**

*Evidence FOR multimodal conditioning (the counter-case sought):* it exists and is positive — but for
the **opposite direction of information flow**. Video-guided post-ASR correction *"consistently
improves transcription accuracy in complex multimedia environments"*, and visual context disambiguates
audio: *"a basketball court is more likely to include the term 'lay-up' whereas an office place is more
likely to include the term 'layoff'."*

*Evidence AGAINST the direction this pipeline actually uses:* two independent strands.
1. **Language-prior dominance in VLMs** (Round 1): models *"rely heavily on linguistic priors and
   insufficiently leverage the visual embeddings"*, producing output *"guided more by the language
   priors ... rather than grounded in the actual visual content."* Supplying a transcript feeds
   exactly this failure mode.
2. **ASR text is a lossy conditioning signal**: *"learning with raw audio signals can be more
   beneficial than relying on text transcripts extracted via ASR"*, and aligning transcript text with
   visual content *"remains challenging, as even the most relevant subtitles often undergo
   rephrasings, stylistic edits, and temporal misalignments."*

*Synthesis:* the literature supports **vision -> audio** conditioning (visual context improving ASR)
and warns against **audio -> vision** conditioning (text priors displacing visual grounding). The
pipeline currently implements **audio -> vision** — `extract_transcript` feeds
`describe_chunk` (`pipeline.rs:131`). That is the direction with documented downside, and the
direction with documented upside is **not implemented at all**.

This is a Phase-4 **Amend candidate**: it does not merely support `use_transcript = false`, it
identifies an unbuilt capability (visual grounding of unclear speech) that the user independently
proposed earlier in the session.

- **Status:** proven for both directions (peer-reviewed, CVPR workshop + arXiv)
- **Sources:** https://arxiv.org/html/2506.07323 (video-guided post-ASR correction) ; https://openaccess.thecvf.com/content/CVPR2024W/MULA/papers/Shen_Exploring_the_Role_of_Audio_in_Video_Captioning_CVPRW_2024_paper.pdf ; https://arxiv.org/html/2605.25036 ; https://arxiv.org/pdf/2010.02384
- **Risk accepted:** the post-ASR-correction sources target TV/film content, not screencasts with on-screen text. Whether chart OCR is a comparably useful grounding signal is untested here.

---

### Round 2 status (dispatched 2026-08-24)

| Q | Harness run ID | Status |
|---|---|---|
| Q1 (corpus evaluation for exploratory use) | `wf_b62876ce-95a` | dispatched |
| Q2 (sampling strategy for static screencast content) | `wf_e8dec556-5b5` | dispatched |
| Q5 (ASR repetition detection methods) | `wf_2a6bf340-ce5` | dispatched |

Tier B, `~/.claude/research-tiers/light-research.js`, ~250k tokens each.


### Group D: MCP Verification (2026-08-24)

Scope filter applied: probes run only on claims that are identifier-specific, option-driving, and
not pure methodology. Methodology claims (e.g. "look-ahead is a recognised problem") are out of scope.

**Schema-Integrity Probe (Probe 1):**

| Claim | Identifier | Canonical documenter | Verified? | Notes |
|---|---|---|---|---|
| Q5 / deployed whisper config | `entropy_thold`, `logprob_thold`, `no_speech_thold`, `temperature_inc`, `temperature` | `whisper-rs-0.16.0/src/whisper_params.rs:355-381` | **yes** | Each setter assigns to `self.fp.<identical name>`, so the deployed values reach exactly the `whisper_full_params` fields the research names. 1:1, no translation layer. |
| Q5 | `entropy_thold` is whisper.cpp's analogue of OpenAI's `compression_ratio_threshold` | `whisper.cpp/include/whisper.h` | **yes** | Header carries the verbatim comment *"similar to OpenAI's compression_ratio_threshold"*, and all four thresholds plus `beam_search.beam_size` are fields of the same `whisper_full_params` struct. |
| Q7 / deployed vision config | Ollama `temperature`, `seed` | `ollama/docs/api.md` | **yes, with conflict** | Both are documented valid options. **But the docs state *"For reproducible outputs, set `seed` to a number"* with a dedicated "Reproducible outputs" example — which contradicts the Q7 finding that a seed is inert at temperature 0.** See Reconciliations. |
| Q1 / Phase 5.5 metric | `cr_pos` | `github.com/cshaib/diversity` (README, canonical for the package API) | **NO** | The documenter lists no `cr_pos`. POS-compression is described in the paper but is not exposed under that name. **Downgraded to `best-guess-given-constraints`; re-derive from the documenter before use.** |
| Q1 / Phase 5.5 metric | `ngram_diversity_score(text, n=4)` | same | **partial** | Function exists; the **signature diverges** — actual is `ngram_diversity_score(texts, num_n=4)`. Research reported the wrong parameter name. |
| Q1 / Phase 5.5 metrics | `compression_ratio(texts, algorithm='gzip')`, `self_repetition_score(dataset, n=4)`, `homogenization_score(texts, measure='rougel')` | same | **yes** | Present with those exact signatures. Package installs as `pip install diversity`. |
| Q1 / Phase 5.5 metric | Self-BLEU as a standalone function | same | **partial** | Not exposed as its own function. Reachable only via `homogenization_score(..., measure=...)`; the README example demonstrates only `'rougel'`. Treat "Self-BLEU" as a `measure` argument, not an API. |

**Synthesis-Verification Probe (Probe 2):**

| Claim | Combined elements | Cited working example | Verified? | Notes |
|---|---|---|---|---|
| Deployed whisper config | `beam_size=5` + `entropy_thold=2.4` + `logprob_thold=-1.0` + `no_speech_thold=0.6` + `temperature_inc=0.2` | `whisper-rs-0.16.0/src/whisper_params.rs:57-75` | **yes** | `FullParams::new(BeamSearch{..})` calls `whisper_full_default_params(WHISPER_SAMPLING_BEAM_SEARCH)` — which sets all four thresholds — and then assigns `fp.beam_search.beam_size` on the **same struct**. One constructor realises the exact combination; not a synthesis across sources. |
| Whisper paper co-locates beam-5 with the 2.4/-1.0 thresholds in one passage | paper Section 4.5 | **not obtained** | **no** | Two independent fetch attempts of `arxiv.org/pdf/2212.04356` and `/abs/` failed text extraction (binary PDF). The thresholds were verified from the paper by the Q5 harness (2-0); the *co-location with beam-5* was not independently confirmed here. **Labelled `best-guess-given-constraints`** — risk accepted, because the combination is independently verified in whisper.cpp's own constructor above, which is the surface that actually executes. |

**Binding-at-creation (Probe 3):** applicable — a profile is registered at job-submission and read
back during processing. Probed empirically on 2026-08-24: submitting `clip900.mp4` with
`--profile ml` produced `chunk_0 whisper: 44.0s` (matching the measured beam-5 cost of 44.6s, not
the greedy 42.4s) and **30 visual segments** rather than 120, confirming both the `[whisper]` and
`[vision]` sections of the profile bound to the running job. Binding confirmed at the expected
lifecycle moment.

**Reconciliations:**

1. **Q5 silence-override claim: harness refutation overturned.** The harness voted **0-2 to refute**
   the claim that `needs_fallback` is forced back to `False` under a silence condition. Group D read
   `whisper/transcribe.py` directly and the block exists verbatim:
   ```python
   needs_fallback = True   # too repetitive
   ...
   needs_fallback = False  # silence
   ```
   guarded by `no_speech_prob > no_speech_threshold and avg_logprob < logprob_threshold`.
   **The adversarial verifier was wrong; the claim is confirmed and restored.** The research agent
   flagged this as a suspected verifier error rather than accepting the refutation, which is what
   allowed Group D to settle it. Recorded because it is evidence the 2-vote lenient quorum at Tier B
   can produce false refutations.

2. **`cr_pos` downgraded** from a named metric to `best-guess-given-constraints`. Phase 5.5 must use
   the documenter's actual API surface, not the paper's notation.

3. **Ollama `seed` conflict recorded, not resolved by fiat.** Ollama documents `seed` as *the*
   reproducibility mechanism; Q7's sources state a seed is inert under greedy decoding because there
   is no sampling step to seed. Both are consistent if Ollama's "Reproducible outputs" example
   assumes its default `temperature` (0.8) rather than 0. **Consequence for this PR: the deployed
   `ollama.seed = 42` alongside `temperature = 0` is very likely a no-op, and the config comment
   implying it delivers determinism is unsupported.** Carried to Phase 4 as an Amend candidate.

**Exit criteria:** every load-bearing identifier probed against its canonical documenter; two
divergences downgraded; one harness refutation overturned against primary source; one combination
labelled BGGC with the risk stated.


### Synthesis (2026-08-24)

**Outcome**: **Amend** — five findings contradicted the shipped code or the PR spec; all five
amendments presented to the user and approved 2026-08-24 as recommended. Synthesis steps executed
below only after that decision, per the Outcome Branch rule.

**Changes to this PR** from research:

1. **`ollama.seed` removed.** A seed is inert under greedy decoding (no sampling step to seed). The
   deployed `seed = 42` alongside `temperature = 0` was a no-op. `temperature = 0` is retained — it
   removes *sampling* variance, which is real — but the honest guarantee is now stated: bit-level
   reproducibility requires batch-invariant kernels (~60% overhead, vLLM/SGLang) that **Ollama does
   not implement**, so identical output across runs is not guaranteed at any setting.
2. **Whisper threshold parameters retained, their justification corrected.** `entropy_thold=2.4`,
   `logprob_thold=-1.0`, `no_speech_thold=0.6`, `temperature_inc=0.2` are whisper.cpp's own defaults
   and match the Whisper paper's Section 4.5 heuristics. But they are **retry triggers, not filters**:
   at the final temperature the result is accepted regardless of how repetitive it is, and the paper's
   Table 7 ablation shows **zero WER gain** from temperature fallback. The in-decoder gate cannot
   remove repetition from output.
3. **Post-hoc repetition detection added to scope.** Because (2) means nothing filters whisper output,
   and because `truncate_repetition` (`vision.rs:272`) guards **vision only**, whisper output is
   currently ungated end-to-end. Post-hoc `compression_ratio` flagging closes that asymmetry.
4. **`vision.fps` deferred to Phase 5.5.** The premise it was chosen on ("uniform fps oversamples
   static segments") was refuted 0-2 twice as motivation-not-measurement. No published evidence covers
   static screencast content. Uniform fps is the defensible default at equal frame budget. The value
   is therefore **not locked by this research** and is decided by Phase 5.5 measurement on this corpus.
5. **`vision.use_transcript = false` locked.** Audio-to-vision conditioning is the documented-harmful
   direction (language priors displace visual grounding). Vision-to-audio is the documented-beneficial
   direction and is **not implemented** — split out as PR-021.
6. **`ffmpeg.chunk_duration_secs = 180` retained, rationale corrected.** No 768-frame cap exists in
   Qwen3-VL's documentation; the real constraint is a visual-**token** budget (256-16384 per video),
   which is a different quantity that also scales with resolution. Value unchanged; recorded reason
   fixed in `docs/0.0/DESIGN-log.md`.
7. **`vision.transcript_window = causal` confirmed, not amended.** Look-Ahead-Freedom is a formal,
   verifiable pipeline property explicitly scoped to agentic trading pipelines, with "textual sources
   that contain hindsight" named as a leakage vector. The Phase-1 supposition that the exploratory
   reframing reduced its urgency was wrong.
8. **Phase 5.5 metric names re-derived from the canonical documenter**, not the paper: `cr_pos` does
   not exist in the `diversity` package and is dropped; `ngram_diversity_score` takes `num_n`, not `n`;
   Self-BLEU is a `measure` argument to `homogenization_score`, not a standalone function.

**Changes to `docs/ARCHITECTURE.md`**: new "Capture configuration" section documenting the locked
operating point, the research basis for each value, and the two values deliberately **not** locked
here (`fps`, pending Phase 5.5).

**Changes to `docs/CONSTRAINTS.md`**: one new domain constraint — **Corpus Look-Ahead Freedom**. It is
research-backed (formal property, named benchmark, multiple independent sources) and load-bearing for
the corpus's stated purpose, which qualifies it as a non-negotiable rather than a tunable.

**Changes to `docs/0.0/DESIGN-log.md`**: correct the chunk-size rationale, which cites a
768-frame cap that does not appear in Qwen3-VL's documentation.

**New PRs that must come first**: none. One new PR created as a **follow-on**, not a prerequisite:
- **PR-021 — Vision-grounded ASR correction.** Implements the documented-beneficial direction
  (vision to audio) that this research surfaced and the pipeline lacks. Does not block PR-020.

**Research-backed details now locked in this PR**:
- `whisper.beam_size = 5` — Whisper paper Section 4.5 prescription; greedy is documented to fall into
  repetition loops on long-form audio. Previously-shipped `Greedy{best_of:1}` was below upstream default.
- `whisper.initial_prompt = ""` — upstream-documented failure mode ("Repetitions and Hallucinations
  when using prompt feature"); the jargon benefit it buys did not materialise on this corpus while the
  repetition cost did.
- `whisper.{entropy,logprob,no_speech}_thold`, `temperature_inc` — whisper.cpp defaults, verified 1:1
  through `whisper-rs` to identically-named `whisper_full_params` fields (Group D Probe 1).
- `vision.use_transcript = false` — VLM language-prior literature.
- `vision.transcript_window = causal` — Look-Ahead-Freedom as Temporal Non-Interference.
- `ollama.temperature = 0`, **no seed** — Q7 + Group D reconciliation.
- Phase 5.5 metrics: `compression_ratio(texts, algorithm='gzip')`, `self_repetition_score(dataset, n=4)`,
  `homogenization_score(texts, measure=...)`, `ngram_diversity_score(texts, num_n=4)` — signatures from
  `github.com/cshaib/diversity`, used **diagnostically, never as targets to maximize** (Deutsch et al.),
  with token length reported alongside every score (Shaib et al. length confound).

**Explicitly NOT locked by this PR** (recorded so the gap is legible):
- `vision.fps` — no domain evidence exists; decided by Phase 5.5.
- `vision.max_frames_per_request` — carried at 15; measured effect not statistically established.
- `whisper.model_path` — carried at `large-v3-turbo`; measured equal to `large-v3` at 2.3x lower cost.


### Gate Check (2026-08-24)

**1. Does research invalidate the PR's premise?** — **No.**
The premise was "lock a research-backed capture config for market-research corpus capture." Research
locked 7 of 10 dimensions with citations. One (`vision.fps`) proved **unlockable from literature** —
no published evidence measures frame sampling on static screencast content, and the premise it had
previously been chosen on was refuted. That does not invalidate the PR; it is why the PR carries a
Phase 5.5. The dimension moves from "locked by research" to "locked by measurement", and is recorded
as unlocked rather than quietly defaulted.

**2. Did research surface prerequisite PRs?** — **No prerequisites. One follow-on.**
- **PR-021** (vision-grounded ASR correction) is a **follow-on**, not a prerequisite. PR-020 does not
  depend on it, and PR-021 depends on PR-020 specifically so the audio-to-vision path is closed before
  the reverse is opened.
- **PR-018 is a dependency and is non-conforming**, and this needs stating plainly rather than being
  waved through. It was implemented without `PROCEDURE-pr-research.md`. However, **PR-020's own Phase 3
  supplies the research backing PR-018 lacked**: Q3 (VLM language-prior dominance) is the evidence for
  `use_transcript`, and Q4 (Look-Ahead-Freedom as Temporal Non-Interference) is the evidence for
  `transcript_window`. Those are precisely PR-018's two design decisions.
  **Assessment:** PR-018's mechanism is now research-backed by reference to this PR. It still needs the
  template retrofit and a Phase 1 state assessment of its own, but that is a documentation debt, not a
  correctness risk, and it does not block PR-020. Tracked in `docs/0.0/RESEARCH-BACKLOG.md`.

**3. Scope changes since the draft** (all from Phase 1 / Phase 4, all approved):
- `vision.fps` current value corrected 0.5 -> 2.0 (the `ml` profile's 0.5 applies only when named)
- `ffmpeg.chunk_duration_secs` added to scope (fps-coupled rationale, since found unsupported)
- Profile version-control added to scope (the config being "locked" existed only on one untracked machine)
- `ollama.seed` removal added
- Post-hoc repetition detection added (whisper output is ungated; vision is not)
- Verification criteria replaced with a research-backed set

**4. Implementation surface** (what clearing this gate authorises):
| Change | Files |
|---|---|
| Remove `seed` from config, request struct, client, and call site | `vtt-core/src/config.rs`, `vtt-core/src/vision.rs` |
| Correct the temperature-fallback comment | `vtt-core/src/whisper.rs` |
| Add post-hoc `compression_ratio` flagging over whisper output | `vtt-core/` (new; diagnostic only, never edits segments) |
| Create the locked profile **in the repo** + a documented deploy mechanism | new `config/profiles/` |
| Update tests | `vtt-core` test modules |

Phase 5.5 (empirical validation, incl. the `fps` decision) runs **after** implementation and is gated
separately.

**5. Known risks accepted at this gate:**
- `vision.fps` unresolved until Phase 5.5. The cost spread is real and large (106.2h at 2.0 vs 26.5h at 0.5).
- Phase 5.5's metrics measure **redundancy and diversity only**. No validated reference-free measure of
  faithfulness, novelty, or coverage exists — so Phase 5.5 cannot certify the corpus is *useful*, only
  that it is not degenerate. Stated rather than implied.
- The Whisper-paper co-location of beam-5 with the 2.4/-1.0 thresholds is `best-guess-given-constraints`
  (PDF extraction failed twice); the combination is independently verified in whisper.cpp's own
  constructor, which is the surface that executes.
- Tier B's 2-vote lenient quorum produced one **false refutation** (the silence-override claim),
  caught only because the agent flagged it and Group D re-checked against primary source. Other Tier-B
  verdicts in this PR carry that same residual risk.

- Premise still valid: ✓
- No prerequisite PRs surfaced: ✓ (PR-021 is a follow-on; PR-018 is research-backed by reference)
- User approved updated spec: **pending**
- Implementation cleared: **pending**


## Motivation

The capture config governs what the corpus **is**. Every downstream use inherits it, and
re-running the corpus is a ~26h+ GPU commitment, so the settings need to be right and
justified before that spend.

**Purpose of the corpus (drives every decision in this PR):** the data is **research
substrate for developing an automated trading system** — material the user and models read to
form hypotheses, design candidate models, and combine with other non-OHLCV sources. It is
**not** primarily a feature matrix for direct ML training ingestion.

This distinction is load-bearing and inverts several conclusions reached during the ad-hoc
session of 2026-08-24. Under a training-data framing, transient on-screen values were dismissed
as noise and richness was traded for parsimony. Under an exploratory framing, a price level
shown once may be exactly what generates a hypothesis, and richness may outrank cost.

**Why the current values cannot simply be ratified:** they were selected on 2026-08-24 using
metrics the author invented on the spot — duplicated 8-gram rate, audio-citation rate, and
"recall stratified by on-screen persistence". Those metrics were useful and internally
consistent, but none is research-backed, and under `docs/CONSTRAINTS.md` they are
`best-guess-given-constraints`. The measurements are recorded below as **inputs to research,
not as settled answers.**

## Scope

Lock, with research backing, the capture configuration for market-review video ingestion, and
ship it as a named profile.

**Config dimensions in scope:**
| Dimension | Current value | Status |
|---|---|---|
| `vision.fps` | **2.0** (code default; `ml` profile sets 0.5) | chosen ad hoc; **re-open under the exploratory framing** |
| `vision.max_frames_per_request` | 15 | tested, effect not statistically established |
| `vision.use_transcript` | `false` (ml profile) | mechanism from PR-018; value unresearched |
| `vision.transcript_window` | `causal` (ml profile) | mechanism from PR-018; value unresearched |
| `ollama.temperature` / `ollama.seed` | 0.0 / 42 | determinism; rationale not research-backed |
| `whisper.beam_size` | 5 | measured better here; not research-backed |
| `whisper.initial_prompt` | `""` | measured harmful here; mechanism not researched |
| `whisper.model_path` | `large-v3-turbo` | measured equal-or-better vs `large-v3`; not researched |
| `ollama.default_prompt` | references the transcript | produces disclaimer noise when `use_transcript=false` |
| `ffmpeg.chunk_duration_secs` | 180 | **added in Phase 1** — rationale is fps-coupled (768-frame cap at 2fps) |

**Also in scope:**
- Research-backed **evaluation metrics** for corpus value in exploratory (non-training) use. This
  is a must-answer question in its own right, and it is what Phase 5.5 measures against.
- A named profile capturing the locked values, with the evidence cited in comments.
- **Bring the locked profile into version control** (added in Phase 1) — all 11 profiles currently exist only on the desktop, untracked.
- Documentation of the locked config in `docs/ARCHITECTURE.md` and, where a value becomes a
  non-negotiable, `docs/CONSTRAINTS.md`.

**Explicitly out of scope:**
- Corpus survivorship bias (22 coverage gaps of unknown cause) — a data-provenance question,
  not a capture-config question. Tracked separately.
- The staging/runner tooling (`stage-videos.sh`, `run-corpus.sh`) — already built, unaffected.
- PR-018's retrofit and research backfill — its own work item.
- Actually running the corpus. This PR locks the config; the run is separate.
- `CLAUDE.md` § Remote server access staleness — recorded in `docs/0.0/RESEARCH-BACKLOG.md`.

## Dependencies

- **PR-019** (vibe-rails sync) — supplies `PROCEDURE-pr-research.md` and `prs/PR-TEMPLATE.md`,
  without which this PR cannot be written in final form. **Landed 2026-08-24 (`24f237c`).**
- **PR-018** (causal vision context) — supplies the `use_transcript` / `transcript_window`
  mechanism whose *values* this PR locks. Implemented but non-conforming; see
  `docs/0.0/RESEARCH-BACKLOG.md`.

## Architecture section implemented

`docs/ARCHITECTURE.md` — the processing-pipeline configuration surface. This PR does not change
the pipeline's structure; it fixes the operating point and records why.

## Verification criteria

Research-backed as of Phase 4 synthesis. Placeholder criteria from the draft are replaced.

**Config correctness**
- [ ] `ollama.seed` removed from `OllamaConfig`, `OllamaOptions`, and the request path; `temperature = 0` retained
- [ ] The determinism comment states the honest guarantee (no bit-reproducibility without batch-invariant kernels), not implied determinism
- [ ] `whisper.rs` temperature-fallback comment corrected: thresholds are retry triggers, not filters
- [ ] `whisper.beam_size = 5` and `whisper.initial_prompt = ""` set in the locked profile with cited rationale
- [ ] `vision.use_transcript = false`, `vision.transcript_window = causal` set in the locked profile
- [ ] `ffmpeg.chunk_duration_secs` unchanged at 180; `docs/0.0/DESIGN-log.md` rationale corrected to cite the visual-token budget rather than a 768-frame cap

**Version control** (Phase 1 finding: the config was untracked)
- [ ] The locked profile exists in the repo, not only on the desktop
- [ ] A documented mechanism deploys it to the server; the repo copy is the source of truth

**Post-hoc repetition detection**
- [ ] `compression_ratio` computed over whisper output, reference-free, `len(utf8)/len(zlib.compress(bytes))`
- [ ] Applied as a **diagnostic that flags**, not a filter that edits — segments are never silently truncated
- [ ] Known false-positive surface documented (counting 3.48, chorus 9.14, backchannel 3.22, litany 7.76 all exceed 2.4; segments under ~30 tokens can never be flagged)

**Phase 5.5 empirical validation**
- [ ] Metrics computed with the documenter's actual API signatures, not the paper's notation
- [ ] **Token length reported alongside every score**; no cross-arm comparison at unequal length
- [ ] Metrics reported as diagnostics; no config value chosen by maximizing one
- [ ] `vision.fps` decided by this measurement and the result recorded with its limits
- [ ] If Phase 5.5 contradicts a Phase 3 recommendation, the Amend branch is taken and the user approves

**Documentation**
- [ ] `docs/ARCHITECTURE.md` documents the locked config and what is deliberately not locked
- [ ] `docs/CONSTRAINTS.md` carries the Corpus Look-Ahead Freedom constraint
- [ ] `cargo test --workspace` passes

## Research backing

Tier-2. No dimension in the scope table currently has research backing; all values are
empirical-but-ad-hoc or inherited defaults.

**Candidate must-answer questions** (to be formalized, ranked, and tier-assigned in Phase 2 —
listed here so the scope is legible, not to pre-empt Phase 2):

1. How is the value of a video-derived text corpus evaluated when the intended use is
   **exploratory hypothesis generation** rather than model training? What published metrics exist?
2. What frame-sampling rate is defensible for screen-recorded chart/presentation content, and on
   what evidence? Is there prior art on sampling rates for slide/screencast understanding?
3. Does conditioning a vision model on the concurrent audio transcript improve or degrade the
   descriptions' value as **independent evidence**, and how is that assessed?
4. Is look-ahead contamination inside a generated corpus a recognized problem for exploratory
   financial research, or only for training/backtesting? What is the accepted mitigation?
5. What is the accepted method for detecting and quantifying **hallucinated repetition** in ASR
   output? (The 2026-08-24 session used an invented duplicated-8-gram metric.)
6. Does beam search vs greedy decoding have documented effects on ASR hallucination, and does
   `initial_prompt` have documented failure modes? (Both were observed here; neither researched.)
7. Determinism: what is the accepted practice for reproducibility of LLM-generated corpora, and
   is `temperature=0` + fixed seed sufficient given GPU non-determinism?

## Notes

### Empirical inputs from the 2026-08-24 ad-hoc session

Recorded as **inputs to research, not conclusions.** All metrics used were invented; all are
labeled `best-guess-given-constraints`. Single video (`2024_2_12.mp4`), single channel.

**fps sweep** (4 arms, same video, fps the only variable):

| fps | visual segs | span | corpus (38 videos) | recall of levels on screen >=30s |
|---|---|---|---|---|
| 2.0 | 326 | 7.5s | 106.2 h | 100% (reference) |
| 1.0 | 163 | 15s | 53.1 h | 92% |
| 0.5 | 82 | 30s | 26.5 h | 82% |
| 0.25 | 41 | 59s | 13.3 h | 69%, and plateaus at ~91% even for long-lived values |

**Cost model:** 1.949 s/frame, ~2% spread across a 4x fps range. Cost is linear in frame count.

**whisper matrix** (5 configs, same 900s clip):

| config | dup 8-gram | worst segment | whisper cost |
|---|---|---|---|
| turbo greedy (current default) | 4.4% | 0% | 42.4s |
| turbo greedy + initial_prompt | 4.9% | **90%** | 37.6s |
| turbo beam5 | **0.0%** | 0% | 44.6s |
| large-v3 greedy | 14.4% | 38% | 77.1s |
| large-v3 beam5 | 0.0% | 7% | 102.0s |

`initial_prompt` produced hallucinated repetition loops under **both** decoders and deleted real
content (a named support level and a divergence signal). Beam search eliminated repetition on
both models. Whisper is overlapped with vision, so its cost is hidden under the vision window.

**PR-018 verification:** audio-citation in visual segments fell 98% -> 30% with
`use_transcript=false`. The residual 30% is the model *announcing the absence* of a transcript,
caused by `ollama.default_prompt` still instructing it to cross-reference audio.

**Determinism:** with `temperature=0` + `seed=42`, two warm runs were byte-identical; a run
against a freshly-loaded model diverged and extracted 13 numbers where warm runs extracted 25.
Ollama unloads after ~5 min idle, so a cold start silently degrades one chunk per job.

### Known confounds in the above

- Single video, single channel. No claim of generality.
- All four fps arms and the whisper matrix ran **before** `temperature=0` was deployed, so they
  carry an uncontrolled sampling-noise component. Effect sizes were large relative to it, but it
  was not controlled.
- The `fps 0.5` recommendation was made under a **training-data** framing that this PR rejects.
