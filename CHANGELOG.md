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

### Changed
- Visual segment timestamps derive from ffmpeg presentation timestamps in every mode instead of
  `frame_offset / fps` arithmetic. Output is identical for uniform sampling. (PR-022)
- The whisper repetition report scores 30-second windows (`whisper.repetition_window_secs`) instead of
  individual segments — the unit the 2.4 threshold was calibrated on. (PR-022)
- The `market-research` profile now sets `vision.fps = 2.0` with adaptive sampling enabled, replacing
  the deliberately-unset `0.0` sentinel from PR-020. (PR-022)
