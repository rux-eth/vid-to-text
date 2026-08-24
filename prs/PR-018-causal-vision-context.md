# PR-018: Causal Vision Context

**Landed-in:** (not yet landed)

<!-- NON-CONFORMING: written and implemented without PROCEDURE-pr-research.md.
     Retrofit to prs/PR-TEMPLATE.md + research backfill tracked in
     docs/0.0/RESEARCH-BACKLOG.md (Tier 2). -->

## Motivation

The vision pipeline feeds Whisper output into the vision prompt (PR-005, by design)
so descriptions read richly for a human. That choice is correct for its original
purpose and wrong for using the timeline as ML features.

Two measured defects, both on `2024_2_12.mp4`:

**1. Look-ahead bias of up to `chunk_duration_secs` (180s).**
`extract_transcript` (`vtt-core/src/pipeline.rs:221`) filters by `SegmentType::Speech`
only — there is no time filter. It concatenates the whole chunk's speech and hands
it to `describe_chunk` (`pipeline.rs:131`, `pipeline.rs:148`). A visual segment
covering t=0–7.5s is therefore generated from a prompt containing the analyst's words
through t=180s.

The leak is independent of `fps`. Measured across three arms of the same video, every
arm carries the full 180s:

| arm | visual segs | segment span | look-ahead |
|-----|-------------|--------------|------------|
| fps 2.0 | 326 | 7.5s | 180s |
| fps 1.0 | 163 | 15s | 180s |
| fps 0.5 | 82 | 30s | 180s |

**2. Visual descriptions are not independent observations.**
`build_prompt` (`vtt-core/src/vision.rs:291-299`) appends the transcript with the
instruction *"Use this transcript as context to enrich your visual description. Note
how the visual content relates to what is being said or heard."*

Measured rate of visual segments citing the audio:

| arm | citing audio |
|-----|--------------|
| fps 2.0 | 320/326 (98%) |
| fps 1.0 | 158/163 (97%) |
| fps 0.5 | 79/82 (96%) |

Consumers that treat speech and visual segments as independent evidence are double
-counting one source. Descriptions assert visually-observed events
("the price nearing and subsequently falling through these key resistance levels")
that may be restatements of the audio rather than readings of the frames.

**Aggravating factor.** `build_prompt` is called once at `vision.rs:145`, *outside*
the batch loop, so all batches in a chunk share one prompt. Per-batch windowing is
impossible without moving that call. The batch time bounds already exist
(`vision.rs:157-161`), so the data needed for windowing is present.

## Scope

Two independent, independently-toggleable changes. Defaults preserve current
behaviour so existing cached timelines stay reproducible.

### Fix 1 — per-batch causal transcript windowing

- New `TranscriptWindow` enum in `vtt-core/src/config.rs`:
  - `Full` — whole chunk (current behaviour, default)
  - `Concurrent` — speech overlapping `[batch_start, batch_end)`; bounds leakage to
    one segment span instead of 180s
  - `Causal` — speech with `end <= batch_start`; zero look-ahead
- `describe_chunk` signature changes from `transcript: Option<&str>` to
  `whisper_segments: &[Segment]`, so windowing can be applied per batch.
- Move `build_prompt` (`vision.rs:145`) **inside** the batch loop.
- New helper in `vtt-core/src/vision.rs`:

```rust
fn transcript_for_window(
    segments: &[Segment],
    batch_start: f64,
    batch_end: f64,
    mode: TranscriptWindow,
) -> Option<String>
```

  Uses `types::parse_timestamp` to resolve `Segment::start` / `Segment::end`.
  Returns `None` when the window is empty (e.g. `Causal` on batch 0).

Resulting leakage:

| mode | look-ahead | note |
|------|-----------|------|
| `Full` | 180s | current |
| `Concurrent` | one segment span (7.5–60s) | scales with `fps` |
| `Causal` | 0 | batch 0 gets no transcript |

### Fix 2 — independent visual observation

- New `vision.use_transcript: bool` (default `true`).
- When `false`, `build_prompt` omits the transcript block entirely
  (`vision.rs:291-299`) — not merely an empty string, so the instruction to
  cross-reference audio is gone too.
- `build_chunk_context` (`pipeline.rs:238`) also injects speech via its
  `Recent dialogue:` line. Gate that on the same flag, while **keeping** the
  `Last visual:` line so cross-chunk visual continuity survives.

With `use_transcript = false`, the visual track becomes a genuine independent
observation of the frames, and `TranscriptWindow` becomes irrelevant.

## Config

```toml
[vision]
transcript_window = "full"   # "full" | "concurrent" | "causal"
use_transcript    = true     # false => vision never sees speech
```

Recommended for ML feature extraction:

```toml
[vision]
use_transcript = false       # independent observation
# transcript_window unused when use_transcript = false
```

Recommended for human-readable transcripts: leave both at defaults.

A `profiles/ml.toml` preset should ship alongside the existing `charts.toml`.

## Dependencies

PR-003 (chunking), PR-005 (vision pipeline), PR-017 (config profiles).

## Verification Criteria

Unit tests in `vtt-core/src/vision.rs`:

- `transcript_for_window` in `Causal` mode returns only segments ending at or before
  `batch_start`; empty window yields `None`.
- `transcript_for_window` in `Concurrent` mode returns segments overlapping the
  window and excludes segments starting after `batch_end`.
- `transcript_for_window` in `Full` mode matches current `extract_transcript` output.
- `build_prompt` with `use_transcript = false` contains neither the transcript text
  nor the "Note how the visual content relates" instruction.
- **Regression guard for the once-outside-loop defect:** for a chunk with >1 batch
  and mode != `Full`, assert the prompts for batch 0 and batch N differ. This fails
  against current `main` and is the test that pins the fix.

Unit tests in `vtt-core/src/pipeline.rs`:

- `build_chunk_context` with `use_transcript = false` omits `Recent dialogue:` and
  retains `Last visual:`.

Integration (`#[ignore]`, needs Ollama):

- Same chunk processed with `use_transcript = true` vs `false` produces visual
  content whose audio-citation rate drops from ~98% to near zero. Reuse the detector
  regex from the analysis: `audio transcript|the speaker|voiceover|commentary`.

## Migration Impact

Cache keys are `sha256(realpath)[:8]` and do not encode profile settings, so a
re-run requires `force: true` or a cache purge, and cached results produced under
different settings are **not** distinguishable after the fact.

All 70 currently cached timelines were produced under `Full` + `use_transcript=true`.
Mixing them with newly-generated timelines yields a corpus with inconsistent leakage
characteristics — the worst outcome for a training set. For ML use the corpus must be
re-run wholesale under one setting.

Re-run cost at the measured 1.949 s/frame (2% spread across a 4x fps range):

| fps | corpus (38 videos, 43 min mean) |
|-----|--------------------------------|
| 2.0 | 104 h |
| 1.0 | 52 h |
| 0.5 | 26.5 h |
| 0.25 | 13.3 h |

Neither fix changes frame count, so neither changes runtime.

## Non-Goals

- Deriving tradeable price levels from vision output. Extracted chart numbers are
  unvalidated: fib→price pairs are internally inconsistent (0.618 maps to 28276,
  45022 and 48475 in one transcript), and at least one description narrates cursor
  scrubbing as price action ("high of $48,323 to a low of $34,530 and then back up to
  $46,105" inside 15s). Use exchange OHLC data for prices.
- Changing `chunk_duration_secs`. Reducing it would shrink `Full`-mode leakage but
  also weakens cross-chunk continuity and raises whisper model-load overhead
  (~42s per job, amortised over fewer chunks). `Causal` mode addresses the leak
  directly and at zero runtime cost.
