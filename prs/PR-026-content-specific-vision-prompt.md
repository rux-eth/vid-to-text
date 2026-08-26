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

### State Assessment (2026-08-25)

Everything below was verified today against the live laptop, the live desktop and the current
tree — not carried over from the PR draft.

**Current state**:

*The prompt surface.*
- `prompts/vision.txt` is the CRISPE prompt, md5 `d19050165a5fc3e343bb4f5f56a7c309`, **byte-identical
  on the laptop and on the desktop** (`~/vid-to-text/prompts/vision.txt`).
- The server's systemd unit sets `WorkingDirectory=/home/rux/vid-to-text` and the running process's
  `/proc/<pid>/cwd` confirms it, so the default `ollama.prompt_template_path = "prompts/vision.txt"`
  (`config.rs:559`) resolves to that file. **The CRISPE prompt is what the GPU is actually running** —
  verified, not assumed.
- `load_prompt_template` (`vision.rs:400`) **errors** on an unreadable path; only `None`/empty falls
  back to `ollama.default_prompt`. There is no silent-fallback hazard. Noted anyway: the desktop's
  `server.toml` still carries the **pre-CRISPE** prompt as `default_prompt` — a fossil that would
  become live if the path were ever unset.
- Nothing logs which prompt was loaded, so prompt provenance is absent from the logs as well as from
  the data.

*Provenance.*
- `CaptureInfo` (`types.rs:103`, built at `pipeline.rs:395`) records vision/whisper model, chunk
  duration, fps, sampling mode, the four adaptive parameters, `max_frames_per_request`,
  `use_transcript`, `transcript_window` and `temperature`. **No prompt field.** The PR's premise is
  confirmed.
- `Timeline.capture` is `Option` + `skip_serializing_if`, but `CaptureInfo` itself is not
  `#[serde(default)]`. Timelines carrying a capture block already exist on the desktop, and
  `review` / `rescore` read them, so any field added to `CaptureInfo` must be `Option` or
  `#[serde(default)]` or those timelines stop deserialising. Checkpoints are unaffected —
  `ChunkCheckpoint` (`checkpoint.rs:11`) holds segments only.

*Selection mechanism — already exists.*
- `ollama.prompt_template_path` is a normal config key, and `load_profile` (`config.rs:838`)
  deep-merges a profile's TOML over the base config. **A profile can already point at a different
  prompt file**; the PR's "per-content-class prompt selection" is mostly a matter of using this, not
  building it. The general prompt stays the base-config default, so non-chart video is unaffected
  by construction.

*Corpus and baseline data.*
- All **68** corpus videos are staged on the desktop at `/home/rux/vtt-corpus` (14 GB); no LAN
  staging is needed for Phase 6.
- `_pending_corpus.py` reports **`done=0 pending=68 stale=0`**. **No capture under the current prompt
  survives anywhere, on either machine.** The 12:37–14:03 corpus run (10 episodes, all `OK` in
  `transcribe-runs/corpus-20260825-123726.csv`) had its server results deleted at 14:10 and left no
  cache entries; `transcribe-runs/superseded/` does not exist.
- Surviving jobs under the locked profile **and** the current prompt are **`clip900` only**:
  `735042d7` (precision 0.728), `402327e8` (0.220) and `cc423d58` (0.226) — the two low figures being
  the numeric-degeneration runs PR-023/PR-025 documented — plus `f7ba8b47` (no fidelity block) and
  `ecc53c3f` (the `use_transcript = true` experiment arm).
- Every corpus-episode job cited in PR-023's and PR-024's tables (`9dcbd12e`, `e01dc38f`, `3334b1f8`,
  `857f0ff6`, `e85b456d`) is **gone from the desktop**. Those tables can be trusted as recorded but
  **cannot be re-derived from data**.
- Eight orphan pre-PR-020 cache entries survive (`2023_10_2`, `10_9`, `10_23`, `11_6`, `11_20`,
  `11_27`, `12_4`, `12_18`, all `processed_at` 2026-03-31, no `capture` block). They are keyed on a
  different realpath than the current video directory, so the runner neither uses nor reports them —
  which is why `stale=0` rather than `stale=8`. Harmless, but the 2026-08-25 archive note's "removed
  from both machines" is not literally true.

*Tests.* `cargo test --workspace`: **228 passed, 0 failed**, 13 ignored (12 ffmpeg/GPU-dependent + 1
system test).

**Assumptions at PR draft time**:
- That `CaptureInfo` records no prompt identifier — **holds**.
- That `run-corpus.sh`'s profile check would not notice a prompt change — **holds only for the
  in-place case** (see Stale assumptions).
- That the 13 current-prompt captures were deleted and Phase 6 must regenerate its baseline —
  **holds, and is broader than written**.
- That PR-023 delivers "the fidelity diagnostic and `vid-to-text review`, which Phase 6 measures
  with" — **the tools exist and work; the clearance to tune with them does not** (see New constraints).

**Stale assumptions** (where current state disagrees with the PR draft):

1. **"The runner would not notice a prompt change" is only half true.** `_pending_corpus.py` marks a
   video done only when `meta.json.profile` equals the profile now requested. If the new prompt ships
   as a **new profile** (a profile setting `ollama.prompt_template_path`), the existing skip logic
   already treats every video as pending and reports the old ones by name. Only an **in-place** edit
   of `prompts/vision.txt` under the same profile name goes unnoticed. This does not remove the need
   for prompt provenance in `CaptureInfo` — a timeline must be self-describing regardless of how it
   was produced — but it shrinks the runner-side work to "confirm and pin", not "build".

2. **"Roughly 20 minutes of GPU on two videos" needs restating as a per-cycle cost.** Measured today
   at 0.32–0.38× realtime across ten episodes, a 20-minute episode costs ~7 min and a 40-minute one
   ~14 min. Two episodes is therefore ~15–25 min **per arm**, and every Phase 6 cycle pays it again
   for every arm it compares. The figure is right; the framing ("the first cycle's zero point") is
   not — there is no free baseline.

3. **The PR's own measured motivation cannot be re-verified.** The 76% / 61% / 33% boilerplate rates
   and the `2024_4_8` cursor-hover example were computed on the 13 timelines that were then deleted,
   and the underlying job dirs are gone too. The finding stands as recorded; it cannot be recomputed.
   Phase 6's before/after therefore measures against a **regenerated** baseline, which will not be
   bit-identical to the one the motivation was written from (`temperature = 0` is repeatable in
   distribution, not bit-exact — `docs/ARCHITECTURE.md` § Capture Configuration).

**New constraints** (learned from prior PRs and from today's state):

1. **The fidelity metric is not currently cleared for tuning — this is a documented rule, not a
   caveat.** `docs/ARCHITECTURE.md` § Fidelity Diagnostic: *"The metric is not trusted for tuning
   until that κ has been reported."* `docs/0.0/DESIGN-log.md` 2026-08-25 decision 4: *"nothing ships
   without the κ."* PR-023's calibration was instead done **without the user** (the review sheet was
   found unworkable) by classifying unsupported facts by distance to the nearest on-screen number,
   and **κ was never computed**; PR-023's verification criterion "Calibration done: ~150 human
   judgments, κ reported" is still unchecked. `cohen_kappa` is implemented and tested
   (`vtt-client/src/review.rs:127`), so the instrument exists and only the labels are missing.
   **Consequence for this PR:** naming fidelity F0.5 as the Phase 6 primary measure requires first
   reporting κ, or choosing a different primary measure, or amending the rule with the user. This is
   a harder blocker than the "precision is depressed by its own matching rules" caveat the PR cites.

2. **PR-023's study never completed.** Its scope-D study (grid over `scene_threshold` × `max_gap_secs`
   plus fixed 0.5 fps, objective "F0.5 per GPU-hour with a hard precision floor") has no results table
   in the PR file, all four of its study-related verification criteria are unchecked, and the job dirs
   are gone. PR-025 refers to "7 completed runs" of study arms, so arms ran and were never analysed.
   PR-026's upstream dependency on PR-023 therefore delivers the **diagnostic**, not the **validated
   objective** — the one artefact that would have answered PR-026's blocking question by precedent.

3. **Single-video results do not replicate on this corpus — measured, in-repo.** PR-024's headline
   effect (invented values 3.4% → 1.6% on `2024_4_8`) **reversed** on the confirmation clip
   (1.1% → 5.3% on `2024_6_24`) and pooled to nil. This is direct evidence for the PR's held-out
   requirement and sets a floor on how many videos a cycle needs; a one-video win is not a result.

4. **Circularity discipline (PR-024).** Never score a change with a metric fed by the thing changed.
   If a prompt revision tells the model which numbers to report, fidelity precision inherits exactly
   the circularity that made grounded precision uninformative — and `FidelitySummary.ocr_grounded`
   only records the OCR-grounding case, not a prompt-induced one.

5. **Measure with the implementation's own tokenizer (PR-025).** A first threshold pass used a
   tokenizer that split at unit suffixes and nearly shipped a cap of 24 instead of 40. Any
   boilerplate-rate, verbosity or numeric measure in Phase 6 must fix one stated tokenizer/regex and
   use it for every cycle, or cycles are not comparable to each other.

6. **Profile TOML table re-homing (PR-022).** A bare key appended after a nested table silently
   re-homes under it — this shipped once, putting `use_transcript` under `[vision.adaptive]`, and was
   caught only by the provenance block on the first run. If this PR puts `prompt_template_path` in a
   profile it must sit explicitly under `[ollama]` and be pinned by a test that loads the repo profile
   through the real merge path (`config.rs:1474` is the existing pattern to copy).

7. **Backward-compatible provenance (PR-022).** Any field added to `CaptureInfo` must be `Option` or
   `#[serde(default)]`, or the surviving `clip900` timelines stop loading in `review` / `rescore`.

8. **No tooling deploys the prompt to the GPU.** `config/deploy-profiles.sh` copies `*.toml` only, and
   `~/vid-to-text` on the desktop is **not a git checkout** (re-verified today: `fatal: not a git
   repository`) — it is an unversioned file copy. A prompt revision reaches the model only by manual
   `scp`, with nothing verifying that what ran is what was intended. For a PR whose entire method is
   "revise the prompt, re-measure, repeat", this is a live correctness hazard on every cycle and the
   PR does not name it.

9. **Prompt length enters the context pre-flight.** `ollama.prompt_reserve_tokens = 4096` bounds
   prompt text at `vision.rs:373`; measured usage at 15 frames of 1080p was 34,726 tokens of 65,536
   ungrounded (45,526 grounded). Headroom is ample, but a substantially longer prompt must be
   re-checked against the pre-flight rather than assumed to fit.

10. **Generation-time editing only.** Segments Are Immutable After Merge; any output-shaping this PR
    introduces belongs where `truncate_repetition` / `truncate_numeric_run` already live.

**Observation for Phase 2, recorded as an observation and not a conclusion.** The PR states that
PR-020 Phase 5.5 limits the diversity metrics, and it does — but the *specific* reason may not
transfer. Finding 1's fatal confound was that length and time-coverage **cannot both be held constant
when fps changes**, because fps changes tokens-per-second-of-video by construction. A prompt change at
fixed sampling holds frames and time coverage constant by construction, so only the length confound
remains — and length is itself an intended outcome here, not a nuisance. That does not make those
metrics valid for this question; it means Phase 2 must re-derive whether they are, rather than
inheriting a "no" that was reached about a different comparison.

**Downstream contracts**: **none** — no PR depends on this one. Verified mechanically:
`grep -rl "PR-026" prs/ docs/` returns exactly three files — this PR file, its `docs/0.0/ROADMAP.md`
row (last row of the Process & Config Lock phase, no dependents), and its `docs/0.0/RESEARCH-BACKLOG.md`
row. Nothing declares PR-026 as a dependency and PR-026 unblocks nothing.

**Upstream contracts** (what this PR consumes, and whether it is actually delivered):
- **PR-022** (landed `c37e8a1`) → `Segment.frames` + `CaptureInfo` + the adaptive segment shape.
  **Delivered**, live on the desktop.
- **PR-023** (landed `f570aec`) → the fidelity diagnostic, `review`, `rescore`. **Partially
  delivered**: the tools exist and are tested, but the κ that `docs/ARCHITECTURE.md` requires before
  the metric may be used for tuning was never reported, and the study that would have validated an
  objective never completed. This is the upstream gap behind PR-026's own blocking question.
- **PR-025** (landed `f45ce72`) → `truncate_numeric_run`, so cycles are not confounded by enumeration
  collapse. **Delivered** (`vision.max_numeric_run = 40`).

**Documentation drift found** (not this PR's scope; recorded so it is not lost):
- `prs/PR-022`, `PR-023`, `PR-024`, `PR-025` all still read `**Landed-in:** (not yet landed)` though
  they landed in `c37e8a1` / `f570aec` / `f45ce72`. This may be deliberate — no version tags exist yet
  (`docs/0.0/RESEARCH-BACKLOG.md` § Known documentation gaps) and `docs/VERSIONING.md` §4 keys the
  field to a released version — but four PRs in a row carrying it warrants an explicit decision.
- `CLAUDE.md` § Remote server access still states there is "no systemd unit, launch script, or
  persistent log file". Re-verified false today: `vtt-server.service` is enabled and active with
  `WorkingDirectory=/home/rux/vid-to-text`. Already recorded as a known gap; re-confirmed.
- `docs/0.0/RESEARCH-BACKLOG.md` files PR-026 under **"Tier 1 — Implementation-Ready"** while marking
  it `design-research ✗`, which is the Tier-2 ("Research-Pending") condition. Misfiled; see the
  path-tier checkpoint.

**Path-tier checkpoint**: the PR header says **Tier-2 (full path)**; `docs/0.0/ROADMAP.md` carries no
tier column; `docs/0.0/RESEARCH-BACKLOG.md` has it under the Tier-1 heading with `design-research ✗`.
Per the procedure's "the newer, more specific source wins", the PR file — written today (`b14a10f`) and
specific about an unresearched fix, a blocking measurement question and a Phase 6 — governs.
**Confirmed Tier-2; all five phases run, plus this PR's Phase 6.** The RESEARCH-BACKLOG row should move
to the Tier-2 table.

**Exit criteria assessment**: no stale assumption changes the PR's *premise* — the prompt is
content-mismatched, the provenance gap is real, and both are confirmed against live state. Two items
are severe enough to shape Phase 2 rather than pass silently: **the fidelity metric's tuning
clearance (New constraint 1) — which is the blocking question the PR already names, now with a
documented rule behind it rather than only a caveat** — and **the absence of any deployment path for
the prompt itself (New constraint 8)**, without which no Phase 6 cycle can be trusted to have measured
what it thinks it measured. Neither loops to `PROCEDURE-design-planning.md`; both are Phase 2 inputs.

### Research Questions (2026-08-25)

**Reading of the Phase 6 extension, stated so it can be corrected.** The extension says the primary
measure "is set in Phase 2". Q1 below *is* that question, and its answer comes from the Phase 3 round;
Phase 2 fixes the question, the success criteria a valid answer must meet, and every Phase 6 parameter
that does **not** depend on research (unit, sets, cycle cap, error method, stopping rule). The primary
measure is therefore **selected in Phase 3, locked at the Gate Check, and never revised after cycle 1**
— which satisfies the extension's actual guard ("fixed BEFORE the first cycle, so iteration cannot
chase noise"). If the intent was instead to pick a measure now without researching it, say so and I
will, but that would reproduce the PR's own named hazard.

**Two measurement facts established while scoping** (they narrow Q1 and are recorded here, not in
Phase 3, because they are internal measurements rather than research):

1. **Prompt arms are segment-aligned by construction — verified, not argued.** Frame selection is
   pure ffmpeg (scene scores over decoded frames), so it is model-independent and deterministic.
   Four independent `clip900` runs under the locked profile — `735042d7`, `402327e8`, `cc423d58`,
   `f7ba8b47` — produce **bit-identical segment spans and bit-identical per-segment frame lists**
   (span sha256 `777ba6fd…`, frame-list sha256 `4ec1c47d…`, all four). Only the text differs.
   **Consequence:** a prompt comparison is a *paired* comparison over the same segments, over the
   same instants, at the same coverage. The confound that invalidated PR-020 Phase 5.5 — that length
   and time-coverage cannot both be held constant when fps changes — **cannot arise here**, because
   fps is not what changes. This does not by itself make any particular metric valid; it removes the
   specific reason the previous round's metrics were invalid, and it makes paired variance reduction
   available, which PR-020 did not have.

2. **The upstream precedent is thinner than PR-023's file implies.** `/home/rux/vtt-exp/study/runs/manifest.tsv`
   contains **two completed cells of the twenty-one** the study designed (`2024_2_19` × `study-t05-g15`
   and `× study-t05-g30`), and both job directories are gone. So PR-023's study did not "not get
   analysed" — it barely ran. Nothing in-repo has ever validated an objective for comparing vision
   output across arms. Q1 cannot be answered by precedent and must be researched.

   Useful by-product: those two cells give the only wall-clock figures for a 900 s excerpt —
   420 s and 360 s (0.47× and 0.40× realtime) on frame-heavy `t05` arms, against 0.32–0.38× measured
   on ten full episodes today under the locked profile. Phase 6 budgets at **0.40×**.

**Must-answer:**

1. **What is the primary measure for Phase 6, and what is its measurement error?** *(the PR's Q5,
   promoted to first: it blocks Phase 6, and Phase 1 showed it is also blocked upstream by PR-023's
   unreported κ.)*
   *Success criteria — a valid answer names a measure that is all five of:*
   (a) **segment-decomposable**, so the paired bootstrap below applies;
   (b) **non-circular with respect to a prompt change** — PR-024's lesson: a metric fed by the thing
       being changed cannot score it, and a prompt that names which numbers to report would corrupt
       fidelity precision exactly as OCR grounding did;
   (c) **sensitive to at least one of the two defects this PR targets** (absent-human boilerplate;
       fabricated chronology), or explicitly paired with a secondary measure that is;
   (d) **computable offline from persisted artifacts** (`timeline.json`, `fidelity.json`, `ocr.json`,
       thumbnails — all written per job today), so a cycle can be re-measured without re-running the GPU;
   (e) **either already cleared for tuning use, or delivered with the specific step that clears it.**
   *Also required:* an error estimate, and an honest statement of what the measure cannot see.

2. **Does removing presuppositional instructions improve factual accuracy, or only reduce verbosity?**
   *Success:* cited evidence on VLMs that *removing* instruction presupposition (not only adding it)
   changes accuracy, with an effect size and a measurement method that can be copied. A finding that
   only verbosity moves is an equally good answer — it would narrow this PR's claim without changing
   its scope. The problem direction is already proven (arXiv:2604.21911, Qwen2-VL-7B); the remedy
   direction is what is unverified, and that paper's own mitigation is fine-tuning.
   *(As written in Phase 2 this line cited "94.1% → 56.7%". Phase 3 found that framing is wrong —
   see the correction opening the Round 1 findings. Left annotated rather than silently rewritten,
   so the record shows what Phase 2 believed when it set the question.)*

3. **What prompt structure most improves factual grounding for dense information displays** (charts,
   dashboards, screen recordings)?
   *Success:* at least two cited options compared by measurement — e.g. free narrative with domain
   framing; schema-constrained extraction plus a short narrative; two-pass extract-then-narrate — each
   with its failure modes, **and an explicit statement of whether the evidence covers open-ended
   description or only VQA/extraction.** The PR flags that gap; the answer must close or admit it.

4. **Does explicitly describing an interface's conventions reduce false inference about it?** *(the
   cursor-hover chronology defect, stated as a research question: that a TradingView header shows the
   hovered candle, that the right edge is unfilled future space.)*
   *Success:* evidence that supplying interface semantics in-prompt changes model behaviour on that
   interface — or an explicit finding that no such evidence exists, in which case this becomes a
   Phase 6 empirical arm rather than a research-backed choice, and is labelled as such.

5. **How should observation be separated from inference in the output**, given the corpus is read by
   humans and models rather than scored by a benchmark?
   *Success:* a cited convention for epistemic marking that a downstream reader can parse, plus whether
   it costs accuracy. **A valid answer must explain why asking is insufficient** — the shipped prompt
   already instructs "Clearly separate observation from inference. State what you see, then state what
   you think it means and why", and the cursor-hover fabrication happened anyway, narrated as
   observation.

6. **Can the cursor-hover chronology error be detected mechanically?**
   *Success:* either a detector design grounded only in artifacts already recorded (per-frame OCR items
   with boxes and scores, frame timestamps, segment text) with an estimated false-positive rate; or an
   explicit finding that it cannot be built, in which case the defect is handled in-prompt and measured
   by hand on a stated sample. Either outcome satisfies the PR's scope item; only silence does not.

**Dependencies:**
- **Q1 blocks Phase 6** and is the gating question for the whole PR.
- Q6 depends on Q1 (whether the detector is a component of the primary measure or a secondary one).
- Q5 depends on Q3 (a schema answer to Q3 may subsume Q5 entirely).
- Q2, Q3, Q4 are mutually independent.

**Research plan (depth tiers):**
- **Q1 — Tier A probe → escalate to Tier C if the probe does not resolve the choice.** Escalation
  pre-authorised rather than argued later, because: it blocks Phase 6 entirely; the in-repo precedent
  that would have settled it never ran (2 of 21 cells); and PR-020 Phase 5.5 already spent one full
  round on metrics that proved invalid for their comparison. Being wrong here is expensive early and
  cheap to avoid.
- **Q2 — Tier A → B if inconclusive.** Load-bearing (it is the PR's central accuracy claim) and the
  cited literature answers the opposite direction.
- **Q3 — Tier A → C if the probe finds only secondary sources.** Highest-leverage content question;
  the PR already records that the sources found so far are secondary or general-purpose.
- **Q4 — Tier A → B if load-bearing uncertainty remains.**
- **Q5 — Tier A.** Reassess after Q3 lands; may be answered by Q3's structure finding.
- **Q6 — Tier A** (external precedent for temporal-consistency checking of generated narration) plus
  an internal feasibility pass over the recorded artifacts.
- **Rounds:** Round 1 = Q1, Q2, Q3 as parallel Tier-A probes. Round 2 = Q4, Q5, Q6 after Round 1
  (Q5 needs Q3; Q6 needs Q1). Escalations are decided at the end of each round, not mid-question.
  **Group D (MCP verification)** runs after both rounds, before Phase 4 is locked.
- **Default tier: A.** Escalations above A pre-authorised: Q1 → C, Q3 → C, Q2 → B, Q4 → B, each with
  the reason recorded above. **Tier E is not proposed and would need explicit approval.**

**Internal — no web round** (resolved in Phase 4 synthesis; listed so the boundary is explicit):

- **I1 — Prompt provenance field shape.** What identifier and hash go into `CaptureInfo`. Constrained
  by Phase 1: must be `Option`/`#[serde(default)]` or the surviving `clip900` timelines stop
  deserialising. In-repo pattern exists (`Timeline.capture`, `Segment.frames`).
- **I2 — Prompt selection mechanism.** Phase 1 established the mechanism already exists
  (`ollama.prompt_template_path` + profile deep-merge). The remaining choice is *new profile per
  prompt* versus *editing the locked profile*. Two facts already point one way and are recorded now
  so Phase 4 does not re-derive them: a new profile makes `_pending_corpus.py`'s existing skip logic
  correct with no change, and it makes PR-023's `study-023.sh` harness — which drives (clip × profile)
  jobs against the server from a manifest — reusable for (excerpt × prompt-arm) cycles unchanged.
  Constrained by PR-022's table-re-homing defect: `prompt_template_path` must sit explicitly under
  `[ollama]` and be pinned by a test loading the repo profile through the real merge path.
- **I3 — How the prompt reaches the GPU.** Phase 1 New constraint 8: nothing deploys `prompts/`, and
  `~/vid-to-text` on the desktop is an unversioned copy. `config/deploy-profiles.sh` is the in-repo
  pattern to extend (checksum-compare, `--apply`, verify-after). Without this, no Phase 6 cycle can be
  shown to have measured the prompt it claims to have measured.
- **I4 — κ clearance.** Whether fidelity is cleared for tuning by reporting the κ that
  `docs/ARCHITECTURE.md` requires. `cohen_kappa` is implemented and tested
  (`vtt-client/src/review.rs:127`); PR-023's protocol is ~150 disagreement-first items. **This is a
  user decision, not a research question** — the review sheet was found unworkable once already, and
  the alternatives are: do the labelling, choose a primary measure that does not need it, or amend the
  rule in `docs/ARCHITECTURE.md` with the reason recorded. Feeds Q1(e).

**Explicitly excluded from this round** (nice-to-have, deferred):
- `prompts/format.txt`'s identical content-mismatch question — the PR's own note; its own work item.
- Fine-tuning / preference optimisation — out of the PR's scope by its own statement.
- Re-opening `use_transcript` or OCR grounding — measured in PR-024 (worse, and null); reopening them
  would confound Phase 6 with a second variable.
- Frame downscaling and 720p legibility — a recorded gap, not a prompt dimension.
- Whether Trend/Magnitude claims need their own instrument — PR-023 deferred it pending a human review
  that never happened; reopening it here would widen Q1 past what blocks this PR.
- Bringing `~/Documents/seer_archive/bin/` under version control — recorded gap, separate work item.
- Backfilling the `Landed-in:` fields on PR-022..025 and the RESEARCH-BACKLOG tier misfiling — Phase 1
  drift findings, not this PR's scope.

### Phase 6 Parameters Fixed In Phase 2

Everything here is fixed **now**, before any cycle, and is not revisable during iteration. Only the
primary measure itself is deferred — to Q1's answer, locked at the Gate Check.

**Unit of measurement: 15-minute excerpts (minutes 5–20).** The PR-023 precedent, and the reason a
multi-cycle Phase 6 is affordable at all: at the measured 0.40× realtime a full episode costs ~13 min
of GPU per arm against ~6 min for an excerpt. Three such excerpts already exist on the desktop at
`/home/rux/vtt-exp/study/`; the rest are cut with the same recipe. *Recorded limitation:* an excerpt
cannot exhibit whole-episode structure, so nothing about cross-chunk continuity can be concluded from
Phase 6.

**Universe: the 59 episodes at 1920×1080.** The nine 1280×720 episodes are excluded from the primary
comparison because the resolution effect is measured and lands on exactly the signal Phase 6 tests —
720p loses ~26% of OCR-readable numbers (measured 2026-08-25, recorded in
`~/Documents/seer_archive/bin/_store_result.py`). The mechanism: TradingView header glyphs are ~10 px
tall at 1080p (PR-024), so ~6.7 px at 720p — at or under the ~8 px floor below which OCR recovers
nothing, which is the same floor that made the six 360p episodes unusable. Mixing resolutions would
confound a prompt effect with a legibility effect. **One 720p excerpt is carried as a separate generalisation check**, reported
alongside but **not** part of the stopping rule, so the prompt is not silently validated only on the
resolution where it is easiest.

**Split rule (stated before the sets were drawn; deterministic, so it cannot be re-run to taste):**

1. Universe `U` = the 59 × 1080p episodes.
2. Sort `U` ascending by measured triggers/min — the corpus's dominant covariate, and the one PR-022's
   whole design rests on (6.7× spread between videos). Ties broken by episode date ascending.
3. Take the eight episodes at ranks `floor(i × 58 / 7)` for `i = 0..7` — evenly spaced across the full
   change-rate range, both extremes included.
4. Walking those eight in sorted order, alternate: ranks `i = 0, 2, 4, 6` → **tuning**;
   `i = 1, 3, 5, 7` → **held-out**. Both sets then span the range, and neither can be systematically
   calmer or busier than the other.
5. **Contamination rule:** `2024_4_8` is where the cursor-hover defect was found by reading output, so
   it is not eligible for held-out; if it lands there it swaps with the adjacent tuning pick. Likewise
   `2024_2_19`, `2024_6_24` and `2025_05_26` carry prior published results (PR-023 study cells and
   PR-024's A/B), so they are preferred for tuning and, if drawn into held-out, are swapped the same
   way. Held-out must be genuinely unseen.
6. Generalisation check: the 720p episode with the **median** triggers/min of the nine.

**The change-rate measurement behind step 2.** Run on the desktop today (CPU only, no GPU, no model)
over all 68 staged episodes, replaying `vtt-core/src/ffmpeg.rs` `select_expression()` semantics
exactly rather than approximating them: candidates at `vision.fps = 2`, per 180 s chunk, kept when
first-of-chunk, or `t − prev_selected ≥ max_gap_secs (15)`, or `scene > scene_threshold (0.08)` and
`t − prev_selected ≥ min_trigger_interval_secs (2)`; a frame is counted as a *trigger* only when the
scene clause is what selected it. This is the PR-025 discipline applied to a covariate — measure with
the implementation's own definition, not a plausible substitute. Validation: the replay yields
5.70 kept-frames/min on `2023_10_23` against PR-022's corpus figure of 15.4 frames per 180 s chunk
(= 5.13/min), i.e. the same regime.

*(A first version of this probe returned 0 triggers on every video. Cause: without the `eq(n,0)`
term ffmpeg's `prev_selected_t` stays NaN, so `gte(t − prev_selected_t, 2)` is false forever and
nothing is ever selected. Recorded because the wrong number was briefly on screen — the same class of
error PR-025 caught with its tokenizer.)*

**Number of cycles: capped at 6**, with the stopping rule free to end it earlier. Budget at 0.40×
realtime and four 900 s tuning excerpts per arm: **~24 min GPU per cycle**, plus a ~24 min cycle-0
baseline and at most two ~24 min held-out confirmations ⇒ **≈ 3.2 GPU-hours** for the whole of
Phase 6. That is the same order as PR-023's approved ≤ 3 GPU-hour study budget. OCR and thumbnails are
produced per job automatically (`[fidelity] enabled = true` on the server) and are CPU-side, overlapped.

**Error estimate: paired segment-level bootstrap**, 10,000 resamples, fixed seed, 95% CI on the
**difference** between the candidate prompt and the current best, resampling segment *indices* and
scoring both arms on the same drawn segments. Pairing is valid because arms are segment-aligned by
construction (verified above). The existing helper — `analyze-023.py` `bootstrap()`, 1,000 resamples,
seed 23 — resamples **one arm** and yields a CI on that arm's F0.5, not on a difference; the paired
version is an extension of it, not a rewrite, and the resample count is raised because the tail
estimate is what the stopping rule reads. **This is why Q1(a) requires a segment-decomposable
measure** — a corpus-level scalar cannot be bootstrapped this way.

GPU-hours per cycle come from the server log's `[timing] pipeline total: <n>s` lines, which
`analyze-023.py` already parses and which persist under `~/.vid-to-text/logs/`.

**Stopping rule, made operational.** After each cycle, compute the paired 95% CI of the primary
measure's change against the *previous best* prompt on the **tuning** set. Iterate while the CI
excludes zero in the improving direction. **Stop** when it includes zero — the improvement is inside
measurement error — or at cycle 6, whichever comes first. Then, and only then, measure the winner on
the held-out set.

**Overfitting guard, made operational.** The held-out set is measured **at most twice**: once on the
candidate winner, and once more only if the first fails and a specific alternative is proposed. Every
held-out look is recorded with its date and result. A prompt that wins on tuning and not on held-out is
**rejected, and the PR ends with the current prompt retained** — not with a third look. Reporting a
retained-prompt outcome is a valid result for this PR and is stated here so that it cannot later be
treated as a failure to be iterated away.

**What each cycle records** (so the final prompt arrives with its derivation, per the extension):
prompt version identifier and content hash; what changed and the reason; the arm's job ids; the primary
measure with its paired CI; the secondary/guardrail measures; wall clock; and any degeneration the
PR-025 guard truncated.

**The sets, drawn by that rule and fixed as of 2026-08-25 — before any cycle has run.**

Sweep result over all 68 staged episodes: 59 × 1080p spanning **0.65 – 3.77 triggers/min** (median
2.06), and 9 × 720p. The 5.8× spread confirms PR-022's finding that no single fixed rate fits this
corpus, and is why the split is stratified on this covariate rather than on date or duration.

| rank in sorted universe | triggers/min | set | episode | duration | kept frames |
|---|---|---|---|---|---|
| 0 | 0.65 | **tuning** | `2024_4_15` | 43.4 min | 191 |
| 8 | 1.14 | held-out | `2024_5_13` | 30.6 min | 145 |
| 16 | 1.68 | **tuning** | `2024_9_2` | 32.7 min | 167 |
| 24 | 2.00 | held-out | `2025_09_22` | 39.1 min | 202 |
| 33 | 2.19 | **tuning** | `2025_5_12` | 35.6 min | 190 |
| 41 | 2.55 | held-out | `2025_10_21` | 39.2 min | 222 |
| 49 | 2.81 | **tuning** | `2025_10_13` | 45.6 min | 259 |
| 58 | 3.77 | held-out | `2025_11_17` | 25.2 min | 161 |

- **Tuning set:** `2024_4_15`, `2024_9_2`, `2025_5_12`, `2025_10_13` (minutes 5–20 of each).
- **Held-out set:** `2024_5_13`, `2025_09_22`, `2025_10_21`, `2025_11_17` (minutes 5–20 of each).
- **720p generalisation check:** `2023_11_20` (2.00 triggers/min, the median of the nine).
- **Contamination rule did not fire** — none of `2024_4_8`, `2024_2_19`, `2024_6_24`, `2025_05_26`
  was drawn into either set, so no swap was needed and neither set carries a prior published result.

**Cross-validation of the replay against PR-022's independent measurement.** The three clips PR-023
selected by change-rate percentile score 1.10 / 2.09 / 3.24 triggers/min here against PR-022's
1.24 / 2.52 / 3.93 — **the same ordering, and the same p10 / p50 / p90 placement** in this sweep
(1.02 / 2.06 / 3.13). Absolute magnitudes run 11–18% lower, consistent with a different counting
convention (this replay restarts selection per 180 s chunk, as the pipeline does). Rank agreement is
what the split rule consumes; absolute agreement is not claimed. Recorded rather than smoothed over.

**Raw sweep data:** `~/Documents/seer_archive/corpus-scene-rates-20260825.tsv` (68 rows: episode,
width, duration, candidates, kept, triggers, triggers/min, kept/min). Outside version control, like
the rest of the corpus tooling — the recorded gap in `docs/0.0/RESEARCH-BACKLOG.md`. The split above is
reproducible from that file plus the rule; if the file is lost, the held-out guarantee becomes
unverifiable, so it is named here explicitly.

**Excerpts still to cut:** all eight (the three excerpts already on the desktop are `2024_2_19`,
`2024_6_24`, `2025_05_26` — none of which the rule drew). Cutting is CPU-only, minutes, with the
recipe already used for the study clips.

### Findings — Round 1 (Tier A probes, 2026-08-25)

**A correction to this PR's own Motivation, found by reading the cornerstone source.**
The Motivation says: *"on Qwen2-VL-7B … object recognition falls from 94.1% to 56.7% under adversarial
presupposition."* That is not what those numbers are. In `arXiv:2604.21911` Table 1 (HalluScope),
**94.1% is `Rec_pos`** — recognition accuracy on objects that *are* present — and **56.7% is `AdP`**,
"the proportion of samples for which the model correctly rejects the false presupposition". They are
two different metrics on two different subsets, not one metric before and after. The paper's actual
argument is the *contrast*: `Rec_pos` and `Rec_rnd` "remain consistently high, above 85%" while `AdP`
is far lower, which isolates presupposition as the cause rather than perception — and the paper states
plainly that "hallucinations in modern LVLMs are predominantly driven by textual instructions, even
when visual perception remains reliable." **The qualitative claim survives; the sentence stating it
does not.** It must be rewritten in Phase 4.

---

**Q1: What is the primary measure for Phase 6, and what is its measurement error?**

*Options considered:*

- **Option A — PR-023 fidelity precision/recall/F0.5, paired per segment.**
  - Sources: in-repo PR-023; CHOCOLATE (Huang et al., ACL 2024 Findings); *Measuring the Measurers*
    (arXiv:2406.17115), which rejects holistic LLM scoring — "current LLMs often struggle to assign
    consistent and accurate scores" — and recommends decomposing evaluation into "explicit, objective
    steps"; *The Devil is in the EOS* (arXiv:2507.20077).
  - Pros: already built, tested, and produced automatically per job; re-scorable offline from
    `ocr.json` with no GPU; segment-decomposable, so the paired bootstrap applies; expressed as
    **rates**, not counts, so it is not trivially inflated by longer output.
  - Cons, concrete: (i) blocked for tuning by the κ rule in `docs/ARCHITECTURE.md`; (ii) PR-023
    measured 44% of its own unsupported facts to be **metric** false positives, and this is not a
    local defect — *Do More Details Always Introduce More Hallucinations?* (arXiv:2406.12663) states
    in its abstract that prior work "attribute[s] the occurrence of OH to the inclusion of more
    details" but that "our study finds **technical flaws in existing metrics, leading to unreliable
    evaluations of models and conclusions about OH**"; (iii) **the metric's error is not
    arm-independent** — arXiv:2507.20077 acknowledges the confound directly, that "the increase in
    descriptiveness comes with an expected increase in the rate of hallucination" and that CHAIR-style
    scores are "likely to be inflated"; since this PR's expected effect *is* a change in verbosity, a
    terser prompt could win on precision without being more accurate; (iv) circular if the new prompt
    names which numbers to report (PR-024's lesson).
  - Mitigation the sources supply: arXiv:2507.20077's recommendation is that a hallucination rate
    "can thus be interpreted only in reference to the relative recall achieved by a model" — i.e.
    **never report precision alone; always jointly with recall.** F0.5 already does this, which is a
    point in Option A's favour that PR-023 did not know it had.

- **Option B — invented-number rate with a relative tolerance** (PR-024's ad-hoc bucketing:
  of the plain numbers a segment states, the share with **nothing within 5%** anywhere in the OCR of
  its own frames).
  - Sources: PlotPick (arXiv:2605.06021) scores chart extraction by "fraction of ground-truth numeric
    values recovered … **with a 5% relative tolerance** for matching" and by RMSF1. This is published,
    independent precedent for exactly the tolerance and bucket PR-024 invented for this corpus.
  - Pros: **the least circular instrument available** — grounding or prompting cannot make a value
    that is absent from the screen appear in the reference; directly measures the thing this corpus
    exists for; offline-computable from `fidelity.json` + `ocr.json`; a rate, so length-normalised.
  - Cons: covers numbers only; **blind to the cursor-hover chronology defect**, where every number is
    genuinely on screen; inherits RapidOCR's errors (it misread `65018`/`64498` on this very corpus);
    and its denominator — numbers stated — moves with verbosity, so the denominator must be reported
    alongside the rate, not hidden inside it.

- **Option C — FaithScore-style atomic-fact decomposition with a model verifier.**
  - Sources: FaithScore (arXiv:2311.01477): sub-sentence identification → atomic-fact extraction →
    consistency verification against the image; reference-free; "highly correlates with human
    judgments of faithfulness" on LLaVA-1k and MSCOCO-Cap.
  - Pros: reaches non-numeric claims, including in principle the chronology defect; sub-sentence
    granularity, so segment-decomposable by construction.
  - Cons: needs an external verifier model — and if that verifier is the same family under test, the
    result is circular in the PR-024 sense; **never applied to charts, screenshots or text-dense
    images** (confirmed absent from the paper); adds cost to every cycle; and arXiv:2406.17115's
    warning about inconsistent LLM scoring applies to the verification step.

- **Option D — holistic LLM-as-judge scoring.** Rejected on cited evidence rather than taste:
  arXiv:2406.17115 finds LLM scoring neither reliable nor valid for this purpose.

*Disconfirming evidence sought:* I searched specifically for the failure mode that would break the
leading option — length-confounding of caption hallucination metrics — because a terser prompt is this
PR's *expected* outcome and would be the easiest way to win falsely. It exists and is documented
(arXiv:2507.20077, arXiv:2406.12663). I also searched for a validated metric for open-ended
description of **text-dense screen content** specifically and **found none**: FaithScore is natural
images, PlotPick is structured extraction, and the chart-QA prompting work serialises charts to tables
and never touches an image.

*Two search-result claims that did NOT survive verification and are therefore not used anywhere in
this PR:* a figure that "over 55% of hallucinations identified by CHAIR are unjustified when manually
inspected", and a length-alignment protocol of "truncate both captions to the first 64 tokens and
recompute CHAIR". Neither appears in the sources the search attributed them to (arXiv:2406.12663's
abstract and arXiv:2507.20077 respectively — the latter explicitly does *not* length-align). Recorded
per the cite-or-flag rule.

*Recommendation:* **Option B as primary, Option A as the joint secondary, with an explicit length
control — and Q1 escalated to Tier C before this is locked.**
- **Status: best-guess-given-constraints.** The components are cited (5% tolerance: PlotPick, proven;
  report-precision-with-recall: arXiv:2507.20077, proven; decompose rather than score holistically:
  arXiv:2406.17115, proven), but **no cited work uses this combination on this content class**, and
  the search found no validated measure for open-ended description of dense screen content at all.
  That is precisely the synthesis failure mode the procedure's cite-or-flag rule exists to catch, so
  it is labelled rather than dressed up.
- **Risks accepted if it ships as-is:** the primary measure sees numbers only, so the chronology
  defect is unmeasured until Q6 resolves; and the length control is a design of ours, not a cited
  protocol.

---

**Q2: Does removing presuppositional instructions improve accuracy, or only reduce verbosity?**

*Options considered:*

- **Option A — remove the human/cinematography presuppositions (this PR's proposal).**
  - Sources: `arXiv:2604.21911` and *Antidote* (`arXiv:2504.20468`) both establish the *problem*.
  - **Neither paper tests a prompt-only remedy.** Verified directly in both: 2604.21911 proposes
    HalluVL-DPO (Qwen2-VL-7B `AdP` 57.96% → 80.40%, at −2.08 points on adversarial recognition);
    Antidote is a three-stage synthetic-data pipeline plus DPO (LLaVA-1.5-7B F1 5.7% → 78.4%) and
    "does not compare against prompt-only baselines like rewriting or removing presuppositions."
  - **Pros:** the mechanism is proven and our prompt is a textbook instance of it — nine of ten
    numbered instructions presuppose entities absent from every frame.
  - **Cons:** the inference "therefore removing them raises accuracy" is **an extrapolation the cited
    literature does not make**, and both papers' authors chose fine-tuning instead — which is weak
    evidence that prompting alone was not sufficient for them.

- **Option B — keep the instructions and add verification/cautionary wording.** Evidence **against**,
  and it is the most useful finding of this round: *Risk-aware Selective Prompting*
  (`arXiv:2605.28123`) measures always-on verification prompting on POPE and finds it **harms easy
  inputs**: LLaVA-1.5 F1 .896 → .889 (random), .864 → .862 (popular), and helps only the hard split
  .810 → .827. The stated mechanism is attention competition — under verification prompting
  "instruction tokens receive 38–60% of attention mass in early layers", with visual attention down
  0.171 at layer 1 — and, critically, their **neutral-prompt control shows that adding non-verification
  text alone has a measurable cost** (middle-layer entropy collapse, −23% at layer 15). The authors
  "explicitly reject always-on instructions". Models: LLaVA-1.5-7B and InstructBLIP-Vicuna-7B;
  **no Qwen**.

- **Option C — shorten the instruction rather than rewrite its content.** Supported by the same
  attention-competition mechanism as Option B's counter-evidence, and by an independent line: the
  *Mechanisms of Prompt-Induced Hallucination* paper (`arXiv:2601.05201`) locates PIH in early-layer
  heads that copy prompt values before multimodal integration, and **did include Qwen2-VL and
  LLaVA-OneVision (Qwen2 backbone)** — the closest model family to ours in any source found. It tests
  no prompt interventions and offers no prompt-writing guidance, so it supplies mechanism, not remedy.

*Disconfirming evidence sought:* I searched directly for prompt-only hallucination mitigation with
measured effects. What came back argues *against* elaboration, not for it — the RSP result above, plus
the general finding that "manual prompting strategies are highly sensitive to wording and lack tunable
hyperparameters, with effectiveness varying across tasks."

*Recommendation:* **Option A and C together — remove the presuppositions *and* make the prompt
shorter — but the PR must stop claiming the accuracy benefit is research-backed.**
- **Status: best-guess-given-constraints.** The problem is proven; the remedy is not, on any cited
  source, and the two papers closest to it chose fine-tuning.
- **Why anyway:** the boilerplate cost is measured on this corpus and is real regardless of accuracy —
  3,783 words spent on the absence of humans — and the one mechanism with cited numbers (instruction
  tokens competing with visual tokens for early-layer attention) points the same way. It is a
  defensible change with an honest label, not a proven one.
- **Risks accepted:** accuracy may not move at all. Phase 6 must be able to report exactly that
  outcome without treating it as a failure to iterate away.

---

**Q3: What prompt structure most improves factual grounding for dense information displays?**

*Options considered:*

- **Option A — a detailed, domain-specific prompt** (the PR's implicit direction).
  - Source, and it is a **direct measurement against this option**: PlotPick (`arXiv:2605.06021`)
    compared a simple prompt ("Extract the data from this chart as a tab-separated table") against a
    detailed prompt specifying stacked-bar rules, axis-scale multipliers and output formatting, on
    PlotQA (n=529): **Claude Haiku 4.5 96.3% → 97.6%; Claude Sonnet 4.6 98.8% → 99.1%.** The authors
    conclude verbatim: *"This indicates that PlotPick's accuracy is driven primarily by the VLM's
    vision capability rather than prompt engineering."*
  - **They also identify what does drive it** — whether "numeric values appear directly in the image",
    and labelled data — "more so than model size or prompting strategy". That is the same conclusion
    PR-024 reached independently on this corpus: TradingView header glyphs are ~10 px, below one
    32 px visual token, and no prompt reaches them. Two independent lines converging.
  - Limits on transfer: frontier closed models, not an 8B open-weight model; a structured-extraction
    task with a tolerance metric, not open-ended description; and figures with explicit numeric labels,
    which is the opposite of our regime.

- **Option B — few-shot chain-of-thought.**
  - Source: *Evaluating Prompting Strategies for Chart Question Answering* (`arXiv:2603.22288`):
    FS-CoT 77.0% vs Few-Shot 72.3%, ZS-CoT 72.2%, Zero-Shot 69.8% over 1,200 samples.
  - **Transfers poorly, by the paper's own limitations:** charts were serialised as "row-wise
    key–value format" or CSV — it "assumes access to structured chart inputs (tables/JSON) and
    **bypasses challenges of parsing raw images**"; only GPT-3.5/4/4o, **no open-weight or Qwen
    models**; no confidence intervals or significance testing; and few-shot cost 30–300% more tokens.
  - **A specific warning for us:** exact-match *fell* under FS-CoT (57.9%) against Few-Shot (64.7%) —
    reasoning traces improved answers while degrading format adherence. A CoT-shaped prompt would make
    our output harder to parse downstream, which is the opposite of what the `format` step needs.

- **Option C — structured / schema-constrained output plus a short narrative.**
  - **No primary evidence found.** The sources asserting it are vendor engineering blogs (TRM Labs,
    LlamaIndex, Hyperscience) which `docs/CONSTRAINTS.md` explicitly rules insufficient. The PR's own
    scoping probe said the same and it still holds after a second search.

*Disconfirming evidence sought:* the PR leans toward "a domain-specific prompt will help", so I
searched for measured simple-vs-detailed comparisons. The one found (PlotPick) is a **1–3 percentage
point** effect on frontier models and its authors attribute accuracy elsewhere entirely.

*Recommendation:* **do not expect prompt structure to buy factual accuracy; treat structure as a
question about output *usability*, not accuracy.**
- **Status: proven** for the narrow claim "prompt detail contributes little to chart-extraction
  accuracy relative to vision capability" (PlotPick, direct measurement + explicit author conclusion);
  **best-guess-given-constraints** for anything about open-ended description of screen recordings,
  where no primary source was found.
- **Risks accepted:** PlotPick's models and task differ from ours in three ways at once, so the
  transfer is an argument, not a measurement. It is nonetheless the only direct evidence in either
  direction, and it points away from the PR's implicit premise.

---

**What Round 1 means for the PR, stated before Round 2 runs.** Two of the PR's three implicit
premises are weaker than drafted: the accuracy benefit of removing presuppositions is **unevidenced**
in the literature the PR itself cites, and the accuracy benefit of a richer domain-specific prompt is
**measured to be small** by the only direct comparison found, whose authors attribute chart accuracy
to legibility instead — converging with PR-024's own finding on this corpus. The **boilerplate and
verbosity** case is untouched by any of this: it is measured on this corpus, and the one cited
mechanism with numbers (instruction tokens taking 38–60% of early-layer attention mass) supports a
shorter prompt. This is Amend-shaped, not Escalate-shaped, and is carried to Phase 4 rather than acted
on here.

### Findings — Round 1 Tier C escalation (2026-08-25)

Q1 and Q3 were pre-authorised for Tier C in Phase 2; the user approved Tier C for **Q1 and Q2** after
the Tier-A round. Both ran the 5-phase harness
(`~/.claude/research-tiers/mid-research.js`). Q2: `wf_5a359a03-0dc`, 32 agents, 1.50M tokens.
Q1: `wf_0cc3be01-487`, 40 agents, 1.69M tokens across two invocations — see the coverage-guard note
below.

**The Q1 run aborted on its first invocation, and that was the harness working.** Extraction produced
18 claims from 6 sources, 14 rated central, against 10 verify slots — 71% central coverage under an
80% floor — so it refused to return findings on a majority of claims while staying silent about the
rest. Re-run with `MAX_VERIFY_CLAIMS` raised 10 → 14 (100% coverage), resumed from the same run id so
scope/search/fetch/extract replayed from cache and only verification ran live. The constant was
restored to 10 afterwards and the template diffed clean against its backup, so the shared Tier-C
harness is unchanged.

---

**Q1 (Tier C): How should factual accuracy be measured to compare two prompt variants that differ in
output length?**

*Recommendation:* **keep the existing fidelity metric's shape, change three things about how it is
used, and do the κ study.** Status per component below.

1. **Precision alone cannot decide this comparison — unanimous, including from the authors of the
   precision-only metrics.** `FaithScore`'s own Limitations section: *"it does not account for factual
   recall, meaning it doesn't penalize models for generating fewer facts. This can be seen as
   unfair… To address this, we suggest reporting FaithScore and the average length of generated
   text."* `OVFact` (Google DeepMind, arXiv:2507.19262) on CHAIR-style precision: it *"rewards short
   captions with fewer potential mistakes but few details"*, and the degenerate case is explicit —
   *"CHAIR can be artificially improved by simply not predicting any objects."* The verifier hunted
   the counter-case (that a normalised ratio is not mechanically length-biased) and found no credible
   source defending it; the counter-claim was **refuted 0-2**. This lands directly on
   `fidelity.rs`: `precision = supported/stated` has the same blind spot, and a terser prompt that
   states fewer numbers cannot be penalised by it. **Status: proven.**

2. **The metric's *shape* is already right.** The reliable construction in the literature is a
   length-normalised precision term, a recall term whose denominator comes from the **input**, an
   F-combination, and the raw fact count reported alongside. `OVFact` = |grounded|/|candidate| paired
   with recall over a per-image reference set; `CAPTURE` (arXiv:2405.19092) reports precision/recall/F1
   per element type. vid-to-text's `recall = mentioned/prominent`, where `prominent` is built from OCR
   of the frames, is exactly that construction. **Status: proven** (structure), with the qualification
   both papers carry: F removes the *incentive* to be short, neither demonstrates score
   length-invariance.

3. **The β in the F-score is load-bearing, and PR-023's inherited F0.5 systematically prefers the
   terser arm.** `fidelity.rs` uses `f05(p,r) = 1.25pr/(0.25p+r)`. Worked arithmetic against that very
   formula: an arm at **p 0.80 / r 0.40** scores **F0.5 0.6667** but **F1 0.5333**; an arm at
   **p 0.60 / r 0.60** scores **0.6000** under both. **The terse arm wins under F0.5 and loses under
   F1** — a rank reversal produced purely by β, on exactly the precision/recall trade a prompt-length
   change induces. It reproduces at 0.70/0.50 vs 0.55/0.65. Both papers that solved the length problem
   (`OVFact`, `CAPTURE`) chose **β = 1**. This is not "F0.5 is wrong"; it is that **choosing F0.5 is
   choosing to prefer brevity**, and for this PR that choice cannot be inherited silently from PR-023
   — where it was made for a *sampling* study, not a *verbosity* one. **Status: proven** (the
   arithmetic is against the repo's own function). **Amend candidate for Phase 4.**

4. **Normalise per atomic unit, never per output — and the domain-matched precedent is already cited
   by PR-023.** CHOCOLATE (Huang et al., ACL 2024 Findings), verbatim: *"Error rates are computed at
   the sentence level instead of the caption level since different models generate captions of
   different lengths. A sentence-level evaluation helps mitigate this discrepancy and facilitates a
   fairer comparison."* 5,323 sentences, 7 annotators, Fleiss κ 0.63. vid-to-text is already per-fact,
   so it satisfies this. **Status: proven**, with the paper's own hedge — "helps mitigate", not
   eliminates — and the caveat that it compares six models, not two prompt variants of one.

5. **A residual length effect survives normalisation, so length is a reported covariate, not an
   assumption.** FaithScore, itself a per-fact rate, still falls monotonically with entity count
   (InstructBLIP 0.895 at 1 object → 0.662 at 10). **Operational consequence, adopted:** report
   `stated` count and mean segment length per arm beside the score, and **treat a score difference
   accompanied by a large fact-count difference as uninterpretable rather than as a win.** No source
   separates "longer output genuinely contains more errors" from "the metric penalises length" — that
   unresolved ambiguity is exactly what makes this a confound. **Status: proven** (the effect);
   **convention** (the reporting remedy).

6. **Compare paired within-item, not pooled — and this is what supplies the measurement error the
   Phase 2 stopping rule needs.** `CAPTURE` §5.1.1 verbatim: per-sample rank agreement *"is regarded
   as the most important metric for consistency evaluation"*. **With exactly two variants, per-sample
   Kendall's τ collapses to ±1, so it degenerates to a paired sign test over videos** — which is the
   correct instrument and is what Phase 2 already fixed. But `score_segments` currently
   **micro-averages** `stated`/`supported` across all segments into one pooled number, so a long or
   segment-dense video dominates and one number per arm yields no variance estimate.
   **Status: proven** (the pooling behaviour is in the code); **convention** (extrapolating CAPTURE's
   metric-validation protocol to prompt A/B). **Amend candidate for Phase 4.**

7. **The recall denominator is prompt-invariant; the precision denominator is not — verified on this
   corpus today, not merely inferred.** The harness inferred this from `fidelity.rs` and flagged it as
   checkable in one step. Checked, on three `clip900` jobs under identical sampling:

   | job | `prominent` per segment (recall denominator) | `stated` per segment (precision denominator) |
   |---|---|---|
   | `735042d7` | 68, 64, 81, 99, 85, 88, 85, 67, 103 | 7, 15, 15, **44**, 24, 27, 16, 22, 47 |
   | `402327e8` | 68, 64, 81, 99, 85, 88, 85, 67, 103 | 7, 15, 15, **567**, 24, 27, 16, 22, 47 |

   **The recall denominator is bit-identical while the precision denominator moves 13×** on the
   degenerate segment. Combined with the segment-span and frame-list identity already verified in
   Phase 2, both arms are scored against literally the same reference fact set. **Status: proven on
   this corpus.**

   **A caveat the check itself surfaced, which no source would have given us.** A third job,
   `cc423d58`, has *slightly different* prominent counts (67, 64, 80, 99, 85, 87, 87, 67, 104). It
   predates PR-023's tokenizer and matching fixes. So the invariance holds **only when the fidelity
   code and config are held identical across arms** — meaning Phase 6 must either freeze `fidelity.rs`
   for the duration or re-score every arm with one binary. That is now a hard operating rule for
   Phase 6, found by running the check rather than by reading.

   *Not* a licence to rank on recall alone: that has the mirror-image failure of precision-only and
   rewards indiscriminate enumeration — which is precisely what PR-025's numeric guard exists to catch.

8. **κ is the field's standard precondition, not local pedantry — and it bounds what Phase 6 can
   resolve.** Every metric in the set ships a human meta-evaluation as its central claim, and the
   ceilings are sobering: FaithScore, best of its tested field, reaches only **Pearson r 0.482**
   (BLEU-4, ROUGE-L and METEOR are *negatively* correlated); CAPTURE's best Sample τ is **0.6018**;
   CHOCOLATE reaches **Fleiss κ 0.63** with seven trained annotators; ALOHa's absolute localisation
   accuracy is **20.30%**. **An effect smaller than that noise is not resolvable by the instrument.**
   This independently vindicates `docs/ARCHITECTURE.md`'s rule and DESIGN-log decision 4 —
   *"nothing ships without the κ"* — as the field's standard, and it supplies what PR-023 lacked: a
   **domain-matched annotation template** (CHOCOLATE's per-sentence labels under a fixed error
   typology) and a realistic target (**κ ≈ 0.6 is success, not disappointment**). `cohen_kappa` already
   exists at `vtt-client/src/review.rs:127`; only labels are missing. **Status: proven.**

9. **The alternatives are worse for this job.** CLIP-based scores cannot ingest the output at all —
   CAPTURE's authors had to *"truncate the detail caption paragraph… due to the limitation in input
   length"*; CIDEr collapses to Sample τ 0.0991; **ALOHa aggregates as a `min` over objects with no
   length normalisation**, and min is monotonically non-increasing under set inclusion, so adding a
   detail can only lower the score — a longer arm is structurally penalised. The LC-AlpacaEval
   length-controlled regression is real but needs **pairwise preference judgments against a common
   baseline** and cannot length-correct an absolute factuality score. **Status: proven.**

10. **No metric in this literature is validated on text-dense screen content; the structure transfers,
    the tooling does not.** CAPTURE parses with a T5 trained on Visual Genome region descriptions and
    has **no element type for numbers or on-screen text**; OVFact grounds with OWL-ViTv2/OpenSeg over a
    2,792-concept *object* vocabulary; ALOHa grounds in DETR detections. None covers charts,
    dashboards, screen recordings or video. The only domain match is CHOCOLATE, whose sentence-level
    rates come from **paid human annotation**, not an automatic metric. vid-to-text's
    `Fact::{Number, Label, Timeframe}` extraction with a tolerance rule is the right substitution in
    principle and **has no published counterpart to validate against** — which *raises* the importance
    of the local κ study rather than excusing it. **Status: best-guess-given-constraints**, and this is
    the honest answer to Phase 2's Q1(e).

*Source-integrity problems the verification caught, recorded because they are the failure mode this
procedure exists to prevent:* one supporting quote in the fetched set **spliced a CHAIR sentence onto
an ALOHa sentence**, and a search summariser **fabricated an "ALOHa favors shorter captions" quote
that does not exist in the paper**. The min-aggregation argument in finding 9 is the real, verified
basis for that conclusion.

*Correction to the Tier-A round above.* I recorded the figure "over 55% of hallucinations identified
by CHAIR are unjustified when manually inspected" as **failing verification**, because it is absent
from arXiv:2406.12663's abstract. It is **in the paper's full text** and is now verified: Feng et al.
measured it on **500 MSCOCO LLaVA-1.5 captions**, with the effect *"exacerbated in detailed
captions"*. The claim stands and strengthens finding 5; my Tier-A note was a limitation of reading
only the abstract.

---

**Q2 (Tier C): Does removing presuppositional content improve factual accuracy, or only reduce
length?** — **this overturns the Tier-A conclusion in the PR's favour.**

The Tier-A round concluded the remedy was "unevidenced" because the two papers it found both mitigate
by fine-tuning. That under-read the literature.

1. **The add-side effect is large, measured on the Qwen family, and peer-reviewed.**
   `arXiv:2601.05201` is an **ACL 2026 Long Paper** (DOI `10.18653/v1/2026.acl-long.1941`), not a
   preprint. CountBench, 491 pairs expanded to 3,437 image-prompt pairs, three VLMs:

   | prompt | metric | LLaVA-OV-7B | **Qwen2-VL-7B** | Janus-Pro-7B |
   |---|---|---|---|---|
   | `"How many [objects] are there in the image?"` | Exact Match | 76.89 | **78.49** | 80.32 |
   | `"Describe the N+k [objects] in the image"` | True Count Match | 45.68 | **37.70** | 30.54 |

   A **31.2 / 40.8 / 49.8 pp** drop. **Status: proven.**

2. **The effect is not a length artifact — established by construction, which is exactly what this PR
   needed.** The verifier grepped the full text for `length|token length|output length|verbos` and got
   **zero hits**; all three metrics are numeric-value matches (*"Prompt and true-count matches do not
   sum to 100%, as some responses contain no numerical value"*), so verbosity cannot mechanically move
   them. Given Q1's finding that length confounds nearly every factuality metric, a presupposition
   result on a length-immune metric is unusually valuable. **Status: proven.**

3. **The loss is causally attributable to presupposition-following, not to general capability
   damage.** Ablating the identified attention heads collapses Prompt Match 42.58 → 1.42 / 56.51 →
   3.22 / 64.10 → 10.19 and raises True Count Match to **77.80 / 70.66 / 70.90**, while *baseline*
   Exact Match is preserved or improved (76.89 → 81.24 / 78.49 → 79.29 / 80.32 → 79.41). A random-head
   control does almost nothing. A specific circuit converts instruction-borne presupposition into wrong
   answers, and suppressing it recovers accuracy with no capability trade-off. **This is the
   mechanistic warrant for expecting prompt-side removal to recover accuracy** — while remaining a
   model-side intervention, not a prompt remedy. **Status: proven.**

4. **No source runs the ablation this PR actually performs.** Established by explicit full-text greps
   in three separate verifications. Every measured effect is add-side; every mitigation is model-side
   (HalluVL-DPO fine-tuning; attention-head ablation; Ghost-100 has no prompt-editing arm). 2601.05201
   never tests `k=0` in its accuracy table. So **"removing presupposition improves accuracy" is a
   well-warranted inference from add-side evidence, not a directly measured result.**
   **Status: best-guess-given-constraints — and this is the honest label for this PR's central claim.**

5. **A confound that constrains how the new prompt should be written.** In all four studies the
   neutral condition is a **short forced-choice** item while the presuppositional condition is
   **open-ended**, so presupposition is never isolated from question format, and no source reports
   token counts across conditions. Corroboration is directionally consistent but not uniform:
   Ghost-100 (`arXiv:2604.18803`) raises rule-based H-Rate 23.12% → 41.75% on Qwen2.5-VL-7B as
   directive force escalates, but **Gemma-4-E4B-IT peaks then falls *below* its baseline, InternVL2.5-8B
   is flat, and Llama-3.2-Vision-11B drops** — the effect is not universal across models.
   **Status: convention** (multi-benchmark direction), with non-uniformity recorded.

6. **The correction to this PR's Motivation, independently reproduced.** The harness's verifier
   reached the same conclusion I did by a different route, and sharpened it: HalluScope's defensible
   same-object figure is a **15.2–57.0 pp** drop (Adversarial Presupposition vs Adversarial
   Recognition, computed per-row across five LVLMs), and the paper's own looser prose figure is
   *"against DIFFERENT objects (Rec_pos/Rec_rnd)"*. The PR inherited that looser framing. Phase 4 must
   replace the sentence with the same-object figure.

*Net effect on the PR.* Q2 moves this PR's central premise from "unevidenced extrapolation" to
"well-warranted inference from a peer-reviewed, length-immune, Qwen-family result whose causal
pathway is isolated" — while still not being a directly measured remedy. That is a materially stronger
position than the Tier-A round reported, and it is why the escalation was worth running.

### Findings — Round 2 (Tier A probes, 2026-08-25)

**Q4: Does explicitly describing an interface's conventions reduce false inference about it?**

*Options considered:*

- **Option A — supply per-element semantics as text alongside the image** (the strongest measured
  precedent, and already cited by this repo).
  - Source: **OmniParser** (`arXiv:2408.00203`), which `docs/0.0/DESIGN-log.md` already cites for
    screenshot OCR. Adding functional descriptions of detected elements to the prompt moves GPT-4V on
    the SeeAssign task from **70.5% → 93.8%**, and the gain is largest where the screen is densest:
    **62.0% → 90.0%** on the >40-box tier. ScreenSpot **58.38% → 73.0%**.
  - **But it does not answer the question asked.** Verified directly: the paper supplies *icon-specific
    functional descriptions* ("a delete button"), and **convention descriptions — what a region of the
    interface *means* — are not tested.** The task is also action grounding ("which element do I
    click"), not description.
  - **And the analogous intervention has already been measured null on this corpus.** OmniParser's
    mechanism is "tell the model, in text, what the screen elements are." That is structurally what
    **PR-024's OCR grounding** did — and pooled over two videos it produced **no measurable accuracy
    effect** (invented values 2.59% ungrounded vs 2.72% grounded) at **+34% wall clock**. Local
    disconfirming evidence beats a transfer argument.
  - One OmniParser limitation is worth carrying because it is *our* failure in miniature:
    **"Context-Blind Icon Recognition"** — descriptions generated without full context misread a
    three-dot menu as "a loading or buffering indicator". A region's meaning was misread from its
    appearance. That is structurally the cursor-hover defect.

- **Option B — describe the interface convention itself** (that the header shows the hovered candle;
  that the right edge is unfilled future space). **No primary source found, in either direction.**

*Disconfirming evidence sought:* I looked for the measured case *for* adding interface semantics,
found the strongest one (OmniParser), and then checked whether it covers conventions rather than
element labels — it does not — and whether this project has already run the closest analogue — it has,
with a null result.

*Recommendation:* **treat "describe the interface's conventions" as a Phase 6 empirical arm, not a
research-backed choice.** This is exactly the outcome Phase 2's success criteria admitted as valid.
- **Status: best-guess-given-constraints.**
- **Risks accepted:** it adds prompt tokens, and Q2's Tier C round found that instruction tokens
  compete with visual tokens for early-layer attention. A convention paragraph is not free, and the
  arm must be measured against a shorter prompt, not only against the current one.

---

**Q5: How should observation be separated from inference in the output?**

*The question this PR actually has to answer is why **asking** is insufficient*, because the shipped
prompt already instructs it — *"Clearly separate observation from inference. State what you see, then
state what you think it means and why"* — and the `2024_4_8` fabrication happened anyway **and was
narrated as observation**, not as inference.

*Options considered:*

- **Option A — instruct harder / add epistemic-marking rules.** Evidence against, on the part the
  sources agree about: *Overconfidence is Key: Verbalized Uncertainty Evaluation in LLMs and VLMs*
  (`arXiv:2405.02917`, TrustNLP @ NAACL 2024) finds LLMs and VLMs are systematically overconfident and
  that **chain-of-thought and verbalized-confidence prompting do not remove it**; the authors
  recommend practitioners not rely on verbalized confidence as a calibrated estimate.
  *Recorded honestly:* two readings of this paper conflicted on a secondary point — whether
  verbalized uncertainty retains *any* residual value as a mistake signal. The PDF extracted poorly
  and I could not settle it. **My conclusion rests only on the uncontested part** (prompting does not
  remove overconfidence), not on the disputed part.
- **Option B — structural separation in the output** (distinct observation and inference fields rather
  than a requested writing style). No primary evidence for open-ended description; Q3 already found
  the structured-output literature is vendor blogs. Its merit is *legibility for a downstream reader*,
  which is what the PR's question actually asks for, not accuracy.
- **Option C — accept that this class of error cannot be marked, and detect it instead.** See Q6.

*Recommendation:* **Option B for legibility, and do not expect it to buy accuracy; the accuracy path
is Q6's detector.**
- **Why asking fails, grounded rather than asserted:** this is not a compliance failure. Q2's Tier C
  round established that presupposition-following is a *specific attention circuit* that converts an
  instruction's frame into **wrong answers**, with head ablation restoring accuracy while leaving
  baseline capability intact. The model does not know it is inferring — it commits. A hedging
  instruction cannot mark a claim the model believes it observed, and the calibration literature says
  the model's own confidence signal is not calibrated either.
- **Status: convention** (structural separation aids a downstream reader); **proven** (that prompting
  does not fix overconfidence); **best-guess-given-constraints** (that Option B costs no accuracy —
  untested, and it lengthens the prompt).

---

**Q6: Can the cursor-hover chronology error be detected mechanically?**

*The literature answer: the defect class is named and benchmarked, but every automatic detector found
needs something this corpus does not have.*

- **VIDHALLUC** defines **temporal sequence hallucination** and scores it automatically — but only
  because it is a *constructed multiple-choice task* (concatenate two clips, ask which order). Models
  score below 50% (VideoLLaMA2 37.17%, Video-ChatGPT 30.17%) against **90.17% human**. Its mitigation,
  DINO-HEAL, is inference-time feature reweighting, **not prompt-based**.
- **ARGUS** (`arXiv:2506.07371`) penalises out-of-order sentences with an explicit ordering penalty
  (λ=0.1) and reaches **91.26% agreement** with 26 human raters — but it **requires human reference
  captions** (averaging 477 words per video) and uses GPT-4o as judge. Not reference-free, and
  unaffordable per cycle.
- Also relevant, and the *opposite* of our defect: VIDHALLUC reports that "in nearly 50% incorrect
  cases, models perceive only a single action throughout the entire video, failing to detect multiple
  actions or transitions" — the benchmarks measure **missed** change; ours **invents** change.

*So a general chronology detector cannot be lifted from the literature.* **But our error is not the
general problem** — it has a known mechanism (the model reads TradingView's hovered-candle header and
narrates it as a time series), and that mechanism is spatially localised.

*Feasibility, verified against real persisted data rather than assumed.* `ocr.json` stores per frame
a list of items with `text`, `score`, **`x`, `y`, `height_px`**. On a mid-video frame of job
`735042d7` the entire chart header is captured as the **single topmost OCR item**:

```
y=135  x=26  h=26  'Crypto Total Market Cap, $· 1D· CRYPTOCAP 01.521T H1.557T L1.495T C1.528T +7.071B (+0.46%)'
```

The next item is 25 px below at the far right (the price axis at x≈1855). So the header is
**positionally isolable** (topmost item, small height, left-anchored) and **parseable** — the OHLC
values carry `O`/`H`/`L`/`C` prefixes in a fixed order, and the price-axis values that a *legitimate*
movement claim would draw on live in a different region (x≈1850) entirely.

*Recommendation:* **a targeted detector is buildable from artifacts already persisted, and this PR
should build it rather than record that it cannot be built.** Sketch, to be specified in Phase 4:
1. Per frame, locate the header item and parse its `O/H/L/C` values into a **header-only value set**.
2. In a segment's text, find change-claims — two or more numbers joined by movement language
   ("moved higher", "now at", "→").
3. **Flag a claim whose values are drawn from header readings across different frames**, since the
   header reflects the *hovered* candle rather than the passage of time, so a cross-frame header
   sequence is not a chronology.
- **Status: proven** that the required data exists and the header is isolable (verified on this
  corpus today); **best-guess-given-constraints** for the detector's false-positive rate, which is
  unmeasured until it is built.
- **Two hazards recorded now.** The OCR reads the header's `O` prefix as a zero — `01.521T` above is
  really `O1.521T`, the exact misread PR-023 already documented — so the parser must tolerate `O`/`0`
  confusion. And the single-item capture is a layout property of these frames, not a guarantee; the
  detector must degrade to "no header found" rather than mis-parse.
- **Why this matters more than it looks:** Q1's primary measure counts numbers, and every number in
  the cursor-hover fabrication is genuinely on screen — the metric scores that segment **precision
  0.883**. Without this detector, Phase 6 cannot see the defect the PR calls its most damaging, and
  the stopping rule would be driven entirely by an instrument blind to it.

---

**Round 2 in one line.** Q4 has no evidence and becomes an empirical arm; Q5's "just ask" is
foreclosed by the same circuit-level result that supports Q2, so separation is a legibility feature
rather than an accuracy fix; and Q6 — the one that decides whether Phase 6 can see its own target —
is **buildable**, verified against the persisted OCR rather than argued.

### Group D: MCP Verification — WAIVED

**Not run.** The user directed skipping it and proceeding to Phase 4 (2026-08-25). Recorded rather
than quietly omitted, because `PROCEDURE-pr-research.md` makes Group D mandatory before Phase 4 is
locked, and because two of this round's recommendations are exactly the kind Group D exists to check:
the Q6 detector combines an OCR-item schema (`text`/`score`/`x`/`y`/`height_px`), a header-parse rule
and a claim-extraction rule that **no cited source uses in combination**, and the Q1 recommendation
combines a 5%-tolerance number rule, an input-derived recall denominator and a paired sign test that
likewise appear together in no single source.

**Partial substitute, done during the rounds rather than after them:** the two load-bearing
*identifier-level* claims were verified live against this system rather than against documentation —
the recall denominator's prompt-invariance (`prominent` bit-identical across arms while `stated`
moved 13x) and the OCR schema plus header isolability (`y=135, x=26, h=26`, header captured as the
single topmost item). The **synthesis-level** risk that Group D targets — that a recommended
*combination* has no cited working example — is therefore **not** discharged, and both combinations
above stay labelled `best-guess-given-constraints`.

---

### Synthesis

**Outcome: AMEND.** The research does not invalidate the PR's premise — the prompt is genuinely
content-mismatched, the boilerplate is measured, and the provenance gap is real — but it changes
enough load-bearing specifics that `PROCEDURE-pr-research.md`'s Outcome Branch requires stopping and
putting the amendments to the user before any synthesis step runs. Nine amendments follow, grouped by
whether they need a decision.

**Corrections that need no decision (factual; applied when synthesis resumes):**

- **A1 — the Motivation misstates its cornerstone source.** "Object recognition falls from 94.1% to
  56.7%" compares two different metrics on two different subsets. Replace with the defensible
  same-object figure (Adversarial Presupposition vs Adversarial Recognition, **15.2–57.0 pp** across
  five LVLMs) and/or the stronger ACL 2026 result (`arXiv:2601.05201`: Qwen2-VL-7B **78.49 → 37.70**,
  a 40.8 pp drop, on a metric verified length-immune).
- **A2 — the Scope's "or an explicit finding that one cannot be built" is now resolved.** The
  chronology detector **is** buildable from persisted artifacts; the alternative branch can be struck.
- **A3 — Research backing is upgraded.** The PR calls its own probe "secondary or general-purpose".
  It now rests on a peer-reviewed ACL 2026 paper covering the Qwen family, with the causal pathway
  isolated by head ablation.

**Amendments that need a decision:**

- **A4 — F0.5 vs F1 (β).** PR-023 chose F0.5 for a *sampling* study. This is a *verbosity* study, and
  F0.5 pre-favours the terser arm: p 0.80/r 0.40 → F0.5 **0.6667** vs F1 **0.5333**, against
  p 0.60/r 0.60 → **0.6000** under both. Both metric papers that solved the length problem chose β=1.
- **A5 — the κ study.** The literature treats human-agreement validation as the precondition for using
  a metric to rank anything, which independently vindicates `docs/ARCHITECTURE.md`'s existing rule.
  Realistic ceiling κ ≈ 0.6; CHOCOLATE supplies a domain-matched template.
- **A6 — per-video paired scoring.** `score_segments` micro-averages into one pooled number and yields
  no variance estimate, so Phase 2's stopping rule has nothing to read.
- **A7 — the primary measure**, now answerable: recall-anchored (prompt-invariant denominator) plus
  the invented-number rate at 5% tolerance, with `stated` count and segment length reported as
  covariates, and a score difference accompanied by a large fact-count difference treated as
  uninterpretable rather than as a win.
- **A8 — scope.** The PR has grown to six deliverables and now strains **One PR, One Thing**.
- **A9 — an honest expectation.** Q3's only direct measurement attributes chart accuracy to
  *legibility*, not prompting (PlotPick: +1.3 / +0.3 pp from a detailed prompt), converging with
  PR-024's finding that ~10 px header glyphs sit below one 32 px visual token. The boilerplate goal is
  untouched and well-supported; the **number-accuracy** goal may be capped by resolution regardless of
  the prompt. Phase 6 must be allowed to report that.

**User decisions (2026-08-25): all recommendations approved** — β → F1 for Phase 6, run the κ study,
add per-video paired scoring, lock the primary measure as recommended, split the PR, and permit
Phase 6 to report a null accuracy result. Synthesis steps 1–4 then ran:

**Changes to this PR** from research:
- Motivation corrected (A1) and re-anchored on `arXiv:2601.05201` (ACL 2026): Qwen2-VL-7B
  **78.49 → 37.70**, on metrics verified length-immune, with the causal pathway isolated by
  attention-head ablation. The old 94.1%/56.7% sentence is replaced and the error recorded in place.
- **Split (A8).** Prompt provenance, prompt deployment, β, paired scoring, the κ study and the
  chronology detector moved to **PR-027**, which this PR now depends on. This PR keeps the prompt and
  Phase 6.
- **Primary measure locked (A7)** — invented-number rate at 5% tolerance as primary; precision **and**
  recall at **β = 1** (A4) as the joint secondary, never read alone; chronology reported separately
  because the primary measure is blind to it; `stated` count, segment length and boilerplate rate as
  always-reported covariates.
- Candidate directions re-graded by evidence: presupposition removal is the best-supported;
  *shorter* is added as a second lever on an independent mechanism; *interface conventions* is
  demoted to an empirical arm with no evidence and a local null result against it; *observation/
  inference separation* is demoted from an accuracy fix to a legibility feature.
- Verification criteria rewritten to test the things the research says can go wrong — one fixed
  tokenizer for the boilerplate rate, at most two held-out looks, covariates reported per cycle, a
  frozen metric version across arms, and an explicit "current prompt retained" outcome (A9).
- Research backing upgraded from "sources are secondary or general-purpose" to a peer-reviewed
  result on this model family, with the remedy step still labelled `best-guess-given-constraints`.

**Changes to `docs/ARCHITECTURE.md`:** a new **Comparing two arms** paragraph in § Fidelity
Diagnostic recording segment-alignment and its fixed-metric-version caveat, that β is not neutral
(with the rank-reversal arithmetic), and that precision may not be read without recall. Committed
alongside this PR per Documentation Accuracy.

**Changes to `docs/CONSTRAINTS.md`:** none. The κ rule that gated this work already exists in
`docs/ARCHITECTURE.md` and the research independently vindicated it as the field's standard
precondition rather than a local convention — so it is confirmed, not amended.

**New PRs that must come first:** **PR-027 — Vision measurement readiness.** Added to
`docs/0.0/ROADMAP.md` with an order note (it lands before PR-026 despite the higher number) and to
`docs/0.0/RESEARCH-BACKLOG.md` with its own Phase 1 still owed.

**Research-backed details now locked in this PR:**
- The primary measure, its secondary, its covariates, and its resolution floor.
- β = 1 for Phase 6 (F0.5 retained as the repo default for sampling work).
- Paired segment-level bootstrap as the error estimate, valid because arms are segment-aligned —
  verified on this corpus, not assumed.
- Tuning set `2024_4_15`, `2024_9_2`, `2025_5_12`, `2025_10_13`; held-out `2024_5_13`, `2025_09_22`,
  `2025_10_21`, `2025_11_17`; 720p generalisation check `2023_11_20`; 6-cycle cap; ≈3.2 GPU-hours.
- That the accuracy goal may be unreachable by prompting alone, and that reporting so is a result.

---

### Gate Check (2026-08-25)

- **Premise still valid: ✓.** The outcome was Amend, not Escalate. The prompt is content-mismatched
  (measured on this corpus), the presupposition mechanism is proven on this model family, and the
  provenance gap is real. Nothing loops back to `PROCEDURE-design-planning.md`.
- **Prerequisite PR surfaced: ✗ — and it blocks.** **PR-027 (Vision measurement readiness)** was
  created by the Phase 4 Amend and added to `docs/0.0/ROADMAP.md`. Per Phase 5 step 2, it is
  implemented first.
- **User approved the updated spec: ✓ (2026-08-25)** — all six amendments (β → F1 for Phase 6, run the
  κ study, per-video paired scoring, primary measure locked, split the PR, permit a null accuracy
  result).
- **Implementation: CLEARED (2026-08-25)**, against the collapsed scope above.
- **Scope collapsed after the Gate Check, by user decision.** The gate first concluded that PR-027
  had to land first, because Phase 6's stopping rule read an estimator that did not exist. The user
  judged the measurement programme disproportionate to a prompt change and directed a collapse; PR-027
  was deleted and Phase 6's statistical apparatus dropped with it. **The research supports the smaller
  scope** — it found the accuracy goal that justified the apparatus is largely unreachable by
  prompting — so this is a scope decision the findings back, not one they contradict.
- **What the collapse costs is recorded in Scope**, per item, rather than being dropped silently. The
  sharpest loss is that the cursor-hover fabrication is checked by reading instead of measured.
- **The PR-015 hazard is handled differently, not ignored.** This PR no longer claims to have *tuned*
  the prompt by measurement; it claims to have fixed a category error and checked that nothing
  regressed. That is a weaker and more honest claim than the one the Phase 6 machinery was built to
  support.

**Procedure deviations in this round, recorded rather than omitted:**
- **Group D waived** by user direction. Consequence carried into PR-027: the chronology detector and
  the primary measure are combinations no cited source uses together, and both stay labelled
  `best-guess-given-constraints`.
- **Tier C escalation** for Q1 and Q2, approved after the Tier-A round rather than pre-committed in
  Phase 2. Q1's first harness run aborted on its own coverage guard (71% central-claim coverage
  against an 80% floor) and was resumed with the verify budget raised 10 → 14; the shared Tier-C
  template was restored afterwards and diffed clean.


### Implementation Validation (2026-08-25)

**Built.** Prompt provenance (`CaptureInfo.vision_prompt`, `.vision_prompt_sha256`, SHA-256 of the
template `OllamaClient` actually loads); `config/deploy-prompts.sh`; `prompts/vision-chart.txt`
selected by `market-research`, with `exp-prompt-v0` as a version-controlled baseline arm;
`tools/prompt_ab.py`. **233 tests pass** (was 228).

**Provenance verified end-to-end**, not only by unit test: job `95f4bc52` recorded
`prompts/vision.txt` / `c0fe5d3687a0fccd…`, which equals `sha256sum` on the GPU host and the sum
`deploy-prompts.sh` printed. The three values agree, which is the property the feature exists for.

**A/B: two 15-minute excerpts x two prompts, identical sampling, one frozen `fidelity.rs`.**

| clip | arm | segs | words/seg | boilerplate | stated | precision | recall | wall |
|---|---|---|---|---|---|---|---|---|
| `2024_4_8` | old | 8 | 639 | 100% | 111 | 0.883 | 0.194 | 360 s |
| `2024_4_8` | **chart** | 8 | **264** | **37.5%** | 159 | 0.893 | 0.174 | 460 s |
| `2024_6_24` | old | 10 | 652 | 100% | 240 | 0.950 | 0.296 | 361 s |
| `2024_6_24` | **chart** | 10 | **308** | **60%** | 218 | 0.885 | 0.203 | 360 s |

Pooled: **646 → 288 words/segment (−55%)**, boilerplate **100% → 50%**, stated facts **351 → 377
(up)**, precision 0.929 → 0.889, recall 0.241 → 0.187.

**Boilerplate, per pattern** (regexes fixed in `tools/prompt_ab.py` before the first run):

| pattern | old | chart |
|---|---|---|
| absent humans | 77.8% | **0.0%** |
| no scene transitions / camera | 50.0% | **0.0%** |
| no expressions / body language | 27.8% | **0.0%** |
| "In summary…" filler | 33.3% | **0.0%** |
| nothing changed / remains unchanged | 83.3% | 50.0% |

Four of five categories are eliminated outright. The residual 50% is entirely "nothing changed",
which is **not** the same defect: restating that the screen is static is a reasonable thing to say
about a static screen, and the new prompt explicitly permits saying it once, briefly.

**The cursor-hover fabrication is addressed — checked by reading, on the video where it was found.**
Movement claims across `2024_4_8` fall from 13 to 1, and header/cursor-aware phrasing rises from 17
to 65. The model now states the convention itself:

> "The header displays the open, high, low, and close values **for the candle under the cursor**,
> along with a change figure."
> "The presenter appears to be hovering over the chart, with the cursor position changing across
> frames, **causing the header values to update to reflect the candle under the cursor**."

The single remaining movement sentence draws on the correct source: *"The last-price label is now at
42,608."* The old arm's corresponding segment instead narrated indicator panes and closed with an
"In summary" paragraph.

**What did NOT improve, stated plainly:**
- **Recall fell 0.241 → 0.187**, a 22% relative drop, on both clips. The recall denominator is
  prompt-invariant, so this is real: the terser prompt mentions **fewer of the prominent on-screen
  facts**. That is the sharpest cost of the change and it is a genuine trade-off, not noise.
- **Precision moved in opposite directions by clip** — 0.883 → 0.893 on one, 0.950 → 0.885 on the
  other. **No accuracy claim is made in either direction**, which is exactly what the research
  predicted at this effect size and sample.
- Wall clock is unchanged in aggregate (360/361 s on one clip; 360 → 460 s on the other).

**A reproducibility signal worth recording:** the old arm's precision on `2024_4_8` is **0.883**,
matching PR-023's independently recorded 0.883 for that video to three decimals — so that figure was
real and the pipeline is stable across a month and a rebuild.

**Prompt iterations after the first A/B (2026-08-25), driven by the audio transcripts.** By user
direction, the Whisper transcripts were mined as *design-time* evidence for what these sessions are
about. `use_transcript` stays `false` — vision never sees audio at capture time — so this changes what
the prompt says, not what the model is given.

| version | sha | change | `2024_4_8` w/seg · prec · recall |
|---|---|---|---|
| v1 | `923b869a` | drop human/cinematography presuppositions, state interface conventions | 263 · 0.893 · 0.174 |
| v2 | `a4a133fc` | + conditional feature list from 4.4k words of transcript | 278 · 0.926 · 0.235 |
| v3 | `c0921846` | reweighted from 27k words across 8 videos | 543 · **0.529** · 0.330 |
| **v3.1** | `cfab896e` | v3 with the enumeration trigger removed | **200** · 0.890 · **0.265** |

**Why v1 lost recall, and how the transcripts found it.** v1's terseness discarded exactly what the
analysts discuss: segments mentioning `support` fell 10/18 → 2/18, `resistance` 11/18 → 2/18,
`momentum` 2/18 → 0/18 despite being the most-spoken indicator (11x). v2 added a *conditional* feature
list — conditional because a checklist is itself a presupposition risk, the failure this PR exists to
fix — and recovered recall on all three clips (.194→.235, .296→.320, .204→.286), including the
held-out clip, with movement claims at zero on all three.

**v3 regressed and the cause was mine.** Reweighting by a 27k-word vocabulary profile was sound, but
the prompt handed the model a canned sentence template (`Say "a line is drawn at 71,700"`) for an
enumerable feature. It filled the slot and ramped into negative prices. One segment stated 284 facts
with 239 unsupported; excluding it the run scores 0.915. **v3.1 removes the template** — lines are
listed as a set, gridline enumeration is forbidden, and an explicit rule names the ramp — and the
worst segment falls to 64 stated / 3 unsupported.

**That defect is now PR-028**, because neither shipped guard can see it: `truncate_numeric_run` counts
a longest consecutive run of 2, and `truncate_repetition` sees only unique sentences.

**Shipping decision is OPEN.** `prompts/vision-chart.txt` currently holds **v3.1**, validated on one
clip. **v2 is the only version measured on all three**, including held-out. v3.1 is terser (200 vs 278
w/seg) with higher recall (0.265 vs 0.235) and slightly lower precision (0.890 vs 0.926) on the clip
both saw. Completing v3.1 on `2024_6_24` and `2025_05_26` is the outstanding task; if it does not hold,
revert to v2 (`a4a133fc`) which is recorded and reproducible.

**Verdict against the PR's own bar:** boilerplate is measurably down as a rate, the worst known
failure is addressed and verified by reading, nothing collapsed on the guardrail, and no accuracy
claim is made. Recall is the open cost and is recorded rather than smoothed over.

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
The strongest evidence is peer-reviewed and covers the family this pipeline runs. *Mechanisms of
Prompt-Induced Hallucination in Vision-Language Models* (arXiv:2601.05201, **ACL 2026 Long Paper**,
DOI 10.18653/v1/2026.acl-long.1941) swaps a neutral instruction for one presupposing something absent
and measures the cost on CountBench across three VLMs:

| prompt | metric | LLaVA-OV-7B | **Qwen2-VL-7B** | Janus-Pro-7B |
|---|---|---|---|---|
| `"How many [objects] are there in the image?"` | Exact Match | 76.89 | **78.49** | 80.32 |
| `"Describe the N+k [objects] in the image"` | True Count Match | 45.68 | **37.70** | 30.54 |

A **31–50 pp** drop. Two properties make it unusually load-bearing here. The metrics are
numeric-value matches, so **output length cannot mechanically move them** — which matters because
almost every other factuality metric is length-confounded (see Research findings, Q1). And ablating
the specific attention heads that follow the presupposition restores True Count Match to
**77.80 / 70.66 / 70.90** while leaving *baseline* Exact Match intact, isolating
presupposition-following as the causal pathway rather than a general capability loss.

*When Prompts Override Vision* (arXiv:2604.21911) corroborates on its HalluScope benchmark: a
**15.2–57.0 pp** gap between Adversarial Presupposition and same-object Adversarial Recognition
across five LVLMs, with the stated mechanism *"reliance on presuppositions introduced by the
instruction itself"*, even where visual recognition is accurate. *(An earlier draft of this section
cited that paper's 94.1% and 56.7% figures as a before/after on object recognition. They are two
different metrics on two different subsets — `Rec_pos` on present objects and `AdP`, the rate of
correctly rejecting a false presupposition. Corrected 2026-08-25 during Phase 3.)*

Our prompt instructs the model to track characters, read facial expressions and identify speakers on
charts that contain none of them.

**The remedy direction, stated honestly:** every study above measures the harm of *adding* a
presupposition, and every mitigation they propose is model-side (preference optimisation, attention-head
ablation). **No source runs the removal ablation this PR performs** — confirmed by full-text greps in
three independent verifications. Removing the presuppositions is a *well-warranted inference* from
add-side evidence with an isolated causal pathway, not a directly measured remedy.

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

**Collapsed 2026-08-25, after the research round and by user decision.** The round briefly grew this
into two PRs and a measurement programme — a κ calibration study, a paired bootstrap, a configurable
F-score β, a chronology detector. That was disproportionate to the task, and **the research itself is
the argument against it**: prompt detail buys 1–3 pp of chart accuracy, and accuracy tracks
*legibility* (~10 px header glyphs under one 32 px visual token), not prompting. The accuracy
ambition is what required the apparatus, and the accuracy ambition was probably unreachable by prompt
alone. PR-027 was deleted and the scope is now one PR.

**In scope:**

1. **Prompt provenance** — a prompt identifier and content hash in `CaptureInfo`, so a timeline says
   which prompt made it. Must be `Option`/`#[serde(default)]` so existing timelines still load. Kept
   despite the collapse because without it the corpus becomes unattributable, which is the defect
   class that stranded 36 timelines in August.
2. **A deploy path for `prompts/`** — checksum-compare, `--apply`, verify-after, modelled on
   `config/deploy-profiles.sh`. Kept because there is currently **no way to get a prompt onto the GPU
   host reproducibly**: that script copies `*.toml` only and `~/vid-to-text` on the desktop is an
   unversioned file copy.
3. **The chart/screencast prompt itself**, shipped as a profile setting `ollama.prompt_template_path`
   so the general prompt stays the default for non-chart video. The mechanism already exists;
   `prompt_template_path` must sit explicitly under `[ollama]` and be pinned by a test through the
   real merge path, because PR-022 shipped a table-re-homing defect this way once.
   Directions, graded by what the research actually supports:
   - *Remove the human/cinematography presuppositions* — best-supported. Add-side effect proven on
     Qwen2-VL (78.49 → 37.70 on a length-immune metric, ACL 2026) with the causal pathway isolated by
     attention-head ablation; removal itself unmeasured anywhere.
   - *Make it shorter* — second lever, independent mechanism: instruction tokens take 38–60% of
     early-layer attention mass, and a neutral-text control shows elongation alone has a cost.
   - *Name the content class, and state the interface conventions* — including that the header shows
     the **hovered** candle. No evidence either way; included because it is cheap and targets the
     worst observed failure directly.
4. **A before/after check on two excerpts**, reported honestly and without inventing statistics:
   boilerplate rate (one fixed regex, stated before the run), words per segment, `stated` count, and
   fidelity precision/recall as a **guardrail** — did anything collapse — **not** as an objective.
   Plus reading the output.

**Explicitly out of scope, with what each costs:**
- **The κ calibration study.** Consequence: fidelity may not be used to *rank* prompt variants, per
  `docs/ARCHITECTURE.md`. Using it as a regression guardrail is not ranking, so the rule is respected
  rather than bent — but no claim of the form "prompt B is more accurate than prompt A" may be made.
- **Paired bootstrap, configurable β, per-video scoring, the 6-cycle loop, the tuning/held-out split.**
  Consequence: no statistical claim about accuracy. Per the research, that claim was likely not
  available at this effect size anyway.
- **The chronology detector.** Consequence: the cursor-hover fabrication is addressed in the prompt
  and checked **by reading**, not measured. It may persist undetected. This is the sharpest thing
  given up and is recorded as such.
- Changing sampling, whisper, or any PR-020/PR-022 locked dimension; re-opening `use_transcript` or
  OCR grounding; fine-tuning; running the full corpus.

## Architecture section implemented

`docs/ARCHITECTURE.md` — the vision prompt surface, and the record of which prompt the market-research
operating point uses. (Prompt provenance itself is implemented by PR-027.)

## Verification criteria

- [x] `CaptureInfo` records a prompt identifier and content hash; timelines written before this change
      still deserialise
- [x] `prompts/` deploys to the GPU host by checksum-compare with verify-after; a mismatch is reported
      by name
- [x] The new prompt is selected by profile; the general prompt remains the default for non-chart
      video; `prompt_template_path` under `[ollama]` survives the real profile-merge path (test)
- [x] Before/after run on two excerpts with the old and new prompt, at identical sampling and a frozen
      `fidelity.rs`
- [x] Boilerplate rate reported as a rate over segments, using one regex fixed before the run
- [x] Words per segment and `stated` count reported beside it, so a drop in stated numbers is visible
      rather than hidden inside a rate
- [x] Fidelity precision/recall reported as a guardrail with **no** claim that the new prompt is more
      accurate; a collapse in either is a blocker
- [x] The cursor-hover behaviour checked by reading output on the segment class where it occurred
- [x] `cargo test --workspace` passes

## Research backing

**Tier-2, complete.** Phase 1 state assessment, Phase 2 scoping, Phase 3 (Tier-A rounds plus two
Tier-C harness runs at ~1.5M and ~1.7M subagent tokens), Phase 4 Amend — all recorded above under
`## Research findings`. Group D was waived by user direction.

**What the research settled, with epistemic status:**

- **The problem is proven on this model family.** Presupposition-induced accuracy loss of 31–50 pp on
  CountBench including Qwen2-VL-7B, peer-reviewed (ACL 2026), on metrics verified length-immune, with
  the causal pathway isolated by attention-head ablation. **Status: proven.**
- **The remedy is not.** No source runs the removal ablation; every mitigation in the literature is
  model-side. **Status: best-guess-given-constraints** — and this is the honest label for this PR's
  central claim.
- **Prompt structure will not buy factual accuracy.** The only direct simple-vs-detailed comparison
  found moves accuracy 1–3 pp and its authors attribute chart accuracy to *legibility* instead
  (PlotPick, arXiv:2605.06021) — converging with PR-024's finding that ~10 px header glyphs sit below
  one 32 px visual token. **Status: proven** for chart extraction; **best-guess-given-constraints**
  for open-ended description, where no primary source exists.
- **Shorter is a defensible second lever**, on an independent mechanism (instruction/visual token
  attention competition, arXiv:2605.28123). **Status: proven** on LLaVA/InstructBLIP; no Qwen tested.
- **The primary measure is now decided** — see below. **Status: proven** in its components,
  **best-guess-given-constraints** as a combination, since no cited source uses it together.

**Primary measure for Phase 6 (locked; not revisable after cycle 1):**

- **Primary:** the **invented-number rate** — of the plain numbers a segment states, the share with
  nothing within **5%** anywhere in the OCR of its own frames. Least circular instrument available:
  no prompt can make an absent value appear in the reference. The 5% relative tolerance matches
  published practice (PlotPick).
- **Joint secondary, never read alone:** fidelity precision **and** recall, combined at **β = 1**.
  A hallucination rate "can be interpreted only in reference to the relative recall achieved"
  (arXiv:2507.20077); F0.5 was rejected because it pre-favours the terser arm, which is exactly this
  PR's expected change.
- **Chronology:** PR-027's detector, reported before and after. The primary measure cannot see this
  defect — every number in the fabrication is on screen — so it is reported separately and never
  averaged in.
- **Covariates, always reported:** `stated` count, mean segment length, and the boilerplate rate.
- **Error estimate:** paired segment-level bootstrap over the same segments in both arms, valid
  because arms are segment-aligned by construction (verified: bit-identical spans, frame lists, and
  `prominent` counts).
- **Resolution floor:** bounded by the metric's own agreement with human judgment. Even the
  best-validated instruments in this literature reach only κ ≈ 0.6 / Pearson r ≈ 0.48, so an effect
  smaller than PR-027's measured κ noise **is not resolvable and must not be claimed.**

**An expectation set deliberately, before any cycle runs.** The boilerplate goal is well-supported and
measured on this corpus. The **number-accuracy** goal may be capped by frame resolution regardless of
what the prompt says. Phase 6 is explicitly permitted to report "boilerplate fell, accuracy did not
move" as its result; that is a finding, not a failure to iterate away.

**Known hazard.** PR-015 chose the current prompt using the CRISPE framework and no measurement, and
it has been in place ever since. This PR must not replace one unmeasured prompt with another; the
Phase 6 stopping rule and held-out split exist for that reason.

## Notes

- **No capture under the current prompt survives anywhere.** The 13 videos were deleted by user
  direction (2026-08-25), and Phase 1 found the position is broader than that: `_pending_corpus.py`
  reports `done=0 pending=68 stale=0`, and every corpus-episode job cited in PR-023's and PR-024's
  tables is gone from the desktop. Phase 6 therefore regenerates its own baseline as cycle 0, at the
  measured 0.40x realtime — about 24 minutes of GPU for the four tuning excerpts, paid again for every
  arm of every cycle. (An earlier draft of this note estimated "20 minutes on two videos"; the unit and
  the set were both fixed in Phase 2.)
- Prompt work is open-ended by nature. The stopping rule is the guard against that, and it is fixed
  in Phase 2 rather than discovered during iteration.
- The eight excerpts the split names still have to be cut; the three already on the desktop
  (`2024_2_19`, `2024_6_24`, `2025_05_26`) are not among them, which is the contamination rule working
  rather than a waste.
- `prompts/format.txt` (the GPT format prompt) has the same content-mismatch question and is **not**
  addressed here — one PR, one thing. Worth its own work item.
