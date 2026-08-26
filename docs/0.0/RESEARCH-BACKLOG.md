# Research Backlog

Index of PRs with research status. Every PR runs `PROCEDURE-pr-research.md` before implementation — this document tracks the state of each PR's research and flags drift risk per the time-decay policy.

**Tiers (PR-path):**
- **Tier 1** — Design-time research exists. Phase 1 (State Assessment) required before implementation; Phases 2-4 may be light if no drift found.
- **Tier 2** — Design-time research is partial or absent. Full procedure required before the PR can be written in final form.

**Status legend:**
- `design-research ✓` — research done at design time
- `design-research ~` — partial design research
- `design-research ✗` — no design-time research
- `state-assessed YYYY-MM-DD` — Phase 1 completed
- `fully-researched YYYY-MM-DD` — all 5 phases completed
- `implementation-cleared YYYY-MM-DD` — Phase 5 Gate Check passed

---

## Pre-procedure PRs (landed before `PROCEDURE-pr-research.md` existed)

These shipped under an older `vibe-rails` revision that had no PR-research gate. They are recorded for completeness. **The gate applies going forward; these are not retroactively backfilled** (explicit scope decision in PR-019).

| PR | Description | Status | File |
|----|-------------|--------|------|
| PR-001 | Project skeleton | landed v0.0 | ✓ |
| PR-002 | Config system | landed v0.0 | ✓ |
| PR-003 | Video chunking | landed v0.0 | ✓ |
| PR-004 | Whisper pipeline | landed v0.0 | ✓ |
| PR-005 | Vision pipeline | landed v0.0 | ✓ |
| PR-006 | Timeline merge | landed v0.0 | ✓ |
| PR-007 | Server API | landed v0.0 | ✓ |
| PR-008 | Client and transfer | landed v0.0 | ✓ |
| PR-010 | Checkpointing | landed v0.0 | ✓ |

## Index gaps (recorded, not fabricated)

Reconciling `prs/`, `docs/0.0/ROADMAP.md`, and git history surfaced inconsistencies. These are recorded as found. **No PR files were invented to fill them.**

| PR | In ROADMAP | File in `prs/` | In git history | Note |
|----|-----------|----------------|----------------|------|
| PR-009 | no | no | no | Number skipped. Referenced only as a dependency (`PR-008+009`) by PR-011's ROADMAP row. Never existed. |
| PR-011 | `[x]` | **no** | `ee31c77` (YouTube URL support) | Landed without a PR file. |
| PR-012 | `[x]` | **no** | — | Landed without a PR file (format command). |
| PR-013 | `[x]` | **no** | — | Landed without a PR file (granular visual segments, retry, num_ctx). |
| PR-014 | `[x]` | **no** | — | Landed without a PR file (overlapped Whisper/Vision). |
| PR-015 | `[~]` | **no** | — | Marked in-progress in ROADMAP; no file. Status unverified. |
| PR-016 | **no** | **no** | `a822525` (merge #19, `pr-016-cache-system`) | Landed but indexed nowhere. |
| PR-017 | **no** | **no** | `24a52c8` (merge #20, `pr-017-config-profiles`) | Landed but indexed nowhere. |

Closing these gaps is **not** in PR-019's scope ("one PR, one thing"). It is a candidate follow-up.

---

## Tier 1 — Implementation-Ready (pending state assessment)

| PR | Design research | State assessed | Implementation cleared |
|----|-----------------|----------------|------------------------|
| [PR-019](../../prs/PR-019-vibe-rails-sync.md) | `design-research ✓` (template is the project's own methodology SSOT) | `state-assessed 2026-08-24` | `implementation-cleared 2026-08-24` |
| [PR-023](../../prs/PR-023-visual-fidelity-metric.md) | `design-research ✓` (session 2026-08-25: CHOCOLATE typology; OCR engine measured on corpus) | `state-assessed 2026-08-25` | `implementation-cleared 2026-08-25` |
| [PR-024](../../prs/PR-024-ocr-grounded-vision-prompt.md) | `design-research ✓` (Tier A 2026-08-25: OCR-augmented VQA, both directions) | `state-assessed 2026-08-25` | `implementation-cleared 2026-08-25` |
| [PR-025](../../prs/PR-025-vision-degeneration-guard.md) | `design-research ✓` (in-repo prior art + threshold measured over 2,423 segments) | `state-assessed 2026-08-25` | `implementation-cleared 2026-08-25` |

## Tier 2 — Research-Pending

| PR | Design research | Required research topics |
|----|-----------------|--------------------------|
| [PR-018](../../prs/PR-018-causal-vision-context.md) | `design-research ✗` | **Non-conforming — see below.** Retrofit to `PR-TEMPLATE.md` and backfill research: (a) look-ahead contamination in LLM-derived corpora, (b) whether audio-conditioned visual descriptions are defensible for exploratory research use, (c) evaluation method for the fix. |
| [PR-020](../../prs/PR-020-market-research-capture-config.md) | **`implementation-cleared 2026-08-24`** — CLOSED | Complete through Phase 5.5. 8 must-answer questions: 5 at Tier A, 3 at Tier B. Group D run. Outcome Amend (5 amendments). Implementation reviewed, 2 defects found and fixed. **Phase 5.5: Confirm on all locked dimensions, ESCALATE on `vision.fps`** — ships unset behind a validation sentinel; mechanism escalated to PR-022. |
| [PR-022](../../prs/PR-022-content-adaptive-frame-sampling.md) | `design-research ✓` (session 2026-08-24, Tier A + Tier C) · `state-assessed 2026-08-24` · `fully-researched 2026-08-24` · `implementation-cleared 2026-08-24` | Content-adaptive frame sampling. Design session complete: hybrid floor + trigger + cap, real PTS, token pre-flight, provenance, window-scored repetition. Residual uncertainty recorded: hybrid-vs-uniform quality on static screen content is unmeasured in the literature and unmeasurable here (no metric); fixed mode retained. **Approval gates waived by the user for this PR (2026-08-24).** |
| [PR-029](../../prs/PR-029-fidelity-prices-non-numeric-fabrication.md) | `implemented 2026-08-26` | **Fidelity diagnostic prices non-numeric fabrication.** Step 1 of the Finish & Ship plan. `score_segments` extracts numbers and labels only, so text fabricating neither is unscored: 15,905 chars -> 25 facts, 16,644 -> 22, 13,701 -> 28, and removing 12,236 fabricated chars from `84149f3b` changed **no** fidelity number at all. On the 2026-08-25 live run 70% of generated visual text sat in two degenerate segments, worth 1.53 precision points (re-derived 2026-08-26; recorded as 1.6 until then). **Tier-2**: what counts as a scored fact has no in-repo answer, the choice moves every historical figure, and a migration/comparability rule is mandatory. Blocks PR-030 and PR-031, because no threshold or A/B validated against a blind metric is validated. |
| [PR-030](../../prs/PR-030-close-vision-prompt-shipping-decision.md) | `design-research ~` | **Close the vision-prompt shipping decision.** Step 2. PR-026 shipped v3.1 but left the decision OPEN; two of three cells are now filled (v3.1 leads v2 by +0.020 and +0.005 F0.5 on the shared clips, trading ~4 points of precision for ~3 of recall). One 8-minute run on `2025_05_26` completes the table. **Tier-1** — PR-026's Tier-2 round settled the design; Phase 1 must catch PR-029's deliberate metric drift. **Amended 2026-08-26 by user decision: the ranking was dropped.** PR-029's Phase 1 found that ranking prompt arms on fidelity is forbidden by `docs/ARCHITECTURE.md` § Review until κ is reported, and κ is blocked on the review-sheet interaction (re-opening it would resurrect the deleted PR-027). The prompt is now decided on the precision/recall trade and by reading the output, fidelity as guardrail only; no arm may be called more accurate than another. Two cautions recorded: the *general* prompt still has the best precision of any arm on `2024_6_24` (0.950 vs 0.863), and **v2's prompt file no longer exists** — only v1 and v3.1 were ever committed, so "revert to v2" currently has nothing to revert to. |
| [PR-031](../../prs/PR-031-word-run-degeneration-guard.md) | `design-research ~` | **Word-run degeneration guard.** Step 3. A fourth degeneration mode, found on the live job that deployed the third guard: 1,140 of one segment's 3,140 words are the token `light`. All three shipped guards are blind by construction — sentences unique, no numeric token, and PR-028's skeleton detector scores it **1** because the varying part is the repeat count itself. **The most prevalent mode found so far**: 423 of 11,108 segments carry a word repeated >=5x consecutively and 93 carry >=10x, against 3 in 11,108 for PR-028's mode. **Tier-1**; the tokenizer and threshold are the research, and the prevalence figures are a Python probe that must be re-derived with the shipped tokenizer. Also must answer whether it covers the word-slot templated ramp (mode 6). |
| [PR-032](../../prs/PR-032-config-fidelity-for-job-submission.md) | `implemented 2026-08-26` | **Config fidelity for job submission.** Prerequisite for PR-030, not a numbered step. A job can silently run under wrong sampling, wrong prompt and wrong transcript policy: `--profile` is accepted and never sent for local files (`vtt-client/src/main.rs:238`), the multipart handler `break`s on the `file` field and drops every later field including `profile` and `force` (`vtt-server/src/main.rs:460`), and `check_cancelled()` only runs after a full chunk's vision pass (`pipeline.rs:117/185/217`), so a cancelled job held the GPU for ~13 minutes. All three reproduced live on 2026-08-25. Matters because 68 corpus videos are pending capture. **Tier-1.** |
| [PR-021](../../prs/PR-021-vision-grounded-asr-correction.md) | `design-research ~` | Vision-grounded ASR correction — the documented-beneficial direction PR-020 surfaced. Partial basis from PR-020 Q3; needs its own full procedure. Follow-on to PR-020, not a prerequisite. |
| [PR-028](../../prs/PR-028-template-ramp-degeneration-guard.md) | **`fully-researched 2026-08-25`** · **`implementation-cleared 2026-08-25`** | **Repeated-skeleton degeneration guard** (filed as "template-ramp"; scope widened by research). Tier-1, but **all five phases ran** — Phase 1 was not clean. Findings: the specified detector (mask *numeric* tokens) **misses a known case** whose varying slot is circled glyphs `①②③`; amended to `char::is_numeric()` (Unicode `Nd\|Nl\|No`), verified against the Rust std documenter in Group D. Threshold measured over **11,108 guard-era visual segments** (94 timelines, deduplicated, 4 pre-guard excluded): legitimate max **13**, degenerate **143 / 267 / 878** — every cap in [14, 142] truncates the same segments, so the binding constraint is **head preservation** (levels OCR-supported through position 20), giving `max_skeleton_repeat = 24`. Verified end-to-end offline with the shipped scorer: case 1 precision **0.529 → 0.901**; case 2 **unchanged in every number** despite 12,236 chars removed, proving the diagnostic cannot see the non-numeric variant. Two further instances found that the PR did not know about, one of them a **fourth mode** (verbatim `"You're not."` ×878, under `truncate_repetition`'s 15-char gate) — covered via `min_skeleton_chars = 10`. Provenance correction: the second case was produced by prompt **v2**, not v3, so a prompt fix demonstrably does not prevent the mode. Outcome **Amend** (A1–A6), approved 2026-08-25. |
| [PR-026](../../prs/PR-026-content-specific-vision-prompt.md) | **`fully-researched 2026-08-25`** · **`implementation-cleared 2026-08-25`** | Content-specific vision prompt. Full Tier-2 path run: Phase 1, Phase 2 (Phase 6 parameters + tuning/held-out split fixed before any cycle), Phase 3 with **two Tier-C harness runs**, Phase 4 **Amend**. Group D waived by user direction. Settled: the presupposition problem is proven on Qwen2-VL (31–50 pp, ACL 2026, length-immune metrics, causal pathway isolated); the *removal* remedy is unmeasured anywhere and is labelled `best-guess-given-constraints`; prompt structure will not buy accuracy (PlotPick 1–3 pp; accuracy tracks legibility, converging with PR-024); primary measure locked (invented-number rate at 5% tolerance, joint with precision+recall at β=1, chronology reported separately). A Motivation error was corrected — the 94.1%/56.7% figures were two different metrics on two different subsets. |

### PR-018 non-conformance (recorded)

PR-018 was written **and implemented** on 2026-08-24 without the `Landed-in:`, `Before Implementation`, or `Research findings` sections, and without running `PROCEDURE-pr-research.md` at all. This violates the Research-Backed Decisions and PR Research Procedure Required constraints that PR-019 installs.

Its code is implemented and covered by 8 tests (transcript windowing modes, per-batch prompt regression guard, dialogue gating). The evaluation metrics used to validate it (duplicated 8-gram rate, audio-citation rate, recall stratified by on-screen persistence) were **invented ad hoc, not research-backed**, and are labeled `best-guess-given-constraints`.

Agreed handling: PR-018 supplies the *mechanism* (the config knobs); PR-020 researches and locks the *values*. PR-018 is retrofitted to the template and its research backfilled as its own work item.

---

## Drift Watch

Per the time-decay policy in `PROCEDURE-pr-research.md`, any PR marked `fully-researched` or `state-assessed` more than the staleness threshold before implementation begins must re-run Phase 1.

**Project staleness threshold:** 30 days (set in `docs/CONSTRAINTS.md` § Research Time-Decay).

**Currently watching:** none — PR-026 was researched and cleared the same day.

## Known documentation gaps

- **`docs/VERSIONING.md` bump rules are unset.** The file ships with `[Project-specific — ...]` placeholders for MAJOR/MINOR triggers and v1.0 criteria. Deferred by explicit user decision during PR-019 (2026-08-24). Until set, minor-vs-patch classification is undefined for this project.
- **No version tags exist.** Landed PRs are attributed `v0.0 (untagged)`. The first real cut should establish the tag per `docs/VERSIONING.md` §7.

- **`CLAUDE.md` § Remote server access is stale.** It states "the server is launched manually and foreground when needed — there is no systemd unit, launch script, or persistent log file." As of 2026-08-24 all three exist on the desktop: a `vtt-server.service` systemd unit (`Restart=always`, enabled at boot), `~/vid-to-text/start-vtt-server.sh`, and timestamped logs under `~/.vid-to-text/logs/`. These were installed while deploying an unrelated UTF-8 panic fix. Correcting this section is **out of PR-019's scope** ("one PR, one thing") and needs its own work item — it is a project-doc accuracy fix, not template scaffolding.

- ~~36 of the 74 corpus videos have pre-PR-020 cache timelines that the runner would skip~~ —
  **RESOLVED 2026-08-25.** The 36 timelines (and 9 matching server-side results) were archived to
  `~/Documents/seer_archive/pre-pr020-timelines-20260825.tgz` (sha256 recorded alongside; contents
  verified byte-for-byte before deletion) and removed from both machines. The tooling defect behind
  it was fixed in `~/Documents/seer_archive/bin/`: `stage-videos.sh` now decides what to transfer
  from **remote file presence and size** instead of the client cache (it was conflating transfer
  state with processing state), and `run-corpus.sh` skips a video only when its cached timeline
  records the **same profile** now being requested, reporting anything stale by name. Its default
  profile was also `exp-fps05` — an untracked experiment — and is now `market-research`. New helpers
  `_pending_stage.py`, `_pending_corpus.py`, `_store_result.py`; `meta.json` now records `profile`,
  `capture` and `fidelity`, and a capture superseded by a different profile is copied to
  `transcribe-runs/superseded/` rather than overwritten. **Still outside version control** — see
  [[corpus-tooling-outside-repo]]; bringing `bin/` into the repo remains an open work item.
- **Server-side results record no capture provenance** (`~/.vid-to-text/server/results/<job>/job.json`
  holds only id/source/status). PR-022 adds provenance to the timeline itself; the server-side record
  remains a gap.
- **`CLAUDE.md` § Remote server access** also states the repo "is checked out" at `~/vid-to-text/` on
  the desktop; it is an unversioned file copy (verified 2026-08-24: `fatal: not a git repository`).
  Same work item as the systemd staleness above.
- **Frame downscaling** would cut per-frame vision cost ~57% at 720p (measured 2,042 → 882 tokens) but
  chart-text legibility at 720p is unmeasured. Candidate follow-on; not a PR-022 dimension.
- **Hybrid-vs-uniform description quality on static screen content** cannot be measured with any
  metric found (PR-020 Phase 5.5 Finding 1; Tier C harness `wf_bef168b0-50b` caveats). Open until a
  reference-based evaluation exists.

- **Two findings from the deleted PR-027, kept because they will otherwise be rediscovered.**
  (1) `fidelity.json` already persists **per-segment** detail — `SegmentFidelity.stated` with
  `supported: bool` and `.prominent` with `mentioned: bool` — so per-segment or per-video precision
  and recall need **no change to `score_segments`**; only the summary micro-averages. Anyone wanting
  variance estimates or paired comparison can compute them offline from files already on disk.
  (2) The reason PR-023's κ calibration stalled is the **sheet**, not the code: `cohen_kappa` works and
  `review --labels` scores a completed sheet, but `render_html` emits a table row per fact with three
  radio buttons, and ~150 of those was rejected as unworkable. Any future calibration attempt has to
  change the labelling interaction, not the sampling — which was already fixed once (PR-023 defect 5).

- **PR-023's sampling study never ran and is owned by no PR.** Its manifest
  (`/home/rux/vtt-exp/study/runs/manifest.tsv`) holds **2 completed cells of the 21** the study
  designed (`2024_2_19` x `study-t05-g15` and `x study-t05-g30`), and both job directories have since
  been deleted. So PR-022's `scene_threshold` / `max_gap_secs` remain "measured but not optimised" —
  which was PR-023's original motivation — and the F0.5-per-GPU-hour objective it designed was never
  exercised. PR-023's roadmap status was corrected from `[x]` to `[~]` on 2026-08-25 and the three
  unmet criteria marked inline. The κ half moved to PR-027; **the study half has no owner.** Note that
  PR-026's research also found the study's objective needs revisiting before it is re-run: F0.5
  weights precision double, which is defensible for a sampling comparison but was never examined
  against the precision floor the study also imposes.

- **Vision output can degenerate into a numeric counting sequence** ("1.801, 1.802, … 1.877", 568
  tokens in one clip900 segment on 2026-08-25) that `truncate_repetition` — which keys on repeated
  *sentences* — does not catch, and `repetition_report` skips visual segments on the assumption
  that it does. Exposed by the PR-023 fidelity diagnostic (554 unsupported facts in one segment).
  Candidate fix: score visual segments with the window compression ratio too, or bound numeric
  runs in `truncate_repetition`. Not in PR-023's scope (diagnoses only). **Addressed by PR-025**
  (`truncate_numeric_run`) for the consecutive-token form, and by **PR-028** for the templated and
  sub-threshold-verbatim forms.

- **Vision degenerates in at least two further modes that no shipped guard covers.** Both found in a
  single live job on 2026-08-25 (`046cd326-5473-4c74-ae17-56afed903b2e`, `2024_6_24_5-20.mp4`,
  `market-research`, prompt v3.1 `cfab896e`) immediately after PR-028 was deployed — i.e. at the
  current shipped configuration, not a historical one.

  - **Mode 5 — intra-sentence word repetition.** Segment 6, 16,644 chars: "The presenter draws a
    light light light light light light light light light light light light light light light yellow
    line from point ④ to point ③." **1,140 of 3,140 words are the token `light`**, across 126
    sentences. Blind spot is structural for all three guards: sentences are unique (so
    `truncate_repetition` cannot see it), there is no numeric run (so `truncate_numeric_run` cannot),
    and masking numbers does not collapse the sentences because the repeat count itself varies (so
    `truncate_skeleton_repeat` scores it **1**).
    **Sized across the corpus:** re-sweeping the same 11,108 guard-era visual segments for the longest
    consecutive repeat of one word gives p50 **1**, p99 **8**, max **1,214**, with **423 segments at
    >=5** and **93 at >=10**; seven exceed 400. A p99 of 8 against a degenerate floor in the hundreds
    looks like a clean gap, but the threshold needs its own measured round — and note PR-028's
    experience that the *tokenizer* decides the answer.

  - **Mode 6 — templated ramp with a WORD slot.** Segment 8, 13,701 chars: "the presenter's cursor
    moves to a point near the end of April 2033, and then to a point near the end of May 2033" —
    `near the end of` x**215**, with the year marching **2024 -> 2033** on a 2024 video (fabricated
    dates a decade out). This is **PR-028's own mode with a non-numeric slot**: `char::is_numeric()`
    cannot mask a month name, so the skeletons differ and the count stays at 9, under the cap of 24.
    Widening the mask is not the fix — the slot vocabulary is unbounded (months, colours, ordinals).
    PR-028 measured a vocabulary-free alternative (one-hole grouping) and rejected it on
    false-positive cost; that measurement should be revisited against these two cases, noting one-hole
    scores mode 6 at **1** because *two* slots vary, so it would not catch it either.

  Both are under-counted by fidelity exactly as PR-028's case 2 was (16,644 chars -> 22 scored facts;
  13,701 -> 28), which compounds with the entry below. Not in PR-028's scope; that PR guards the
  numeric/glyph-slotted mode and its verification criteria are met.

- **Corpus degeneration rate was mis-scoped and is corrected here.** PR-028 reports "3 in 11,108
  (0.027%)". That is the rate of **the templated-skeleton mode as measured by that detector**, not the
  rate of vision degeneration, which is materially higher — 93 of the same segments carry a word
  repeated 10+ times consecutively. Recorded because the 0.027% figure reads as reassurance about
  output quality and is not evidence for it. Any future claim about corpus cleanliness must name the
  mode and the detector that produced the number.

- **The fidelity diagnostic is nearly blind to non-numeric fabrication.** `score_segments` extracts
  numbers and labels, so a visual segment carrying **144** fabricated trend-line sentences
  (`- A white line is drawn ... labeled "①"`, job `84149f3b`, segment 109, 15,905 chars) yielded only
  **25** scored facts, and the job's whole-video precision was **0.906**. Measured during PR-028's
  Phase 3: applying a guard that removed **12,236 characters** and 118 of those claims changed **not
  one** fidelity number (precision, recall, F0.5, stated, supported, prominent, mentioned all
  identical). A whole class of fabrication is therefore invisible to the metric that exists to catch
  fabrication, which matters for any A/B that uses precision as its bar — PR-026's, for one. Not in
  PR-028's scope (that PR guards; this is a diagnostic defect) and nothing blocks on it. Fixing it
  needs its own calibration round, since widening what counts as a "fact" moves every historical
  precision figure and breaks comparability with runs already recorded.

## How to update this document

- After Phase 4 (Docs) of a design session, add each new PR as a row with its design-research status
- After running `PROCEDURE-pr-research.md` Phase 1 for a PR, update `State assessed`
- After completing all 5 phases, update `Implementation cleared`
- If state assessment surfaces blocking drift, move the PR back to Tier 2 and document required research
- New PRs start as Tier 2 unless they explicitly inherit prior research
