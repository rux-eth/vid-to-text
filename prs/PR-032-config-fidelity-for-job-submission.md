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

_To be populated by `PROCEDURE-pr-research.md`._

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
- Fail loudly on a named profile that does not resolve, rather than falling back to base config.

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
