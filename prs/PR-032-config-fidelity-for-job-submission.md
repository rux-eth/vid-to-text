# PR-032: Config fidelity for job submission

**Landed-in:** (not yet landed)

**Path tier:** Tier-1 (three concrete defects located in the source with line numbers; the fixes are
mechanical and the only open question is the cancellation-check placement)

## Before Implementation (NON-NEGOTIABLE)

This PR MUST NOT be implemented until `PROCEDURE-pr-research.md` has been completed in full and its
output appended to the `## Research findings` section below.

**Tier-1 PRs** (research-backed at design time): Phase 1 (State Assessment) is required to catch
drift. Phases 2-4 may be light if no drift is found.

**Tier-2 PRs** (research-pending): all 5 phases of `PROCEDURE-pr-research.md` must run before this PR
is written in final form.

Skipping the PR research procedure is a hard violation of the research-backed-decisions constraint in
`docs/CONSTRAINTS.md`.

## Research findings

### State Assessment (2026-08-26)

**Current state** — all three defects confirmed at the lines the PR names, by reading the source:

- **Defect 1 confirmed.** `vtt-client/src/main.rs:238` calls `process::process_single_file(&client,
  &config, path, force)` and `:240` calls `process_directory(..., force)` — neither takes `profile`.
  Only `process_url` receives it (`:229`). `upload_file` (`vtt-client/src/api.rs:36-39`) has no
  `profile` parameter at all, so the client cannot send it even if the caller had it.
- **Defect 2 confirmed.** `vtt-server/src/main.rs:460` ends the `file` branch with `break`, exiting the
  `while let Some(field)` loop. Every field after `file` is unread.
- **Defect 3 confirmed.** `check_cancelled()` is a closure over the token
  (`vtt-core/src/pipeline.rs:32-40`) called at `:117`, `:185` and `:217` — around whole phases.
  `describe_chunk` (`vtt-core/src/vision.rs:159`) takes no token and loops batches internally
  (`:205+`), so a cancelled job runs to the end of the current chunk's entire vision pass.

**Stale assumptions** (where current state disagrees with the PR as drafted):

1. **Scope item 4 — "fail loudly on a named profile that does not resolve" — is ALREADY DONE.**
   `load_profile` (`vtt-core/src/config.rs:869-880`) returns `VttError::Config("profile not found:
   {name} (looked in {path})")` when the file is absent, and `resolve_config`
   (`vtt-server/src/main.rs:210-214`) maps that to `ApiError::BadRequest`. It **also** already
   validates the merged config at submission rather than at job start, with a comment recording that
   PR-020's review put it there. There is no silent fallback to base config. **This scope item should
   be struck, not implemented.** The real defect is that `profile` never *reaches* `resolve_config`
   for local files — which is defects 1 and 2, already in scope.

2. **Defect 2 is live for `--force` today, not hypothetical.** The PR describes it as a risk to "a
   request sending `file` before `profile`". But the **shipped client itself** builds the form as
   `Form::new().part("file", part)` and only then `.text("force", "true")`
   (`vtt-client/src/api.rs:57-60`) — `file` is always first. Combined with the `break`, **`--force`
   has never worked for a local file or directory upload.** It is silently ignored on every run. The
   URL path is unaffected because `submit_url_job` sends JSON, not multipart, which is also why
   `--profile` works for URLs and not for files.

3. **The Architecture section this PR claims to implement does not exist.** `## Architecture section
   implemented` names "the client/server split and profile resolution". `docs/ARCHITECTURE.md` has no
   such section — its headings are System Overview, Components, Data Flow, Frame Sampling, Fidelity
   Diagnostic, Key Abstractions, Storage, Capture Configuration, Testing Strategy — and the only
   mention of profiles anywhere is one incidental "under the market-research profile" at line 120.
   This PR must **add** that documentation, not update it.

**New constraints** (learned from prior PRs and the codebase):

- **The feature was shipped server-only and has never worked end-to-end.** `ae16569` ("Add profile
  support to upload endpoint", 2026-03-30) added the server's `profile` field parsing in an 11-line
  diff that touched **only `vtt-server/src/main.rs`**. The client was never taught to send it, and the
  pre-existing `break` meant the field it added could only ever be read from a request that put
  `profile` before `file`. Nothing has exercised the path since.
- **Nothing would have caught it.** `vtt-server/src/main.rs` has three tests
  (`test_job_response_serialization`, `test_job_response_with_error`,
  `test_create_job_request_deserialization`) — all serialization, none touching the multipart handler.
  The upload endpoint has **no test coverage at all**. Any fix here must add the first.
- **Cancellation granularity has a natural home that avoids the PR's stated risk.** The PR worries
  about "a check inside the per-batch loop that could interact with the retry path". The retry loop is
  `for attempt in 0..3` at `vision.rs:220`, *inside* the per-batch body. A check at the **top of the
  batch loop**, before `build_prompt` and before the retry loop is entered, is strictly outside the
  retry path. Expected effect: at `max_frames_per_request = 15`, the fixed-mode failure case (360
  frames/chunk = 24 batches over ~13 min) drops worst-case latency from a full chunk to roughly one
  batch, ~30 s.
- **`No Network Calls From Client to Models` (CONSTRAINTS) is untouched** by any of this — the fix is
  entirely about what the client *sends to the server*.

**Downstream contracts** (bidirectional sweep — `grep -rn "PR-032" prs/ docs/`):

- **PR-030** → *"so the run is submitted with the profile actually applied. Today the documented CLI
  path silently ignores `--profile` for local files."* Its note also records the interim workaround:
  submit by `curl` with `profile` and `force` **before** `file`, and verify the applied config from
  the first `[ocr] chunk_N` frame count. **Satisfied by the current scope** (defects 1 + 2), and the
  workaround becomes unnecessary once defect 2 is fixed.
- No other PR depends on this one. Upstream: **none** — `Dependencies: None`, confirmed.

**Path-tier checkpoint:** **Tier-1 confirmed** — the PR header, `docs/0.0/ROADMAP.md:89` and
`docs/0.0/RESEARCH-BACKLOG.md:73` all agree, and no cut-plan disagrees. Phase 1 found **no premise
change and no unsatisfiable downstream contract**: every defect is real, located, and the fixes are
mechanical. The three stale assumptions all make the PR *smaller or better-specified*, not different in
kind. **Cleared after Phase 1; Phases 2-4 do not run.**

**Time-decay:** drafted 2026-08-26, assessed 2026-08-26 — 0 days against a 30-day threshold.

### Gate Check (2026-08-26)

- Premise still valid: ✓ — three defects confirmed in source, one of them (dropped `--force`) worse
  than the PR claimed
- No prerequisite PRs surfaced: ✓
- Scope changes: **scope item 4 struck as already-implemented**; `--force` added to the stated
  breakage; the ARCHITECTURE work is an addition rather than an edit
- Research spend: Phase 1 only, per Tier-1
- Implementation cleared: ✓ (2026-08-26)


### Implementation Validation (2026-08-26)

**Built and tested. 251 tests pass** (`cargo test --workspace`), 6 new — including the **first tests
the multipart upload endpoint has ever had**, which Phase 1 identified as the reason the defect
survived.

**The tests were verified to catch the regression, not merely to pass.** Reintroducing the original
`break` makes exactly the two order-sensitive tests fail
(`test_upload_fields_survive_file_first_ordering`, `test_upload_field_order_is_irrelevant`) while
`test_upload_fields_survive_file_last_ordering` still passes — which is correct, since that ordering
always worked and is what the `curl` workaround relied on.

**Defect 1 — `--profile` for local files.** `profile` threaded through `upload_file` →
`process_single_file` → `process_directory` → `main.rs`. The directory path passes it per file.

**Defect 2 — silently dropped fields.** The field loop was extracted to `read_upload_fields` (so it is
testable at all) and no longer `break`s on `file`. The client now writes scalar fields **before** the
file part as well, so an older server still receives them — belt and braces, because the one-sided
version is what shipped and failed.

**Defect 3 — cancellation latency.** `describe_chunk` takes an optional `CancellationToken`, checked at
the **top of the batch loop**, outside the `for attempt in 0..3` retry loop — so a cancellation can
never be consumed as a failed attempt. A test asserts a pre-cancelled token yields `VttError::Cancelled`
against an unroutable endpoint, and that the same call *without* a token fails differently — which is
what proves the assertion tests the cancellation path rather than a network error.

**Observability at submission.** `JobResponse` carries the applied profile (omitted when the job runs
under base config), and the client prints `(profile: market-research)` or `(profile: server default)`
when the job is created.

**Verification criteria:**

- [x] `--profile` applies to a local file and a directory, pinned by a test
- [x] A multipart request with `profile` after `file` applies the profile — never silently ignores it;
      field order is pinned as irrelevant
- [x] An unresolvable profile name fails the submission — **already true before this PR**; verified,
      not reimplemented (see Phase 1 stale assumption 1)
- [x] A cancelled job stops without waiting for a full chunk's vision pass, pinned by a test
- [x] A job's applied profile is observable at submission, not only by reading `capture` afterwards
- [x] `cargo test --workspace` passes

**Beyond the PR as drafted:** `--force` was found to be broken for every local upload, not merely at
risk. It is fixed by the same change and is recorded in the changelog as a user-facing fix.

**Not claimed.** The cancellation improvement is bounded by *batch* granularity and was not measured
live — the estimate of ~30 s against ~13 min follows from 24 batches over a 360-frame chunk, not from
a timed cancellation on the GPU host. Nothing in this PR was exercised against the live server.

---

## Motivation

**A job can silently run under the wrong configuration, and did.** Found 2026-08-25 while deploying
PR-028: a run launched as `vid-to-text process <file> --profile market-research` executed with **none**
of that profile applied — wrong sampling (fixed 2 fps, 360 frames per chunk instead of adaptive's
16-20), wrong prompt (the server's generic `default_prompt` instead of `prompts/vision-chart.txt`) and
wrong transcript policy (conditioned and full-window instead of `use_transcript = false`, causal).

Nothing warned. The job reported success and would have produced a timeline whose `capture` block
recorded what actually ran, but only *after* burning an estimated 108 minutes of GPU time.

Three distinct defects, all located:

1. **`--profile` is accepted and never sent, for local files.** `vtt-client/src/main.rs:238` calls
   `process::process_single_file(&client, &config, path, force)` — no profile argument. Only
   `process_url` receives it (`main.rs:229`). The server supports the field; the client never sends it.
2. **The multipart handler silently drops every field after `file`.** `vtt-server/src/main.rs:460`
   ends the `file` branch with `break`, exiting the field loop. A request sending `file` before
   `profile` loses the profile — and `force` — with no error. The `curl` workaround for defect 1 fails
   silently unless the fields happen to be ordered correctly.
3. **Cancellation can go unnoticed for minutes.** `DELETE /jobs/{id}` cancels a real token, but
   `check_cancelled()` is only called at `vtt-core/src/pipeline.rs:117/185/217` — after a full chunk's
   vision pass. At the fixed-mode frame count that is ~13 minutes, during which the cancelled job
   keeps the GPU and blocks the queue. Observed: a cancelled job continued through two further chunks.

**Why it matters beyond one run.** The corpus is 68 videos pending capture. A silent config fallback
across a batch would produce a corpus that is internally inconsistent in sampling, prompt and
transcript policy, with the damage visible only by reading each timeline's `capture` block afterwards.
`config/profiles/market-research.toml` already carries a comment recording that a key placed after a
nested table header "silently reverted two locked values once already (PR-022)" — this is the same
class of failure at the submission layer.

## Scope

**In scope:**
- Thread `profile` through the local-file and directory upload paths in `vtt-client`.
- Make the server's multipart parsing order-independent, or reject a request whose `profile`/`force`
  fields cannot be honoured, rather than dropping them silently.
- Tighten cancellation latency so a cancelled job stops without waiting for a full vision pass.
- ~~Fail loudly on a named profile that does not resolve, rather than falling back to base config.~~
  **Struck 2026-08-26 (Phase 1): already implemented.** `load_profile` errors on a missing profile and
  `resolve_config` returns it as a 400, validating the merged config at submission. No silent fallback
  exists.

**Explicitly out of scope:**
- Any change to what the profiles contain, or to sampling, prompts or capture parameters.
- Reworking the job queue or concurrency model.
- The `tools/prompt_ab.py` driver.

## Dependencies

None. Both dependencies of the work it unblocks (PR-030) are separate.

## Architecture section implemented

`docs/ARCHITECTURE.md` — the client/server split and profile resolution.

## Verification criteria

- [ ] `--profile` applies to a local file and a directory, pinned by a test
- [ ] A multipart request with `profile` after `file` either applies the profile or errors — never
      silently ignores it
- [ ] An unresolvable profile name fails the submission
- [ ] A cancelled job stops without waiting for a full chunk's vision pass, pinned by a test
- [ ] A job's applied profile is observable at submission, not only by reading `capture` afterwards
- [ ] `cargo test --workspace` passes

## Research backing

**Tier-1.** All three defects are located in-repo with line numbers and were reproduced live on
2026-08-25 (jobs `6358306f`, `89a156c9`, `046cd326`). The one genuinely open question is where the
cancellation check belongs so that latency drops without adding a check inside the per-batch loop that
could interact with the retry path.

## Notes

- Defect 2 is the reason the `curl` workaround for defect 1 appeared to work and did not: two
  submissions ran under the wrong config before the frame count was checked. Any fix must be verified
  by observing the applied config, not by observing that the request was accepted.
- PR-030 needs a correctly-configured run of `2025_05_26`. Until this PR lands, that run must be
  submitted by `curl` with `profile` and `force` **before** `file`, and the applied config verified
  from the first `[ocr] chunk_N` frame count.
