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

_Design-session state assessment below (`PROCEDURE-design-planning.md` Phase 1, run with the full
`PROCEDURE-pr-research.md` Phase 1 shape). `PROCEDURE-pr-research.md` still runs in full after the design
session and before implementation; its Phase 1 re-assesses state at that time._

### Design-session State Assessment (2026-08-24)

Read-only sweep: repo at `8ee6c82`, laptop corpus + cache, desktop (`ssh-desktop`), and the previous
session's harness records. No GPU time used, nothing modified.

**Current state**:

- **Code surface `vision.fps` touches — four files, the whole of it.** `config.rs:217` `fps: f32`,
  default 2.0 (`:363`), validated `> 0` with the sentinel message (`:445-452`). `ffmpeg.rs:270-297`
  `build_frames_command`: `-ss S -to E -i V -vf fps=N -q:v Q frame_%06d.jpg` — **no scaling**,
  native-resolution JPEGs go to Ollama; unchanged since the initial commit `87820ae`, no prior-art
  fixes to carry. `pipeline.rs:154` passes fps into `describe_chunk`. `vision.rs:148-163`
  `seconds_per_frame = 1/fps`, `batch_start/end` computed arithmetically (the draft cites 161-165;
  it is 157-163).
- **Pipeline shape.** `prepare_chunks` extracts audio + frames for *all* chunks up front, before any
  Whisper/Vision. `describe_chunk` batches by `max_frames_per_request` (15) → one Ollama call per
  batch with a bare `images: Vec<String>` — no per-image timestamp is expressible through this API.
  `batch_start/end` also drive `transcript_for_window` (`vision.rs:158-163`): **timestamp correctness
  is the mechanism that enforces Corpus Look-Ahead Freedom** when `use_transcript = true`. Inert for
  the locked profile (`false`); must hold for every other profile.
- **Config mechanism.** Profile load = base → `toml::to_string` → deep-merge `toml::Value` →
  `try_into::<ServerConfig>` (`config.rs:552-570, 595`). Any new `fps` representation must
  **round-trip through TOML serialisation**, not merely deserialise. `fps = "auto"` today fails at
  `try_into` → HTTP 400 "failed to apply profile". Validation runs at submission (`main.rs:212-222`).
  The client has no vision-fps override (only `ytdlp.max_fps`) — this is a server-side change only.
- **Data shapes.** `Segment {type, start "HH:MM:SS.mmm", end, content}`. Checkpoint v1 =
  `Vec<Segment>` per chunk (`CHECKPOINT_VERSION` bump if the schema changes). `timeline.json` =
  `{source, duration_seconds, segments}`; client `meta.json` = `{duration_seconds, hash, processed_at,
  segment_count, source, title, url}`. **No fps, profile, model, or prompt is recorded in any output.**
- **Prompt surfaces asserting uniform spacing** (none listed in the draft): `prompts/vision.txt:27-28`
  ("between 1 and 30 video frames per request … extracted at regular intervals (typically 2fps)") —
  this is the prompt in use (`ollama.prompt_template_path` default; desktop copy identical to HEAD);
  `ollama.default_prompt` in the desktop `server.toml` ("regular intervals"); `prompts/format.txt:32`
  ("visual segments cover broader windows (~7-15 seconds)").
- **Corpus.** 74 videos, **45.36 h** (macOS `mdls`, all 74; mean 36.7 min, range 17.2–73.8 min).
  Three resolution classes: 59 × 1920×1080, 9 × 1280×720, 6 × 640×360. Both Phase 5.5 / cost-model
  videos (`2024_2_12`, `2024_4_8`) are 1080p. Not staged on the desktop (`/home/rux/vtt-corpus`
  absent). The laptop has no `ffmpeg`/`ffprobe`.
- **36 of 74 videos already have cache timelines** (`~/.vid-to-text/cache`, processed 2026-03-30/31)
  under the pre-PR-020 config: fps 2.0 (7.5 s spans), 98% of visual segments cite the narrator
  (transcript-conditioned, full look-ahead), pre-beam-5. They violate the constraints PR-020 locked.
  `stage-videos.sh` defines "pending" as *no cache entry*, so the runner as written skips them.
- **Phase 5.5 raw scene data exists only in the desktop's `/tmp`**: `/tmp/scenes.txt` (clip900,
  16,803 rows), `/tmp/scenes_2024_2_12.txt` (2,653), `/tmp/scenes_2024_4_8.txt` (3,388); format
  `pts_time:X<TAB>scene_score=Y`, floor 0.004. Desktop up since 2026-08-23; a reboot deletes the only
  copy. User declined preservation on 2026-08-24 — recorded, not actioned.
- **Phase 5.5 arm outputs** are on the desktop: `~/.vid-to-text/server/results/{381b1eed (fps 0.25:
  15 visual × 60 s), … (0.5: 30 × 30 s), 0235436a (1.0: 60 × 15 s)}/timeline.json`; `clip900.mp4` at
  `/home/rux/vtt-exp/`.
- **Deployment.** systemd `vtt-server` active + enabled; binary built 2026-08-24 21:19:16 from
  `~/vid-to-text`, which is **not a git checkout** (source arrives by file copy; `docs/`, `prs/` there
  date from March). Seven files differ textually from HEAD; excluding tests and comments the only
  differences are the port default (3000 vs 3001, masked by `listen_port = 3001`) and one log string.
  Binary strings confirm PR-020 + `bf3c026` code. **The running server is functionally HEAD.**
  Deployed `market-research.toml` == repo (sha `53b73588…`). The desktop also carries 11 untracked
  `exp-*`/`ml`/`charts` profiles. GPU idle; 1.3 T free; 22 job dirs / 5.7 G in `/tmp/vtt-jobs`.
- **Environment versions.** ffmpeg **4.4.2** (2021) — has `select` (with `scene`), `scdet`,
  `showinfo`, `mpdecimate`, `freezedetect`. Ollama 0.18.3; `qwen3-vl:8b-instruct-q8_0` 8.8B Q8_0,
  context length 262,144, capabilities completion/vision/tools; `server.toml` sets `num_ctx = 65536`.
- **Measured per-frame vision cost** (every `[timing] chunk_N vision:` line in all server logs, 1080p,
  15-frame batches):

  | frames/chunk (≈ fps × 180 s) | chunks | s/frame mean | min | max |
  |---|---|---|---|---|
  | 45 (0.25) | 18 | 2.128 | 1.73 | 3.12 |
  | 90 (0.5) | 48 | 2.282 | 1.65 | 3.67 |
  | 180 (1.0) | 32 | 1.950 | 0.91 | 2.61 |
  | 360 (2.0) | 25 | 1.901 | 1.65 | 2.26 |

  PR-020's "1.949 s/frame, ~2% spread" was one arm. Cost has a per-chunk/per-batch fixed component;
  it is not linear in frame count alone.
- **Tests:** 173 pass, 13 ignored (need ffmpeg/whisper/ollama/yt-dlp — cannot run on the laptop).
  Workspace version 0.1.0, no tags, `docs/VERSIONING.md` bump rules unset, `CHANGELOG [Unreleased]`
  empty (PR-019/PR-020 landed without entries).
- **PR-020's Tier-B Q2 result was never appended to PR-020.** The harness record exists only at
  `~/.claude/projects/-Users-maxrux/f8ebcfe4-…/workflows/wf_e8dec556-5b5.json` (24 agents, 878k
  tokens, 3 of 8 claims survived, all from three arXiv preprints). What the summaries elide:
  - "All four models tested" are **2B–3B** (Qwen2.5-VL-3B 60.9 vs MaxInfo 57.5 / CSTA 57.9; SmolVLM2;
    InternVL3-2B; Ovis2-2B), one paper (arXiv:2509.14769), `Nmax=96`. Harness caveat: *"Do not assume
    the uniform-fps advantage holds at frontier model scale."*
  - Finding 6: at long durations fps is inert because the frame cap binds — *"the real design
    decision is the frame budget and its placement, not the sampling rate."*
  - Caveats: **"LOAD-BEARING GAP, ESCALATION RECOMMENDED"** — zero verified evidence on
    static/screencast content; Tier C targeted at lecture/slide/screencast/GUI-agent literature
    recommended and **never run**.
  - Open question 4 of that run: *"Does a hybrid strategy (uniform floor plus transition-triggered
    extra frames) beat either pure strategy? … appears untested."*
  - Short-event recall (uniform 11.81% / TransNet 46.28% / InfoShot 84.70% at matched 0.5 fps) rated
    **low** confidence: synthetic benchmark authored by the winning method's team.
  - Both saturation claims and both "uniform oversamples static" claims were refuted 0-2.

**Assumptions at PR draft time**:
- Corpus cost is 13.3–26.5 h at fps 0.25–0.5 (from PR-020's "38 videos" table).
- Cost is linear at 1.949 s/frame, ±2%.
- Static gaps run "up to 577 s"; the floor question is about ~3 silent chunks.
- The known hazard is "adaptive lost across all four models tested".
- `fps` is the axis being redesigned.
- Config typing needs a custom deserializer.
- No prompt or provenance work is implied.

**Stale assumptions**:
1. **"38 videos" is the pending count, not the corpus.** All 74 must run (the 36 March timelines are
   non-conforming). Re-derived for 74 with the measured per-shape s/frame: fps 2.0 ≈ **173 h**,
   1.0 ≈ **88 h**, 0.5 ≈ **52 h**, 0.25 ≈ **24 h** (derived, not measured end-to-end).
2. **The scene statistics reproduce exactly from the raw data** (97.3% < 0.02; 5/33/72.1 s/31.3 s;
   1/36/48.0 s/10.6 s) — but **"gaps up to 577 s" is the *mean* gap at >0.30 on `2024_2_12`.** The
   *max* gap at >0.30 is **1,986 s** (33 min ≈ eleven consecutive chunks); at >0.10 it is 397 s
   (`2024_2_12`) and 539 s (`2024_4_8`). The floor question is harsher than stated.
3. **The threshold cliff is steep and the "meaningful" band was chosen by eye.** >0.05 yields 289 /
   498 events (median gap 0.6 s / 0.2 s — cursor bursts); >0.10 yields 33 / 36. Open question 4 lives
   in a band between 0.05 and 0.10.
4. **The hazard is narrower than summarised**: small-model scale, natural video, with the
   domain-specific escalation the harness itself asked for never run.
5. **fps may be the wrong axis**: the surviving evidence says frame budget and placement is the
   decision (Q2 Finding 6).
6. **Cost is per-batch, not per-frame** (table above); open question 6 must be answered in those terms.
7. **Config typing must survive the profile round-trip**, not just deserialise.
8. **Three prompt surfaces assert regular intervals** and would become false under adaptive sampling.
9. **Outputs carry no capture provenance.** Fixed fps is reconstructible from segment spans; adaptive
   selection is data-dependent and irreproducible without recorded parameters.

**New constraints**:
- Real frame timestamps are the look-ahead enforcement mechanism (`transcript_for_window`), not a
  labelling nicety.
- Any new `vision.fps` representation must round-trip `toml::to_string` → merge → `try_into`, and
  every existing numeric profile (`p55-*`, `ml`, `charts`, `exp-*`) must still load.
- Ollama's images API cannot carry per-frame time; any temporal cue reaches the model only as prompt
  text.
- Frames are sent at native resolution and the corpus has three resolution classes → the token-budget
  ceiling (open question 3) is resolution-coupled.
- ffmpeg 4.4.2's filter surface is the bound; Phase 2 must check filter semantics against 4.4 docs.
- `docs/0.0/DESIGN-log.md` decision 2 explicitly deferred scene-based *chunking*; this PR is sampling
  *within* fixed chunks and must not silently reopen chunking (already excluded in Scope).
- Adaptive-extraction tests will be `#[ignore]` on the laptop; verification runs on the desktop.
- `docs/ARCHITECTURE.md` has **no frame-extraction section** (Data Flow says only "ffmpeg: chunk
  video"); the section this PR claims to implement must be created in design Phase 4. (Also stale
  there: Storage says config lives at `~/.config/vid-to-text/`; code uses `~/.vid-to-text/config`.)
- `CLAUDE.md` § Remote server access: "repo is also checked out at `~/vid-to-text/`" is stale — it is
  an unversioned file copy. Adds to the systemd staleness already recorded in
  `docs/0.0/RESEARCH-BACKLOG.md`.

**Downstream contracts**:
- **none** — no PR depends on PR-022 (verified via `grep -rn "PR-022" prs/ docs/`: only PR-020 as
  creator, `docs/ARCHITECTURE.md:88`, `docs/0.0/ROADMAP.md:70`, `docs/0.0/RESEARCH-BACKLOG.md:66`).
- **Upstream contract from PR-020 → this PR:** (a) decide the sampling mechanism and replace the
  `fps = 0.0` sentinel; (b) calibrate `repetition_report_thold` (deferred; the three clean arms gave
  zero flags, so calibration needs known-bad material).
- **Implicit, undeclared consumers:** PR-021 (reads visual descriptions + their timestamps); the
  `format` command (`prompts/format.txt:32` spacing assumption).

**Path-tier checkpoint:** header Tier-2; `docs/0.0/RESEARCH-BACKLOG.md` Tier 2 `design-research ~`;
`docs/0.0/ROADMAP.md` `[ ]` dep PR-020. Consistent; design-planning-first stated in all three. No
re-tier.

**Untracked, outside this PR, but gating the run it unblocks** (recorded so they are not lost):
- The 36 non-conforming March timelines and the runner's "pending = no cache entry" skip logic.
- Corpus provenance: 22 coverage gaps of unknown cause; undefined unit of observation. No PR tracks
  either.
- Branch `pr-019-vibe-rails-sync` carries PR-020/021/022.

**Inputs Phase 2 should start from rather than rediscover:** the Q2 harness already scoped the missing
research (lecture / slide / screencast / GUI-agent literature, Tier C) and named the untested hybrid
(uniform floor + transition-triggered supplement).


### PR Research Procedure (2026-08-24, run after the design session)

**Phase 1 — State Assessment: re-confirmed, no drift.** The design-session assessment above was taken
the same day at `8ee6c82`; the tree is unchanged apart from the design docs. Path-tier Tier-2
confirmed (header, backlog, roadmap agree). Downstream contracts: none (grep sweep); upstream
contract from PR-020 (replace the sentinel; calibrate the repetition unit) is satisfied by decisions
1 and 9.

**Phase 2 — Research Questions.** Every must-answer question was scoped and run inside the design
session's Phase 2 (`docs/0.0/DESIGN-log.md` session 2026-08-24). Listed here with the tier that
resolved each and the status label the implementation inherits:

| # | question | tier run | status |
|---|---|---|---|
| Q1 | Is a hybrid floor + trigger defensible for static screen content? | A → **C** (`wf_bef168b0-50b`) | best-guess-given-constraints for *quality*; mechanism proven; fixed mode retained |
| Q2 | Which change signal and threshold separate content change from cursor motion? | A (FFmpeg source) + corpus sweep + visual inspection | proven signal semantics; threshold measured on this corpus; no literature value exists |
| Q3 | What floor and ceiling bound cost, in which units? | A (Gemini docs, Video-MMLU via C) + measured tokens | proven token formula; values measured |
| Q4 | Do ffmpeg 4.4.2 `select`/`metadata`/`-vsync`/`prev_selected_t` behave as required? | A (`man` on the deployed host) + empirical | proven |
| Q5 | Can per-frame time labels in prompt text stand in for video-mode timestamps? | A (HF processor source, Ollama renderer) + direct test | proven |
| Q6 | Config representation | internal — no web round | decided (nested struct; `"auto"` rejected) |
| Q7 | Provenance in output | internal — no web round | decided |
| Q8 | Repetition unit | internal, backed by PR-020 Group D + corpus re-scoring | proven |

Excluded (nice-to-have): frame downscaling; hybrid-vs-uniform quality A/B (no metric); scene-based
chunking.

**Phase 3 — Run the Research.** Executed in the design session; sources and disconfirming evidence are
in `docs/0.0/DESIGN-log.md`. Cite-or-flag: every config key, filter option and formula in this PR is
tied to a verified source or a measurement on the deployed stack; the one unverifiable claim (hybrid
quality) is labelled and carried, not asserted. Consumer cross-check: no downstream PR; the implicit
consumers (`format` command, PR-021) receive strictly additive output.

**Phase 4 — Synthesis. Outcome: Confirm.** Nothing in the research contradicts the design; two
amendments to the *draft* were made during design (config shape; provenance added) and are recorded
in Scope.

**Phase 5 — Gate Check.**
- Premise still valid: ✓ (the corpus is measured ~91% redundant at 2 fps; fixed-fps stays available)
- No prerequisite PRs surfaced: ✓ (the 36 stale timelines and runner logic gate the *run*, not this PR;
  recorded in `docs/0.0/RESEARCH-BACKLOG.md`)
- Scope changes since draft: config shape (nested table, not `"auto"`); provenance; token pre-flight;
  window-scored repetition; prompt timing block — all in Scope above
- Implementation surface: `vtt-core/src/{config,ffmpeg,types,vision,pipeline,whisper,checkpoint}.rs`,
  `prompts/vision.txt`, `config/profiles/market-research.toml`, `vtt-client/src/format.rs` (literal),
  tests throughout
- Risks accepted: onset-only detection (bounded by floor); hybrid quality unmeasured (bounded by fixed
  mode); Tier C harness overran its token target 3x (1.58M vs ~500k) — operator to review the tier
  scripts' budget enforcement
- User approved updated spec: **waived by the user for this PR (2026-08-24)**
- Implementation cleared: ✓

### Implementation Validation (2026-08-25)

**Tests.** 202 pass locally (18 + 181 + 3), 12 ignored; the 5 ffmpeg-dependent ignored tests pass on
the desktop against ffmpeg 4.4.2 (extract with real pts, prepare_chunks end-to-end). Zero warnings in
`vtt-core`. Red-then-green: the first run of the new tests failed on four points, two of which were
real findings — `tokens_per_frame` needed round-half-to-even per axis (Qwen `smart_resize`), not
ceil, to reproduce the measured 882 tokens at 720p; and 30 frames × 2,042 + 4,096 reserve = 65,356
*fits* 65,536, so the design text's "thirty would overflow" was corrected to "at most 30 fit".

**Corpus sweep, all 74 videos** (326,663 candidates at 2 fps, 945 chunks; `~/vtt-scenes/` on the
desktop): 82.6% of candidates score < 0.005, 88.9% < 0.01. At T 0.08 / G 15 s / R 2 s: 15.4 frames
per chunk (p50 15, p95 22, max 29) — the cap of 45 never binds; 2.14 triggers/min after the refractory
(raw 2.6/min, 17% clustered), per-video spread 0.71–4.77/min; corpus total ≈ 14,600 frames (versus
326,663 at 2 fps and ~40,800 at 0.25 fps).

**End-to-end, market-research profile, deployed server (`8ee6c82` + this PR):**

| run | video | frames kept | visual segs | frames/seg | wall | realtime |
|---|---|---|---|---|---|---|
| `f7ba8b47` | clip900 (900 s, 1080p) | 87 (vs 1,800 at fps 2.0) | 9 | 8–15, spans 66–180 s | 312 s | **0.35x** |
| `b73bdbf8` | `2024_4_8` (1,854 s, 1080p) | 169 | 17 | 4–15, spans 57–180 s | 629 s | **0.34x** |

Checks on both outputs: every visual segment carries `frames` with real, non-uniform timestamps
(gaps 0.5–15.0 s; the maximum gap over the whole 31-minute video is exactly 15.0 s, so the floor holds
across chunk boundaries; 0.5 s gaps occur only at chunk boundaries, where the first frame is always
kept); segments are contiguous and non-overlapping; `capture` records `sampling: adaptive`,
`use_transcript: false`, `transcript_window: causal`; the model's narration cites the time labels
("captured at irregular intervals between 00:01:14.000 and 00:02:55.500"); mean description length
594–623 words per segment, the same as the fixed fps-0.25 arm (600); `[preflight]` logged
34,726 of 65,536 context tokens for a full request; no `[frames]` capping and no `[repetition]` flags.

**Corpus projection:** 45.36 h × 0.34 ≈ **15.5 h**, versus ~24 h for the fps-0.25 arm at equal
request count. Cost is generation-dominated once frames are informative, so it tracks requests per
chunk (1–3), not frame count; the design's earlier "typical case ≈ 1/3 of the cap" was a frame-count
extrapolation and is withdrawn in favour of these measurements.

**A defect the provenance block caught.** The first validation run reported `use_transcript: true,
transcript_window: "full"`: the profile rewrite had placed the `[vision.adaptive]` table above the
`use_transcript` / `transcript_window` lines, so TOML re-homed those two locked values under the
adaptive table, where unknown keys are ignored. Fixed by ordering, and pinned by
`test_market_research_profile_locked_values_survive_table_layout`, which loads the repo profile
through the real merge path and asserts every locked value.

**Also changed during implementation, recorded per the design rule:** adaptive mode balances batch
sizes (16 frames → 8 + 8, not 15 + 1) so no segment is a single frame standing for four seconds;
fixed mode keeps the legacy split so its output stays byte-identical.

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

Decided in the design session of 2026-08-24 (`docs/0.0/DESIGN-log.md`); the six open questions in the
draft are answered there, decisions 1–9.

**Mechanism — hybrid sampling within fixed chunks.** At the candidate rate `vision.fps`, keep a frame
when it is the first of the chunk, or `max_gap_secs` have passed since the last kept frame (floor), or
its ffmpeg `scene` score exceeds `scene_threshold` and `min_trigger_interval_secs` have passed since the
last kept frame (trigger). If a chunk exceeds `max_frames_per_chunk`, drop the lowest-scoring
triggers, never floor frames. Fixed-fps mode is unchanged and remains the default.

**Config surface** (`[vision.adaptive]`, all values from TOML; `vision.fps` keeps its type):

```toml
[vision]
fps = 2.0                          # adaptive: candidate rate; fixed: sample rate (unchanged meaning)
[vision.adaptive]
enabled = true                     # default false — every existing numeric profile is unaffected
scene_threshold = 0.08             # ffmpeg select `scene` score, 0..1
max_gap_secs = 15.0                # floor: at least one frame every N seconds
min_trigger_interval_secs = 2.0    # refractory: de-cluster bursts
max_frames_per_chunk = 45          # ceiling; must be >= the floor count for the chunk length
[ollama]
prompt_reserve_tokens = 4096       # pre-flight: frames x tokens_per_frame + reserve <= num_ctx
[whisper]
repetition_window_secs = 30.0      # repetition report unit (was per-segment)
```

The draft's `fps = "auto"` is rejected — it hides three parameters and needs a tagged type that must
survive the profile merge's TOML round-trip; a nested struct does not.

**Also in scope, because the mechanism requires them:**
- Real frame timestamps from ffmpeg `metadata=print` (stderr) in both modes, both under `-vsync vfr`;
  count mismatch fails the chunk. `describe_chunk` takes `FrameSample`s, not paths + fps.
- Per-frame capture times in the prompt; `prompts/vision.txt` stops asserting regular intervals.
- Pre-flight per-request token check from the probed resolution (measured 2,042 tokens per 1080p
  frame; at most 30 fit beside the prompt reserve, and more would silently overflow `num_ctx`).
- Provenance: `Timeline.capture` and per-visual-segment `frames`, both optional.
- `repetition_report` scores 30 s windows (carried here from PR-020's deferral).
- The market-research profile sets the values above and drops the `0.0` sentinel.

**Explicitly out of scope:**
- Changing `chunk_duration_secs` or scene-based *chunking* (deferred since the 2026-03-28 session).
- Any whisper decoding change. The speech track is unaffected.
- Re-opening the locked dimensions from PR-020.
- Frame downscaling (would cut per-frame cost ~57% at 720p; legibility unmeasured; not a PR-022
  dimension).
- A hybrid-vs-uniform quality comparison — no metric can score description quality across sampling
  regimes (PR-020 Phase 5.5 Finding 1). Recorded as a gap, not faked.

## Dependencies

- **PR-020** — locks every other capture dimension and ships `vision.fps` unset behind a validation
  sentinel, so this PR fills a hole that is already explicitly marked rather than overriding a value.

## Architecture section implemented

`docs/ARCHITECTURE.md` — the frame-extraction stage. This changes the pipeline's sampling mechanism
and breaks the uniform-spacing invariant that visual timestamps currently rely on, so it is a
structural change.

## Verification criteria

- [x] Visual segment timestamps derive from ffmpeg `pts_time`, not arithmetic; a test pins non-uniform
      spacing, and a second test pins that uniform spacing reproduces the previous arithmetic exactly
- [x] Frame-file count ≠ metadata count fails the chunk with an error naming both counts
- [x] A chunk with zero triggers still yields floor frames (first frame + every `max_gap_secs`)
- [x] Bursts are de-clustered: no two triggers closer than `min_trigger_interval_secs`
- [x] A chunk exceeding `max_frames_per_chunk` is thinned to the cap, dropping lowest-scoring triggers
      and never a floor frame; a cap below the floor count is rejected at validation
- [x] `scene_threshold` outside (0,1), non-positive gaps, or `max_gap_secs × fps < 1` are rejected
- [x] Per-request token pre-flight rejects `max_frames_per_request × tokens_per_frame + reserve > num_ctx`
      before any GPU work, with an actionable message
- [x] Existing numeric-fps profiles load unchanged; `[vision.adaptive]` absent ⇒ disabled; the profile
      merge round-trips the nested table
- [x] The prompt lists per-frame capture times; `prompts/vision.txt` contains no fixed-interval claim
- [x] `Timeline.capture` and visual `frames` serialise when present and are omitted when absent; an old
      timeline / checkpoint without them deserialises
- [x] `repetition_report` flags a loop spread across short segments (per-window) that per-segment
      scoring misses; the false-positive surface is documented
- [x] Deployed and run end-to-end on the desktop: clip900 under the market-research profile produces
      real-PTS visual segments with frame counts within [floor, cap]; one full corpus video measures
      wall time against the predicted bound
- [x] `cargo test --workspace` passes; ffmpeg-dependent `#[ignore]` tests pass on the desktop

## Research backing

Tier-2. Design-session research (Tier A probes + Tier C harness `wf_bef168b0-50b`) is recorded in
`docs/0.0/DESIGN-log.md` session 2026-08-24 with per-decision status:

| decision | status |
|---|---|
| ffmpeg 4.4.2 select/metadata/vsync behaviour, PTS derivation | **proven** (empirical + source) |
| tokens per frame on the deployed stack (2,042 / 882 / 222) | **proven** (measured) |
| per-frame time labels in prompt text | **proven** (HF processor source + Ollama renderer + direct test) |
| corpus redundancy (~91% unchanged at 2 fps) and change-rate spread | **proven** (corpus sweep) |
| threshold / floor / refractory / cap values | **measured on this corpus**; no literature values exist |
| hybrid beats uniform at equal budget on static screen content | **best-guess-given-constraints** — literature split (GUI-World small adaptive gain; Li & Shi indistinguishable); no scoring metric; fixed mode retained |
| repetition unit = 30 s window | **proven** (OpenAI's unit, PR-020 Group D; two real loops recovered here) |

The draft's four candidate questions: (1) threshold — no defensible literature value; measured here,
fixed absolute. (2) adaptive vs uniform on static content — unmeasured in the literature; split on GUI
content; recorded as the PR's residual uncertainty. (3) production cost bounding — fixed frame budgets
everywhere (Gemini 1 fps floor / 258 tokens per frame; GUI-World 10 per request; Brkic 96 per video);
adopted as floor + per-chunk cap + per-request token check. (4) is scene score the right signal for
small-region chart changes — **no**, by construction (mean luma change per pixel); the floor covers
them and this is documented as a limitation rather than solved.

**Known hazard, restated precisely.** The uniform-wins result (Brkic) is at 2–3B model scale on
natural video and exceeds standard error for one of four models; the harness that produced it
recommended the Tier C run above, which found the screen-content record split. This PR does not claim
adaptive is better; it claims the corpus is measured to be ~91% redundant at 2 fps, that every
persisted state is captured within `max_gap_secs` by construction, and that the choice is reversible.

## Notes

- The measurement that motivates this PR came from a user domain observation ("charts don't change
  often while the narrator talks about them"), which was then tested rather than accepted. It held,
  and more strongly than stated.
- Scene detection is essentially free — ffmpeg computed it over two full videos in ~2 minutes with no
  GPU. Whatever the outcome, the *measurement* is cheap enough to run over the whole corpus to
  characterise the change-rate distribution beyond n=2, which should probably happen during design.
