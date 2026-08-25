# Architecture

## System Overview

vid-to-text is a client/server system that converts mp4 videos into structured JSON combining speech transcription, visual scene descriptions, and sound event tags. The CLI client runs on the user's laptop and dispatches processing jobs to a server running on a desktop with an RTX 4090. The server orchestrates ffmpeg, Whisper (CPU), and Qwen3-VL via Ollama (GPU) to produce a unified, timestamped segment timeline.

## Components

| Component | Location | Description |
|-----------|----------|-------------|
| `vtt-client` | Laptop | Rust CLI binary. Sends mp4 files to the server, receives JSON results. Handles config, output writing, batch directory processing. |
| `vtt-server` | Desktop | Rust HTTP server. Receives jobs, chunks video, dispatches to Whisper and Qwen3-VL, merges results, returns JSON. |
| `ffmpeg` | Desktop | External dependency. Splits video into time-based chunks, extracts audio tracks, extracts frames. |
| `Ollama` | Desktop | External dependency. Serves Qwen3-VL-8B-Thinking model via HTTP API. |
| `Whisper` | Desktop | Integrated via `whisper-rs`. Runs on CPU for speech transcription and non-speech sound tagging. |

## Data Flow

```
                         LAPTOP                                    DESKTOP
                    ┌──────────────┐                        ┌─────────────────────┐
                    │  vtt-client  │                        │     vtt-server      │
                    │              │    HTTP/Tailscale       │                     │
 video.mp4 ────────▶  upload     ─────────────────────────▶│  receive + save     │
                    │              │                        │       │              │
                    │              │                        │  ffmpeg: chunk video │
                    │              │                        │       │              │
                    │              │                        │  for each chunk:     │
                    │              │                        │    │                 │
                    │              │                        │    ▼                 │
                    │              │                        │  Whisper (CPU)       │
                    │              │                        │  → [SPEECH] [SOUND]  │
                    │              │                        │    │                 │
                    │              │                        │    ▼ transcript      │
                    │              │                        │  Ollama (GPU)        │
                    │              │                        │  → [VISUAL]          │
                    │              │                        │    │                 │
                    │              │                        │    checkpoint chunk  │
                    │              │                        │                     │
                    │              │                        │  merge & sort        │
 video.json ◀───────  write output◀────────────────────────│  return JSON         │
                    │              │                        │                     │
                    └──────────────┘                        └─────────────────────┘
```

Per chunk, Whisper runs first (CPU) so the transcript can be passed to Qwen3-VL (GPU) as context. Chunks are processed sequentially in v1. Parallel chunk processing is a future optimization, though it requires care around cross-chunk context continuity.

## Frame Sampling

Frames are extracted per chunk by ffmpeg at the candidate rate `vision.fps`. Two modes share one code
path; the only difference is the filter chain.

**Fixed mode** (`vision.adaptive.enabled = false`, the default): every candidate frame is kept —
`-vf fps=N,metadata=print:file=…`. Behaviour is unchanged from earlier versions.

**Adaptive mode** (`vision.adaptive.enabled = true`): a frame is kept when any of these holds —

| rule | expression | parameter |
|---|---|---|
| first frame of the chunk | `eq(n,0)` | — |
| floor: bounded gap since the last kept frame | `gte(t-prev_selected_t, G)` | `max_gap_secs` |
| trigger: content change, de-clustered | `gt(scene, T) * gte(t-prev_selected_t, R)` | `scene_threshold`, `min_trigger_interval_secs` |

run as `fps=N,select='…',metadata=print:file=…` with `-vsync vfr` (**required**: without it ffmpeg
duplicates frames to hold a constant rate — 393 files were produced for 9 selected frames on 4.4.2).
If a chunk still exceeds `max_frames_per_chunk`, the lowest-scoring triggers are dropped; floor frames
are never dropped, and validation rejects a cap smaller than the floor count. Dropping a trigger that
sat between two floor frames can widen that one gap to at most 2 x `max_gap_secs`.

`scene` is ffmpeg's `clip(min(mafd, |Δmafd|)/100)` on luma against the *previous candidate frame*
(FFmpeg 4.4 `f_select.c`). Consequences the design accepts and documents: the signal sees change
**onsets** (a chart switch, the start of a pan) and not settled states (the end of a pan scores ~0),
so settled states are captured by the floor within `max_gap_secs`; and small-region changes (a new
candle, a number tick) score below any usable threshold and are likewise covered by the floor, not by
triggers. Calibration on the market-research corpus is recorded in
`prs/PR-022-content-adaptive-frame-sampling.md`; other content should be re-characterised with the
same sweep before its thresholds are trusted.

**Timestamps are real.** ffmpeg logs `pts_time` (and `lavfi.scene_score`) for every kept frame on
stderr (`-nostats` keeps progress lines out); the pipeline parses it, requires the frame-file count to
equal the metadata count, and
carries `FrameSample { path, timestamp, scene_score }` through to vision. A visual segment spans from
its first frame's timestamp to the next batch's first timestamp (or the chunk end). For uniformly
spaced frames this is identical to the previous `frame_offset / fps` arithmetic, which is pinned by a
test; for adaptive frames it is the only correct answer. These timestamps also bound the transcript
window (`transcript_for_window`), so they are what enforces Corpus Look-Ahead Freedom when
`use_transcript = true`.

**The model is told when each frame was captured.** The prompt lists "Frame N: HH:MM:SS.mmm" for the
batch. Qwen3-VL's own processor interleaves `<X.X seconds>` text before each frame's vision tokens, and
Ollama renders all images before the message text, so ordered labels are the faithful equivalent.

**OCR grounding** (`vision.ocr_grounding`, off by default). With it on, each frame's line also carries
the text OCR read from that frame — items above `min_score`, in reading order, capped at
`max_items_per_frame` — and the prompt states that OCR is a reading aid which can misread digits, that
the images are authoritative, and that nothing may be reported which is not visible in the image.

*Why.* The model misreads on-screen digits rather than inventing them: measured on this corpus it wrote
`70708` for a header reading `O71708`, and `71948` for `H71958`. Qwen3-VL quantises a 1080p frame into
32x32 px cells (2,042 visual tokens) while TradingView header glyphs are ~10 px tall, so the digits are
smaller than one visual token. OCR text is a *visual* signal taken from the frame's own timestamp, so
unlike transcript conditioning it costs nothing in look-ahead freedom or audio independence. Published
support and its documented failure mode (error propagation from a weak OCR engine) are cited in
`prs/PR-024-ocr-grounded-vision-prompt.md`.

*Cost.* OCR runs once per job, pre-spawned one chunk ahead so it overlaps GPU work (the pattern already
used for Whisper), and the result is reused by the fidelity diagnostic instead of being recomputed. The
per-request context pre-flight adds `max_items_per_frame x tokens_per_item x max_frames_per_request`.

**Token ceiling per request.** Ollama sends frames at native resolution; measured on the deployed
stack, a frame costs `⌈w/32⌉·⌈h/32⌉ + 2` tokens (1080p = 2,042, 720p = 882, 360p = 222). Before any
GPU time is spent, the pipeline probes the source resolution and rejects a job whose
`max_frames_per_request × tokens_per_frame + ollama.prompt_reserve_tokens` would exceed
`ollama.num_ctx` — an overflow would otherwise be truncated silently. The per-chunk cap bounds cost
(worst case = the fps-0.25 arm's density); the per-request cap bounds context.

**Cost model.** A request costs image prefill (~1.07 s per 1080p frame, measured) plus generation,
and generation dominates once frames are informative: under adaptive sampling each kept frame shows
something new, so a 15-frame request generates as much text as a 15-frame uniform request that
showed one unchanged chart. Measured on clip900 under the market-research profile: 87 frames, five
180 s chunks at 33–84 s of vision each (~2.9 s per frame), **0.35 x realtime end-to-end** — about
16 h for the 45.4 h corpus, versus ~24 h for the fps-0.25 arm at the same request count. Runtime is
bounded above by the cap (3 requests per chunk) and below by the floor (1 request per chunk), both
computable from duration alone; the frame count alone does not predict it.

**Provenance.** `Timeline.capture` records the sampling parameters, models, chunking and transcript
settings; every visual segment records the timestamps of the frames it was generated from
(`frames`). Both are omitted when absent, so older timelines and checkpoints load unchanged.

## Fidelity Diagnostic

A post-run check of what each visual segment **says** against what was **on screen**, made possible
by PR-022's provenance (`Segment.frames`). Configured under `[fidelity]`; off by default; diagnoses
only and never edits a segment (Segments Are Immutable After Merge).

**Facts.** The OCR-checkable classes of the CHOCOLATE chart-caption error typology (Huang et al.,
ACL 2024): numeric values (`42,000`, `$1.66T`, `44k`, `-0.25%`), labels (uppercase on-screen tokens —
tickers, exchanges, indicators) and timeframes (`15m`, `4H`, `1D`, `1W`, `1M`). Trend and magnitude
claims cannot be checked from a screenshot and are not scored; dates, spelled-out numbers and
mixed-case names are not yet extracted (documented in `vtt-core/src/fidelity.rs`).

**Matching.** A stated number matches an on-screen number when the difference is within half of the
stated number's own last digit — `1.66T` matches 1.661T, `42,000` matches 41,958, `70142` only
70142 — plus an optional relative `number_tolerance`. Plain integers ("2020", "1") are exact; only grouped or suffixed integers ("42,000", "44k") round
to their last digit. Percentages only match percentages; labels and timeframes match exactly after
normalisation, and uppercase prose words (`label_stoplist`: THE, US, …) are not treated as labels.

**Two references.** *Precision* is scored against OCR of the frames the segment was generated from
(a stated fact absent from them is a hallucination — or an OCR miss, which the review sheet exists
to expose). *Recall* is scored against prominent facts in the configured reference:
`recall_reference = "kept"` (the same frames; the per-job diagnostic) or `"candidates"` (OCR of every
candidate frame in the window — what was actually on screen, identical across sampling settings and
therefore the study mode). Prominent = persisted ≥ `min_persist_secs` across the window's reference
frames and OCR box height ≥ `min_text_height_px`.

**OCR.** An external command (`[ocr].command`, default the repo's `tools/ocr_frames.py` wrapping
RapidOCR — PP-OCR models on ONNX Runtime, CPU) prints one JSON record per frame; `[ocr].workers`
processes × `[ocr].threads` inference threads each (onnxruntime's default takes every core per
session, so parallel workers only serialise without the bound — measured 1.85 → 0.5 s per 1080p frame
at 8 × 2 on 32 cores). The same `[ocr]` engine serves the diagnostic and OCR grounding, and each job
OCRs its frames once.

**Circularity, when grounding is on.** If the vision prompt was grounded on OCR, precision is scored
against the same text the model was given, so it approaches 1.0 by construction and is **not** evidence
of accuracy. `FidelitySummary.ocr_grounded` records this and the log line says so; validating grounding
requires reading frames by hand. Chosen by
measurement on this corpus: 76/76 legible price-axis values recovered vs 42/76 with digit
substitutions for tesseract 4.1.1 (`docs/0.0/DESIGN-log.md`, 2026-08-25). `/health` and `doctor`
report it when the diagnostic is enabled in the server's base config.

**Outputs.** `Timeline.fidelity` carries the summary (reference, counts, precision, recall, F0.5);
`fidelity.json` beside the results carries per-segment detail; `frames/` beside the results holds
thumbnails of every kept frame (`thumbnail_width`, `thumbnail_quality`) so the check and the review
stay reproducible after the job's working directory is cleaned.

**Review.** `vid-to-text review <results-dir>` renders a self-contained HTML sheet — thumbnails,
description, judged facts — sampled disagreement-first (every fact the metric called unsupported or
missed, then supported ones), and `--labels` scores the copied judgments: Cohen's κ between metric
and reviewer, the reviewer's precision, false hallucinations (OCR/matching misses) and missed
hallucinations (matches to the wrong number). The metric is not trusted for tuning until that κ has
been reported.
`vid-to-text rescore <results-dir>` recomputes a report offline from the persisted kept-frame OCR
(`ocr.json`) — against a candidates reference (raw wrapper output for uniformly spaced frames) or
with a different tolerance — without touching the GPU; the sampling study is scored that way.

## Key Abstractions

| Abstraction | Description |
|-------------|-------------|
| `Chunk` | A time-bounded segment of a video with start/end timestamps. Unit of processing and checkpointing. |
| `Segment` | A single output entry: type (`speech`, `visual`, `sound`), start time, end time, content text. |
| `ChunkArtifacts` | Extracted audio (WAV) and `FrameSample`s — JPEG path, real timestamp, scene score — for a single chunk, produced by ffmpeg. |
| `ChunkManifest` | All artifacts for a job: job directory, video duration, and list of `ChunkArtifacts`. |
| `Timeline` | The merged, sorted collection of all `Segment`s for a video. Serializes to the final JSON output. |

## Storage

No database. File-based only:

- **Chunk checkpoints**: Completed chunk results stored as JSON files at `{temp_dir}/{job_id}/checkpoints/chunk_NNN.json`. Atomic writes (tmp + rename) ensure crash safety. Enables resumability.
- **Final output**: JSON file written alongside the input mp4 (default) or at a user-specified path.
- **Config**: TOML files at `~/.vid-to-text/config/`. Client uses `client.toml`, server uses `server.toml` (separate files since they run on different machines).

## Capture Configuration

The operating point for market-research corpus capture. Every value below is research-backed; see
`prs/PR-020-market-research-capture-config.md` Research findings for sources and epistemic status.

| Setting | Value | Basis |
|---|---|---|
| `whisper.beam_size` | 5 | Whisper paper Sec 4.5 prescribes beam-5 from temperature 0; greedy is documented to fall into repetition loops on long-form audio. |
| `whisper.initial_prompt` | `""` (off) | Upstream-documented failure mode: the prompt feature "will cause specific hallucinations and repetitions". Its jargon benefit did not materialise on this corpus. |
| `whisper.entropy_thold` | 2.4 | whisper.cpp default; its analogue of OpenAI's `compression_ratio_threshold`. **Retry trigger, not a filter.** |
| `whisper.logprob_thold` | -1.0 | whisper.cpp / Whisper paper Sec 4.5 default. |
| `whisper.no_speech_thold` | 0.6 | whisper.cpp / Whisper paper Sec 4.5 default. |
| `whisper.temperature_inc` | 0.2 | Temperature ladder 0.0 -> 1.0. Note the paper's own Table 7 shows zero WER gain from fallback. |
| `vision.use_transcript` | `false` | Audio-to-vision conditioning is the documented-harmful direction: language priors displace visual grounding. |
| `vision.transcript_window` | `causal` | Enforces the Corpus Look-Ahead Freedom constraint. |
| `ollama.temperature` | 0.0 | Removes sampling variance. |
| `ffmpeg.chunk_duration_secs` | 180 | Retained. Its original stated rationale (a "768-frame cap") is unsupported — see `docs/0.0/DESIGN-log.md`. |
| `vision.fps` | 2.0 (candidate rate) | Adaptive mode evaluates candidates at 2 fps — Qwen3-VL's native video fps — and keeps ~1 in 23. PR-022 design session; the 74-video sweep showed 88.9% of 2 fps candidates are visually unchanged. |
| `vision.adaptive.enabled` | `true` | See **Frame Sampling**. Mechanism proven; benefit over uniform at equal budget on this content is best-guess-given-constraints (literature split, no scoring metric exists) — fixed mode remains one line away. |
| `vision.adaptive.scene_threshold` | 0.08 | Corpus-measured: 0.037 = crosshair redraw, 0.076/0.126 = chart pan/zoom steps, 0.448 = slide change. No literature value exists. |
| `vision.adaptive.max_gap_secs` | 15 | Floor (12 of the ~15 frames a chunk keeps). Bounds the settled-state lag the onset-only signal leaves; precedent for a uniform floor: Gemini 1 fps, Video-MMLU 0.2–1 fps. |
| `vision.adaptive.min_trigger_interval_secs` | 2 | 17% of raw triggers at T 0.08 fall within 2 s of a kept frame (bursts during redraws); de-clustering removes them at no coverage cost. |
| `vision.adaptive.max_frames_per_chunk` | 45 | Ceiling = the fps-0.25 arm's density (3 requests per chunk); never binds on this corpus (mean 15.4, p95 22, max 29 per chunk); bounds worst-case corpus cost at ~26 h. |
| `whisper.repetition_window_secs` | 30 | Repetition is scored over 30 s windows — the unit OpenAI calibrated 2.4 on. Recovered two real loops per-segment scoring missed, one in the locked-config transcript. |

**Deliberately not locked**, recorded so the gap is legible rather than implied-settled:

| Setting | Value carried | Why not locked |
|---|---|---|
| `vision.max_frames_per_request` | 15 | Measured effect of raising it was -5.9% wall time, not statistically established. |
| `whisper.model_path` | `large-v3-turbo` | Measured equal to `large-v3` on repetition and content retention, at 2.3x lower cost. |

**Reproducibility, stated honestly.** `temperature = 0` removes sampling variance but does **not**
deliver bit-identical output. Non-determinism at temperature 0 arises from batch-size dependence of
reduction kernels, and the accepted fix — batch-invariant kernels, ~60% throughput overhead — is not
implemented by Ollama. A fixed seed does not help, because greedy decoding has no sampling step to
seed. Runs are repeatable in distribution, not bit-exact.

**Repetition guarding is asymmetric.** `truncate_repetition` (`vtt-core/src/vision.rs`) guards vision
output only. Whisper output is ungated: its in-decoder thresholds are retry triggers that accept the
result unconditionally at the final temperature. Post-hoc `compression_ratio` flagging closes this,
as a diagnostic that flags rather than a filter that edits.


## Testing Strategy

| Level | What | Where |
|-------|------|-------|
| Unit | Config parsing, timestamp math, segment merging, chunk planning, checkpoint I/O | Inline `#[cfg(test)]` modules |
| Integration | ffmpeg probing, Whisper transcription, Ollama vision, full pipeline | `#[ignore]` tests requiring external deps |
| System | End-to-end: real mp4 → real models → correct JSON output | `vtt-core/tests/system_test.rs` |
