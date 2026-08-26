# PR-031: Word-run degeneration guard

**Landed-in:** (not yet landed)

**Path tier:** Tier-1 (the failure is measured in-repo across 11,108 segments and the mechanism is
prior art shipped three times; the threshold and the tokenizer are what the procedure must settle)

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

**A fourth degeneration mode, found on the live job that deployed the third guard.** Job
`046cd326-5473-4c74-ae17-56afed903b2e`, segment 6, 16,644 chars:

> "The presenter draws a **light light light light light light light light light light light light
> light light light** yellow line from point ④ to point ③."

**1,140 of that segment's 3,140 words are the single token `light`**, across 126 sentences.

**All three shipped guards are blind to it, structurally rather than by tuning:**

- `truncate_repetition` — every sentence is unique, because the number of repeats differs.
- `truncate_numeric_run` — there is no numeric token anywhere in the run.
- `truncate_skeleton_repeat` (PR-028) — masking numbers does not collapse the sentences, because the
  varying part *is the repeat count itself*. It scores this segment at **1**.

**Measured across the corpus** — the same 11,108 guard-era visual segments PR-028 used, scored for
the longest run of one word repeated consecutively:

| p50 | p99 | max | segments >=5 | segments >=10 |
|---|---|---|---|---|
| 1 | 8 | **1,214** | **423** | **93** |

Seven segments exceed 400. This is the most common degeneration mode found so far — two orders of
magnitude more prevalent than PR-028's templated mode (3 in 11,108).

**A related mode is recorded but NOT claimed to be solved here.** The same live job's segment 8 is
PR-028's templated ramp with a **word** slot — `near the end of` x215, the year marching 2024 -> 2033
on a 2024 video. `char::is_numeric()` cannot mask a month name. Widening the mask is not the fix (the
slot vocabulary is unbounded), and PR-028 measured the vocabulary-free alternative (one-hole grouping)
scoring it at **1**, because *two* slots vary. Whether this PR's detector also addresses that mode, or
whether it needs its own, is a research question — not an assumption.

## Scope

**In scope:**
- A detector for a **word repeated consecutively** beyond a threshold within one visual segment,
  truncating at the cap and keeping the legitimate head, in the slot the three existing guards occupy
  at generation time.
- The threshold, **measured** the way PR-025's and PR-028's were: over every guard-era visual segment
  on disk, with the implementation's own tokenizer, reporting where legitimate runs top out and where
  degenerate ones begin. The p99-of-8 against a degenerate floor in the hundreds suggests a wide gap,
  but that is a Python-probe figure and must be re-derived with the shipped tokenizer.
- Config keys under `[vision]` alongside the existing three, with `0` disabling.
- A log line per truncation stating the observed run length.
- An explicit finding on whether the detector also catches the word-slot templated ramp (segment 8),
  or whether that needs its own PR.

**Explicitly out of scope:**
- Changing any of the three existing guards.
- Retrying generation (`ollama.temperature = 0`; settled in PR-025).
- Editing after merge (forbidden by Segments Are Immutable After Merge).

## Dependencies

- **PR-029** — a threshold validated against a metric that cannot see the failure is not validated.
  PR-028's residue trade-off was argued in precision terms; the same argument is unavailable here
  until the metric prices this text.
- **PR-028** — the guard slot, the tokenizer discipline and the corpus partition method. Landed
  `82df218`.

## Architecture section implemented

`docs/ARCHITECTURE.md` — the vision output guards subsection of § Fidelity Diagnostic, which will
document four guards.

## Verification criteria

- [ ] `046cd326` segment 6 is truncated and its legitimate head survives
- [ ] Threshold measured over every guard-era visual segment on disk, with the separation recorded
      and the tokenizer stated explicitly
- [ ] A legitimate segment at the measured p99 is untouched
- [ ] An explicit, recorded answer on whether segment 8's word-slot ramp is caught
- [ ] Truncations are logged with the observed run length
- [ ] `0` disables the guard; default is the measured value
- [ ] `cargo test --workspace` passes

## Research backing

**Tier-1.** The failure, its prevalence (423 and 93 segments) and the blindness of all three guards
are measured in-repo. What the procedure must settle:

1. The tokenizer. PR-025 nearly shipped a cap of 24 against a true 40 on a wrong tokenizer, and
   PR-028's specified detector missed a known case because `is_ascii_digit` was the wrong digit class.
   Word tokenization has the same trap surface: case, punctuation, hyphenation, and whether "light
   light" and "light, light" are the same run.
2. The threshold, and whether a single cap separates the classes — p99 8 vs a degenerate floor in the
   hundreds looks clean, but 54, 36, 31, 29 and 25 sit between and must be inspected, not assumed.
3. Whether the same detector covers the word-slot templated ramp.

## Notes

- Four guards is a symptom count, not a quality measure. Each mode was found *after* deploying the
  guard for the previous one, which is worth stating plainly when judging whether a fifth is coming.
- The prevalence figures (423, 93) come from a Python probe, not the shipped tokenizer. Treat them as
  a scoping estimate until re-derived — that is exactly the error PR-025 recorded and PR-028 repeated.
