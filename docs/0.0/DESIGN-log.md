# Design Log

Design decisions and rationale from planning sessions. Append new sessions below.

---

## Session: 2026-03-28 — Initial Design

### Context

Greenfield project. User wants a CLI tool to convert mp4 videos into structured text combining speech transcription, visual scene descriptions, and sound event tags. Prior research confirmed no existing tool does this — the project fills a gap in the ecosystem.

### Research Summary

Evaluated Video-LLaVA (outdated, 8 frames only), InternVideo2.5 (capable but heavy, research-grade), Qwen2.5-VL-7B (strong but superseded), and Qwen3-VL-8B (best fit). Chose Qwen3-VL-8B-Thinking for visual analysis — fits on RTX 4090, Thinking mode produces verbose descriptions, strong temporal reasoning. Whisper via whisper-rs for speech transcription.

### Decisions

1. **Client/server architecture**: CLI on laptop, processing server on desktop (RTX 4090). Connected via Tailscale.
   **Rationale**: GPU is on a separate machine from the development laptop. Tailscale provides reliable connectivity from anywhere.

2. **Fixed time chunking (configurable, default ~3 min)**: Videos split into time-based chunks for processing.
   **Rationale**: Simple, predictable. Scene-based splitting deferred as enhancement.

   > **Corrected 2026-08-24 (PR-020 research, Q8):** the original rationale recorded here was
   > "Derived from Qwen3-VL's 768-frame cap at 2fps." **No such cap appears in Qwen3-VL's official
   > documentation.** The model specifies a native 256K context and controls video through a
   > *visual-token* budget (256-16384 tokens per video, 32x spatial + 2x temporal compression) — a
   > different quantity that also scales with resolution, not only frame count. The 180s value is
   > retained because it works and because changing it affects the look-ahead window and cross-chunk
   > continuity; only its stated justification was wrong.

3. **Ollama for model serving**: Qwen3-VL served via Ollama HTTP API on desktop.
   **Rationale**: Mature tooling, easy model management, clean HTTP interface. Avoids Python subprocess complexity or immature ONNX export.

4. **Parallel GPU+CPU processing**: Qwen3-VL on GPU, Whisper on CPU simultaneously.
   **Rationale**: Avoids VRAM contention (Qwen3-VL needs ~16-20GB, leaving no room for Whisper). i9-13900k handles Whisper at reasonable speed.

5. **Whisper non-speech tags for [SOUND] stream (v1)**: Leverage Whisper's incidental non-speech tags rather than a dedicated classifier.
   **Rationale**: Free — already running Whisper. Good enough for v1. Dedicated audio classifier (hybrid approach) deferred.

6. **Flat sorted timeline**: All segments merged into a single list sorted by start time.
   **Rationale**: Simplest structure, most flexible for consumers. Nesting/grouping can be done by a presentation layer.

7. **TOML config with CLI overrides**: Config at `~/.config/vid-to-text/config.toml`, CLI flags override.
   **Rationale**: TOML is idiomatic Rust, human-readable, supports comments. Separate client/server configs.

8. **Chunk-level checkpointing**: Completed chunks saved to disk for resumability.
   **Rationale**: Long videos may take significant time. Losing progress on failure is unacceptable. File-based state keeps it simple.

9. **Dependencies pre-installed with doctor command**: User installs ffmpeg, Ollama, etc. manually. `vid-to-text doctor` validates setup.
   **Rationale**: Single user, personal tool. Docker is overkill.

### Scope

**v1**: Single/batch mp4 processing, [SPEECH]/[VISUAL]/[SOUND] streams, JSON output, client/server, TOML config, resumability, doctor command.

**Later**: Wake-on-LAN, YouTube URL support, dedicated sound classifier, human-readable output layer, SRT/VTT export.

### Changes to docs
- Populated ARCHITECTURE.md with full system design
- Added domain constraints to CONSTRAINTS.md
- Created PR plan in ROADMAP.md
- Created PR description files in prs/

## Session: 2026-03-29 — Post-v1 Enhancements & Optimization

### Context

v1 complete and system-tested. Session focused on post-v1 features, quality improvements, and performance optimization through iterative testing with real YouTube videos.

### Decisions

1. **YouTube URL support (PR-011)**: Server downloads via yt-dlp, client sends `--url` flag. Configurable resolution/FPS.
   **Rationale**: User shouldn't need to manually download videos.

2. **Human-readable format command (PR-012)**: GPT-5.4 via OpenAI API produces Markdown from JSON. Separate `format` subcommand runs client-side.
   **Rationale**: Machine-readable JSON needs a presentation layer. Separate command allows re-formatting without reprocessing.

3. **Q8 Instruct model over Q4 Thinking (PR-013)**: Switched from `qwen3-vl:8b` (Q4_K_M, Thinking) to `qwen3-vl:8b-instruct-q8_0`. 3x more reliable, faster per batch, no empty response issues.
   **Rationale**: Q4 Thinking variant produced empty responses non-deterministically. Q8 Instruct uses more VRAM (20GB/24GB) but eliminates the problem.

4. **Granular visual segments (PR-013)**: One visual segment per batch (~7.5s) instead of one per chunk (3min). Went from 2 to 28 visual segments.
   **Rationale**: Fine-grained timestamps correlate better with speech for the format command.

5. **Overlapped processing (PR-014)**: Pre-spawn Whisper(N+1) on CPU while Vision(N) runs on GPU. Cross-chunk context passed to vision prompt.
   **Rationale**: Whisper is free when overlapped. Cross-chunk context prevents vision model from losing continuity at chunk boundaries.

6. **Release build**: Whisper 2.6x faster in release vs debug (45s vs 118s for 3-min chunk).
   **Rationale**: Free performance win for CPU-bound work.

7. **CRISPE prompt framework (PR-015)**: Prompts externalized to `prompts/vision.txt` and `prompts/format.txt`. Structured with Context, Role, Instruction, Specification, Parameters, Examples.
   **Rationale**: Prompts were too long for source code. CRISPE produces robust, situation-agnostic prompts. Editing prompts no longer requires recompilation.

8. **num_ctx=65536**: Increased Ollama context window from default to 65536. Enables reliable multi-image requests.
   **Rationale**: Default context window caused empty responses with 15+ images.

### Performance summary (3.5 min TED-Ed video)

| Stage | First run | Final run |
|-------|-----------|-----------|
| Whisper (chunk_0) | 125s (debug) | 45s (release) |
| Vision (chunk_0) | 648s (Q4) | 639s (Q8 + CRISPE) |
| Total | 883s | 790s |

### Quality improvements
- Vision: spatial storytelling, character tracking, OCR, expression analysis
- Format: scene-based grouping, character identification with confidence, narrator attribution
- Tested on animation (TED-Ed) and live chart analysis (crypto) — both produce high-quality output

### Changes to docs
- Updated CLAUDE.md with CLI usage, key config, Q8 model recommendation
- Updated ARCHITECTURE.md with sequential Whisper→Vision flow
- Updated ROADMAP.md with Phase 5 post-v1 PRs

## Session: 2026-08-24 — Content-Adaptive Frame Sampling (PR-022)

### Context

PR-020 Phase 5.5 escalated `vision.fps` here: the diversity metrics structurally cannot choose an fps
(length- and time-coverage-confounded), and direct ffmpeg measurement showed every fixed rate tested
oversamples static chart content by an order of magnitude while the change rate varies ~3x across
videos. The premise — is a clock the right trigger at all? — is a design question, so this session ran
`PROCEDURE-design-planning.md` from Phase 1 with a full state assessment (recorded in
`prs/PR-022-content-adaptive-frame-sampling.md` § Research findings).

**Approval-gate waiver.** On 2026-08-24 the user explicitly authorised autonomous execution of this PR
through completion "with your leans, no approval gates by me". Phases 2–5 below therefore ran without
per-phase user approval. Every decision is labelled with its epistemic status so the waiver does not
launder a best-guess into a proven result.

### Research summary

Evidence gathered (Tier A inline probes + one Tier C harness, `wf_bef168b0-50b`, 32 agents,
1.58M tokens — over the ~500k tier target; flagged for the operator).

**Corpus measurement (proven, this corpus).** 74 videos, 45.36 h, 326,663 candidate frames. Scene-score
sweep at the 2 fps candidate rate over the whole corpus (ffmpeg 4.4.2 `select` `scene`; ~5 s per
video): 82.6% of candidates score < 0.005 and 88.9% < 0.01 — uniform 2 fps sampling captures a
visually unchanged frame nine times out of ten. Trigger rate at T > 0.08 is 2.6/min raw corpus-wide
with a 6.7x per-video spread (0.71–4.77/min); at T > 0.10, 1.6/min. Inter-trigger gaps at T > 0.10
reach 1,171 s and 2,730 s at T > 0.30. Trigger bursts are real: 17% of raw triggers at T 0.08 fall
within 2 s of a previous kept frame. Under the chosen rule (T 0.08, G 15 s, R 2 s) the corpus keeps
15.4 frames per 180 s chunk on average (p50 15, p95 22, max 29; 945 chunks), so a cap of 45 never
binds.

**What the score bands contain (proven by inspection, clip900).** Before/after frame pairs were
extracted at representative scores. 0.037 = crosshair/hover redraw (cursor-induced, not content);
0.076 and 0.126 = chart pan/zoom steps (real view changes); 0.448 = title card → chart. A chart pan
also appeared at **0.012**: `scene` is `clip(min(mafd, |Δmafd|)/100)` on luma against the *previous
input frame* (FFmpeg 4.4 `f_select.c`), so sustained motion is suppressed by construction — the
sampler sees change **onsets**, not settled states. Settled states must come from the floor.

**Alternatives measured and rejected.** `mpdecimate` compares against the last *kept* frame (a better
signal in principle), but its `frac` is relative to (w/16)(h/16) blocks while positions are sampled at
stride 4, so the effective area fraction is `frac/16`; even `frac=0.20` kept 1,511/1,800 frames on
clip900 and the parameter cannot be documented honestly. Rejected.

**ffmpeg 4.4.2 behaviour (proven, empirical + `man ffmpeg-filters`).** Without `-vsync vfr`,
frame-dropping filters make ffmpeg **duplicate** frames to hold a constant rate: 393 files were written
for 9 selected frames. `metadata=print` logs `pts_time` and `lavfi.scene_score` per kept frame (to a
file with `file=`, or to stderr; the implementation uses stderr to avoid filtergraph path escaping);
`-frame_pts 1` encodes pts in the filename in the `fps` output timebase. `select` exposes
`prev_selected_t`, which gives a true max-gap floor and a refractory interval in one expression.

**Model/serving facts (proven, measured on the deployed stack).** Ollama 0.18.3 sends frames at native
resolution: `prompt_eval_count` minus text = **2,042 tokens for 1080p, 882 for 720p, 222 for 360p**
— exactly Qwen3-VL's 32-px-unit formula with no downscale cap. Fifteen 1080p frames = 30,640 tokens
of the 65,536 context and ~16 s of prefill; 30 is the most that fit beside a 4,096-token prompt
reserve, and anything above would be silently truncated. Qwen3-VL's
own HF processor interleaves `<X.X seconds>` text before each frame's vision tokens
(`processing_qwen3_vl.py`), Ollama's renderer places all images before the message text
(`model/renderers/qwen3vl.go`), and a direct test showed the model correctly attributes content to
per-frame time labels given as "Frame N: t=… s" text. Gemini's production video pipeline is a fixed
1 fps floor at 258 tokens/frame with MM:SS references (Google AI docs) — precedent for a uniform
floor, not for adaptive selection.

**Literature (Tier C, conflicting; transferred).** No source measures uniform vs change-triggered vs
hybrid on static screen content. GUI-World (ICLR 2025): uniform 3.44 vs embedding-based change
detectors 3.58–3.61 vs a generic frame-difference extractor 3.30 (1–5 scale, no variance). Li & Shi
2026 (verifier-surfaced, unfetched): uniform 1 fps, pixel-diff and CLIP-adaptive statistically
indistinguishable. Natural video is split (Brkic: uniform best at 96 frames on 2–3B models; AKS: +3.8
on LongVideoBench with query-conditioned selection). No validated change threshold exists anywhere;
the only concrete numbers are one practitioner's unvalidated defaults (mean-abs-diff/255, hot 0.030 /
cold 0.020 — the same order as our 0.08–0.10 in `scene` units). Lecture benchmarks (Video-MMLU) use
uniform 32 frames/video and manual 0.2–1 fps floors without ablation. Cost is bounded everywhere by
fixed frame budgets, never by a frames-per-minute floor. The harness recommended an in-house
comparison over a Tier D; PR-020 Phase 5.5 already established that no available metric can score
description quality across sampling regimes, so that comparison is deferred to a future work item
rather than faked here.

**Repetition-threshold unit (proven, this corpus).** Re-scoring all 33 server results over 30 s
windows found two genuine hallucination loops that per-segment scoring missed — including one in the
**locked-config** beam-5 transcript of `2024_2_12` ("So let's go to the 4th hour." x15 at 09:57–10:31:
per-segment CR max 0.85, window CR 4.90). Window scoring matches the unit OpenAI calibrated 2.4 on.

### Decisions

1. **Hybrid sampling: uniform floor + change-triggered frames, with a per-chunk cap.** Keep a frame
   when it is the first of the chunk, or ≥ `max_gap_secs` since the last kept frame (floor), or its
   scene score > `scene_threshold` and ≥ `min_trigger_interval_secs` since the last kept frame
   (trigger, de-clustered). Then, if a chunk exceeds `max_frames_per_chunk`, drop the lowest-scoring
   triggers (never floor frames).
   **Rationale:** the corpus is measured to be ~91% redundant at 2 fps and its change rate varies 4.5x
   between videos, so no fixed rate fits; the floor guarantees every persisted state is captured
   within a bounded lag (the literature's only consistent precedent — Gemini, Video-MMLU — is a
   uniform floor); triggers capture change onsets at their real time; the cap bounds cost. Fixed-fps
   mode is retained unchanged, so the choice is reversible per profile.
   **Status:** mechanism **proven** (ffmpeg behaviour, corpus structure); *benefit over uniform at
   equal budget on this content* is **best-guess-given-constraints** — the literature is split and no
   scoring metric exists. Recorded honestly; the fixed mode remains one config line away.
2. **Signal = ffmpeg `select` `scene` score at a 2 fps candidate rate; threshold fixed and absolute.**
   Per-video derivation was rejected: a percentile threshold equalises trigger *counts* across videos,
   which hides genuinely busier videos and breaks cross-corpus comparability; an absolute threshold has
   a physical meaning (mean luma change per pixel) and the floor covers the rest.
   **Status:** signal semantics **proven** (source); threshold value **measured on this corpus**
   (bands inspected), not literature-backed — none exists.
3. **Floor = `max_gap_secs`, default 15 s** (profile and code). Bounds the settled-state lag that the
   onset-only signal leaves open (a pan's end state is not a trigger). 12 floor frames per 180 s chunk,
   i.e. most of the ~15 a chunk keeps: on this corpus the floor, not the triggers, carries coverage.
   **Status:** value **measured** (cost table); the need for a floor is **proven** (signal analysis).
4. **Ceiling in two units, both enforced.** Per request: the existing `max_frames_per_request` is the
   token ceiling, and a new pre-flight check `frames x (⌈w/32⌉·⌈h/32⌉ + 2) + prompt reserve ≤ num_ctx`
   fails the job before GPU time is spent (an overflow past 30 frames at 1080p would otherwise be silent).
   Per chunk: `max_frames_per_chunk`, default 45 — the fps-0.25 arm's density (3 requests per chunk)
   — bounding the corpus's worst case at ~26 h. Measured after implementation: cost is
   generation-dominated once frames are informative, so it tracks *requests*, not frames — clip900
   ran at 0.35x realtime (~16 h corpus), about two-thirds of the fps-0.25 arm, not the one-third the
   frame count alone suggested. **Status:** token formula **proven** (measured 2,042/882/222); cap
   value **measured** (never binds: max 29/chunk on the full corpus); cost **measured** on clip900
   (0.35x) and on the full 31-minute `2024_4_8` (0.34x realtime, 169 frames, 628.9 s).
5. **Real frame timestamps replace arithmetic, in both modes.** ffmpeg logs `pts_time` per kept
   frame via `metadata=print` (stderr, `-nostats`); the file count must equal the metadata count or the chunk fails
   (a silent mismatch would mislabel every segment). A visual segment spans from its first frame's
   time to the next batch's first frame (or the chunk end). For uniform frames this reproduces the old
   arithmetic exactly, pinned by a test. **Status: proven** (empirical, 4.4.2). This also removes the
   invariant that `transcript_for_window` silently depended on for look-ahead freedom.
6. **Config surface: `vision.fps` keeps its type and meaning; adaptive parameters live in
   `[vision.adaptive]`.** `enabled` (default false), `scene_threshold` (0.08), `max_gap_secs` (15),
   `min_trigger_interval_secs` (2), `max_frames_per_chunk` (45). In adaptive mode `fps` is the
   candidate rate at which frames are evaluated (2.0, which is also Qwen3-VL's native video fps).
   The draft's `fps = "auto"` was rejected: it hides three parameters behind one word, needs a
   tagged type with a custom deserializer, and must survive the profile merge's TOML
   serialise→merge→deserialise round-trip (`config.rs:552-570`); a plain nested struct does all of
   that for free and leaves every numeric profile untouched. **Status:** internal; no web round.
7. **Per-frame capture times go into the prompt as text** ("Frame N: HH:MM:SS.mmm"), in both modes,
   and `prompts/vision.txt` no longer asserts "regular intervals (typically 2fps)". Matches how the
   model was trained (`<X.X seconds>` interleaving) and was verified to work through Ollama's
   images-first rendering. **Status: proven.**
8. **Capture provenance is recorded in the output.** `Timeline.capture` records the sampling
   parameters, models and chunking; each visual segment records the timestamps of the frames it was
   generated from (`frames`). Fixed-fps output was reconstructible from segment spans; adaptive output
   is data-dependent and would otherwise be irreproducible. Both fields are optional and omitted when
   empty, so old timelines and checkpoints load unchanged. **Status:** internal; no web round.
9. **Repetition report scores 30 s windows, not segments.** `whisper.repetition_window_secs` (default
   30) groups consecutive speech segments; the 2.4 threshold is retained at the unit it was calibrated
   on; flags name the window and its segment count. Closes PR-020's deferred calibration item without
   retuning to one video. **Status: proven** (two real loops recovered on this corpus; OpenAI's unit
   established in PR-020 Group D).

**Explicitly not decided here (recorded as gaps):** whether the hybrid beats uniform at equal budget
on this content — needs a description-quality metric that does not exist (PR-020 Finding 1);
downscaling frames to 720p would cut per-frame cost ~57% but its effect on chart-text legibility is
unmeasured and resolution is not a PR-022 dimension.

### Convergence (Phase 3)

User-waived. The full design above was checked against every open question in the PR draft (six) and
every verification criterion; all are answered or explicitly deferred with reason. Two ambiguities
remain and are carried as documented limitations rather than hidden: onset-only detection of pans
(bounded by the floor) and the untested hybrid-vs-uniform quality question (bounded by the retained
fixed mode).

### Changes to docs
- `docs/ARCHITECTURE.md`: new **Frame Sampling** section; Capture Configuration `vision.fps` row
  replaced by the adaptive operating point; Storage config path corrected.
- `docs/CONSTRAINTS.md`: new domain constraint **Visual Timestamps Are Frame Timestamps**.
- `prs/PR-022-content-adaptive-frame-sampling.md`: scope finalised, verification criteria populated,
  research findings appended.
- `docs/0.0/RESEARCH-BACKLOG.md`: PR-022 status; known gaps recorded (36 non-conforming March
  timelines + runner skip logic; server `job.json` records no profile).
- `CHANGELOG.md`: Unreleased entry.
- `CLAUDE.md`: Key Configuration updated for `[vision.adaptive]`.
