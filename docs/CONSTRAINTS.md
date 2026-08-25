# Constraints

Hard rules that must never be violated. These are enforced by Claude at all times.

---

## Structural Constraints

These apply to every project built with this template.

### No Phantom Implementations (NON-NEGOTIABLE)

A step is NOT complete if:
- A function exists but returns a default, stub, or placeholder value
- A module is declared but never called from the main flow
- Tests only verify something exists, not that it works correctly

Every PR must include:
1. A test that exercises the **actual behavior**
2. Explicit listing of any stubs or TODOs in the PR description
3. Proof of end-to-end data flow where applicable

### Documentation Accuracy (NON-NEGOTIABLE)

Every code change must include corresponding doc updates in the same commit. Before writing docs, check the actual diff — base doc updates on what changed, not memory. Docs must never describe behavior that doesn't exist in code.

### One PR, One Thing

Each PR is a single, reviewable change. No "while I'm here I'll also add..." — that's scope creep. Each PR references the specific section of the architecture it implements.

### Config Over Hardcoding

All configurable values come from config files. Zero hardcoded parameters for behavior that might change. If a value could reasonably vary between environments or over time, it belongs in config.

---

### Research-Backed Decisions (NON-NEGOTIABLE)

Every architectural and significant design decision must be backed by research from **reputable sources**. "I think this is right" is not sufficient.

**Reputable sources:**
- Production system documentation
- Official framework source and docs
- Published post-mortems and engineering blog posts from serious engineering teams
- Battle-tested open-source code with meaningful adoption

**Not sufficient:**
- LLM intuition
- Marketing pages
- Personal blog posts without engineering weight
- StackOverflow answers without corroborating evidence

**Process:** decisions without research backing must be explicitly flagged as unresearched in the relevant PR or design doc, and researched before implementation begins. `docs/0.0/DESIGN-log.md` tracks which decisions have research and which don't. `PROCEDURE-design-planning.md` integrates research rounds into Phase 2 (Decisions).

### PR Research Procedure Required (NON-NEGOTIABLE)

No PR is implemented until `PROCEDURE-pr-research.md` has been followed and its findings are documented in the PR file's `## Research findings` section.

**Applies to all PRs — including PRs that were research-backed at design time.** State drifts between design and implementation. The procedure's Phase 1 (State Assessment) catches drift before implementation begins.

**Enforcement:**
- Every PR file starts from `prs/PR-TEMPLATE.md`, which includes a `## Before Implementation (NON-NEGOTIABLE)` section requiring this procedure
- Every PR file has a `## Research findings` section that must be populated before implementation
- PRs without completed research findings are rejected
- Research findings include a state-assessment date; if implementation hasn't started within the project's staleness threshold, re-run state assessment per the time-decay policy in `PROCEDURE-pr-research.md`

State drifts. Research must be validated before code.

### Per-Phase Approval Gate (NON-NEGOTIABLE)

In any multi-phase procedure (`PROCEDURE-pr-research.md`, `PROCEDURE-design-planning.md`, future procedures), Claude does **not** advance to the next phase without explicit user approval.

**What this means:**
- After completing a phase, Claude presents the phase output and explicitly requests permission to enter the next phase.
- "Auto-flowing" through multiple phases in a single response without user interjection is a hard violation.
- This applies even when a phase is "light" or no-op — the outcome and rationale are presented and approved before the procedure is treated as advanced.
- Implementation never begins until Phase 5 (Gate Check) has been explicitly approved.

**Why:** Phases exist to give the user explicit decision points. When Claude advances unilaterally, those decision points are skipped. The user should not have to interrupt to halt phase progression — the default behavior is to halt.

**Enforcement:**
- Every phase response ends with "Phase X complete. Awaiting approval to enter Phase X+1." (or equivalent.)
- A response covers at most one phase, then halts.

---

## Domain Constraints

### Corpus Look-Ahead Freedom

No generated segment may be produced from a prompt containing information that did not exist at that
segment's own timestamp.

This is a property of the corpus, not of any downstream consumer, and it survives into every later
use. It is violated by construction when a chunk's full transcript is fed to the vision model, since a
description covering t=0-7.5s is then generated from speech through t=180s.

**Enforcement:** `vision.transcript_window` must be `causal` (or `use_transcript = false`) for any
corpus intended for research or trading use. `full` is permitted only for human-readable transcripts
where the leak is irrelevant and the corpus will not inform a decision about the past.

**Research basis:** look-ahead freedom is formalised as Temporal Non-Interference — a verifiable
property certifying that a pipeline does not allow future information to influence a decision made in
the past — and is explicitly scoped to backtesting *and agentic trading pipelines*, not backtests
alone. "Textual sources that contain hindsight" are a named leakage vector, and such leakage is
described as something "inspection of the pipeline code cannot rule out."
See `prs/PR-020-market-research-capture-config.md` Research findings, Q4.

### Visual Timestamps Are Frame Timestamps

Every visual segment's `start` and `end` derive from the real presentation timestamps of the frames
it was generated from, never from arithmetic that assumes uniform spacing. Frame extraction must
produce those timestamps alongside the frames, and a mismatch between the frames written and the
timestamps recorded fails the chunk rather than guessing.

**Why:** under content-adaptive sampling there is no `seconds_per_frame`; arithmetic would label
every segment plausibly and wrongly. These timestamps also bound the transcript window, so they are
what enforces Corpus Look-Ahead Freedom when the transcript is used.

**Research basis:** `prs/PR-022-content-adaptive-frame-sampling.md`; `docs/0.0/DESIGN-log.md`
session 2026-08-24, decision 5.

### No Audio Data on GPU (v1)

Whisper runs on CPU only. The full 24GB of GPU VRAM is reserved for Qwen3-VL. This prevents OOM conditions during parallel processing.

### mp4 Input Only (v1)

Only mp4 files are accepted as input. No format conversion, no container sniffing. If a user has a different format, they convert with ffmpeg themselves.

### Segments Are Immutable After Merge

Once segments from all chunks are merged and sorted into the final timeline, no post-processing modifies their content. The output JSON is a faithful representation of what the models produced. Any transformation (human-readable formatting, SRT export, etc.) happens in a separate layer.

### Checkpoint Integrity

A chunk checkpoint file is only written after the chunk is fully processed by both pipelines. Partial results are never checkpointed. This ensures resumability is always safe — a checkpoint either has complete data or doesn't exist.

### No Network Calls From Client to Models

The client never communicates directly with Ollama or Whisper. All model interaction goes through the server. This keeps the client thin and the server as the single point of control for processing.

---

## Research Time-Decay

**Project staleness threshold: 30 days.**

Any PR marked `state-assessed` or `fully-researched` in `docs/0.0/RESEARCH-BACKLOG.md` more than 30 days
before implementation begins must re-run Phase 1 (State Assessment) of `PROCEDURE-pr-research.md`.

Rationale: this project depends on fast-moving external surfaces (Ollama HTTP API, whisper.cpp /
whisper-rs, vision model releases). A decision researched against one model revision can be invalidated
by the next. 30 days is the shortest interval that does not force re-assessment within a single work
session.
