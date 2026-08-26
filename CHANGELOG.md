# Changelog

All notable user-facing changes to vid-to-text. Format: [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/). Versioning policy: see [`docs/VERSIONING.md`](docs/VERSIONING.md).

## [Unreleased]

<!-- Add entries here as PRs land. At a version cut, rename to ## [x.y.z] - YYYY-MM-DD. -->

### Added
- Content-adaptive frame sampling (`[vision.adaptive]`): a uniform floor plus scene-change-triggered
  frames, de-clustered, with a per-chunk cap. Fixed-fps mode is unchanged and remains the default. (PR-022)
- Visual segments now carry the real capture timestamps of their frames (`frames`), and the timeline
  records its capture configuration (`capture`). Both are omitted when absent. (PR-022)
- The vision prompt lists each frame's capture time. (PR-022)
- Pre-flight check that `max_frames_per_request` fits `ollama.num_ctx` at the source resolution,
  failing before GPU time is spent. (PR-022)
- Visual fidelity diagnostic (`[fidelity]`): each visual segment's stated numbers, tickers and
  timeframes are checked against OCR of its own source frames; summary in the timeline's `fidelity`
  block, detail in `fidelity.json`, kept-frame thumbnails stored with results. Off by default. (PR-023)
- `vid-to-text review` renders a self-contained review sheet for a job and scores a labels file
  (Cohen's κ, false/missed hallucinations). (PR-023)
- `doctor` and `/health` report the OCR engine when the diagnostic is enabled. (PR-023)
- OCR-grounded vision prompts (`[vision.ocr_grounding]`): each frame's detected text is given to the
  vision model alongside the image, marked as a fallible reading aid with the image authoritative.
  **Off by default, and measured to make no difference** to factual accuracy on this corpus (n=2
  videos; +34% runtime), so it is not enabled in the locked profile. Kept because it is cheap to
  re-test on other content. (PR-024)

- A chart/screencast vision prompt (`prompts/vision-chart.txt`), selected by
  `ollama.prompt_template_path` and enabled in the `market-research` profile. The previous prompt was
  written for TED talks and film clips: nine of its ten instructions presuppose humans, faces and
  camera work, and 76% of this corpus's visual segments carried absent-human boilerplate as a result.
  The new prompt also states the interface conventions that produced a fabricated price chronology
  (a chart header shows the *hovered* candle, not the current price). The general prompt remains the
  default for non-chart video. (PR-026)
- Timelines record which vision prompt produced them: `capture.vision_prompt` and
  `capture.vision_prompt_sha256`. The hash equals `sha256sum` of the prompt file, so a timeline can be
  checked against what is deployed. Omitted when absent, so older timelines still load. (PR-026)
- `config/deploy-prompts.sh` deploys `prompts/` to the server by checksum-compare with
  verify-after — previously nothing deployed prompts at all. (PR-026)

- Guard against degenerate vision output (`vision.max_numeric_run`): a visual description that
  enumerates more than 40 consecutive numbers is truncated at the cap, keeping the legitimate head.
  Catches both observed modes — an arithmetic ramp and a repeated value — which the existing
  sentence-repetition guard could not see. (PR-025)

- Guard against repeated sentence templates in vision output (`vision.max_skeleton_repeat`,
  `vision.min_skeleton_chars`): a visual description that repeats one sentence *skeleton* — the
  sentence with its numeric tokens masked — more than 24 times is truncated at the cap, keeping the
  legitimate head. This is a third degeneration mode that both existing guards miss by construction:
  every sentence is unique because the slot differs, and prose separates every number so there is no
  consecutive run. Observed producing 267 drawn price levels marching past zero into negative Bitcoin
  prices, which dragged one video's precision from 0.926 to 0.529; with the guard it scores 0.901.
  The mask uses `char::is_numeric()` rather than ASCII digits, because one observed case varied a
  circled-glyph slot (`①②③`) that ASCII masking cannot see. Thresholds measured over 11,108 visual
  segments: legitimate repeats top out at 13, degenerate ones are 143 and above. (PR-028)

### Changed
- OCR engine configuration moved from `[fidelity]` to its own `[ocr]` section, shared by the fidelity
  diagnostic and OCR grounding; each job now OCRs its frames once, overlapped with GPU work. (PR-024)

### Changed
- Visual segment timestamps derive from ffmpeg presentation timestamps in every mode instead of
  `frame_offset / fps` arithmetic. Output is identical for uniform sampling. (PR-022)
- The whisper repetition report scores 30-second windows (`whisper.repetition_window_secs`) instead of
  individual segments — the unit the 2.4 threshold was calibrated on. (PR-022)
- The `market-research` profile now sets `vision.fps = 2.0` with adaptive sampling enabled, replacing
  the deliberately-unset `0.0` sentinel from PR-020. (PR-022)
