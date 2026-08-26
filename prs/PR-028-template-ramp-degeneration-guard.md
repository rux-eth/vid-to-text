# PR-028: Template-ramp degeneration guard

<!-- Landed-in: set to the released version this PR shipped under (e.g. v0.1.0).
     Use "(not yet landed)" for in-flight or dormant PRs.
     Use "superseded by PR-XXX" for replaced PRs.
     See docs/VERSIONING.md §4 for the policy. -->
**Landed-in:** (not yet landed)

**Path tier:** Tier-1 (the failure is measured in-repo with a reproducing case; the threshold is not
yet set, and setting it is the research)

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

_To be populated by `PROCEDURE-pr-research.md`. The reproducing case below is measurement, not the
research round: the detector's shape and its threshold are what the procedure must settle._

### State Assessment (2026-08-25)

**Current state.**

- Both guards live in `vtt-core/src/vision.rs` and run at generation time inside the per-batch parse
  path, before a `Segment` is constructed: `truncate_repetition` (`vision.rs:251`) then
  `truncate_numeric_run` (`vision.rs:260`), each logging its own cut.
- `truncate_repetition` (`vision.rs:519`) splits on `.`, keys trimmed lowercased sentences of >=15
  chars, keeps at most 2 occurrences and `break`s at the 3rd. (Its doc comment says "truncate at the
  second occurrence"; the code truncates at the third. Cosmetic, and it matches this PR's description.)
- `truncate_numeric_run` (`vision.rs:442`) uses an **ASCII-only** tokenizer (`number_end`, which tests
  `is_ascii_digit()`), caps at `vision.max_numeric_run` (`config.rs:380`, default 40 at
  `config.rs:581`, `0` disables) and returns the observed run length for the log line.
- `docs/ARCHITECTURE.md` § Fidelity Diagnostic states "**Two** run at generation time" — the line this
  PR amends.
- `rescore` (`vtt-client/src/review.rs:282`) reads `timeline.json` + `ocr.json` and calls
  `score_segments(&timeline.segments, ...)`. **Fidelity can therefore be recomputed offline from
  stored segment text**, with no GPU re-run.

**Corpus reachable for the threshold measurement** (checked, not assumed):

| pool | timelines | visual segments | era |
|---|---|---|---|
| desktop `~/.vid-to-text/server/results/` | 43 | **1,572** | market-research, post-guard |
| laptop `~/.vid-to-text/cache/` | 34 | **3,124** | mixed |
| `seer_archive/pre-pr020-timelines-20260825.tgz` | 73 files | not yet scanned | pre-PR-020 |

**`truncate_repetition` is confirmed live, empirically.** Across all 1,572 desktop visual segments the
maximum verbatim repeat of a >=15-char sentence is **2**, with **zero** segments at 3 or more — which
is exactly the ceiling the guard imposes by construction. This corroborates the Motivation's claim
from the corpus rather than from one job's log.

**Skeleton-repeat distribution**, masking numeric tokens with a faithful port of `number_end` and
splitting the masked text on `.` (desktop pool, 1,572 segments):

| p50 | p90 | p99 | legitimate max | outlier |
|---|---|---|---|---|
| 1 | 1 | 4 | **10** | **267** (the reproducing case) |

The legitimate top of the distribution is ordinary structured description:
`- a candlestick near the # level` (10), `between frames # and #, the cursor moves right` (10),
and `a horizontal line is drawn at #, labeled "cme gap"` (9). So the counter-case named in
§ Research backing ("a segment that legitimately reports six drawn levels in six sentences")
**exists in this corpus and is common**; 6-10 is the normal range for it.

**Stale assumptions** (where current state disagrees with the PR as drafted):

1. **"One segment" is wrong — there are two.** A second template-ramp segment exists in the same
   corpus: job `84149f3b-4fe3-4390-940a-acc52f29c182`, timeline segment index **109** (visual segment
   7 of 10, span `00:10:30`–`00:12:00`, 15,905 chars). It contains **144** sentences of
   `- A white line is drawn from the bottom left to the top right, passing through the point labeled "X".`
   where X ramps through circled-number glyphs — `①②③` … `㊶㊷㊸` … `⑩⑪` … `⑪②⑪③⑪④` — the identical
   shape: one sentence skeleton, one varying slot, marching past the end of the real content.

2. **The varying slot is not always numeric, and the specified detector misses the case where it
   isn't.** § Scope specifies masking "its **numeric** tokens". Case 2's slot is circled glyphs, which
   `number_end` does not tokenize because they are not ASCII digits. Measured with an ASCII-faithful
   port, case 2's maximum skeleton repeat is **14** against a legitimate corpus max of **10** — no
   usable separation. **A numeric-mask-only skeleton detector catches case 1 (267) and misses case 2.**
   This is a scope finding, not a threshold finding, and it is what Phase 2 must scope.

   *Recorded because the wrong number was nearly used again:* the first probe ported `number_end`
   into Python using `str.isdigit()`, which is Unicode-aware and masks `①`/`②`. It reported case 2 at
   **89** repeats and made the detector look sufficient. Rust's `is_ascii_digit()` does not, and the
   true figure is 14. This is the same near-miss class PR-025 recorded; caught this time before it set
   a threshold.

3. **A naive masking regex destroys sentence splitting outright.** Masking with `-?[0-9][0-9,.]*`
   swallows the sentence-final period (`71,836.` -> `#`), collapsing the reproducing segment from 275
   sentences to **8** and reporting a max skeleton repeat of **1**. `number_end` is safe because it
   advances past `,`/`.` **only when the next character is a digit**. Reusing it is a correctness
   requirement, not a style preference.

4. **The fidelity metric is nearly blind to the non-numeric variant, so "precision recovers" only
   tests half the mode.** Case 2's job scores whole-video precision **0.906**; the offending segment
   is fidelity segment 7 at **25 stated / 21 supported / 0.840**. 15,905 characters carrying 144
   fabricated trend-line claims yield **25** scored facts, because the metric extracts numbers and
   labels and these sentences contain neither. The precision collapse in the Motivation (0.926 ->
   0.529) is real for case 1 and does not generalise; verification criterion 1 as written measures the
   numeric variant only.

**New constraints:**

- **Ordering is settled by construction, not preference** (§ Research backing Q3). `truncate_repetition`
  caps verbatim repeats at 2 and cannot mask, so it can never pre-empt a skeleton guard; the skeleton
  guard must run on its output, third in the chain — the same slot `truncate_numeric_run` was given in
  PR-025.
- **The measurement pool must be partitioned by guard era.** The laptop cache contains **4 segments
  with 3+ verbatim repeats** (58, 28, 18, 6 — `4b119bcd`, `92d8534b`, `585cc0e6`, `f91ba589`), which
  the shipped `truncate_repetition` makes structurally impossible, so those timelines predate it.
  `>=3 verbatim repeats of a >=15-char sentence` is therefore a sound, self-verifying pre-guard marker.
  Pre-guard segments inflate the apparent "legitimate max" (58 vs the guard-era 10) and must be
  excluded before a threshold is chosen. Their `processed_at` fields are identical to the second
  across entries, so they are bulk-import timestamps and cannot date the runs.
- **Verification criterion 1 is achievable without a GPU re-run**, via `rescore` over a copy of the
  preserved job directory with the guard applied to the stored text. This matters because the prompt
  that produced case 1 (v3, `c0921846`) is **not deployed** — `prompts/vision-chart.txt` holds v3.1 —
  and PR-026's v2-vs-v3.1 shipping decision is still open, so a live re-run would not reproduce it.

**Downstream contracts:** **none** — no PR depends on this one (verified via `grep -rl "PR-028" prs/
docs/`, which returns only this file, `prs/PR-026-content-specific-vision-prompt.md:1340` where the
defect was split out, and the `ROADMAP.md` / `RESEARCH-BACKLOG.md` index rows). Upstream, both
dependencies are landed and unchanged: PR-023 (`f570aec`) supplies the diagnostic, PR-025 (`f45ce72`)
supplies the guard slot, the tokenizer and the measurement method.

**Prior art carried forward** (from `git log -p vtt-core/src/vision.rs`):

- `6f10acd` (2026-03-29) established that editing vision output at generation time is this project's
  accepted remedy for generation loops.
- `f45ce72` (PR-025) established the measurement method — every visual segment on disk, scored with
  the implementation's own tokenizer — and the failure mode of not doing so (a wrong tokenizer
  suggested a cap of 24 against a true 40).
- PR-025 also established that retrying is useless at `ollama.temperature = 0`, and that compression
  ratio does not separate degenerate from clean visual output at any threshold. Neither is to be
  re-derived.

**Path-tier checkpoint:** header says **Tier-1**; `docs/0.0/ROADMAP.md:75` agrees and there is no
cut-plan to contradict it. Tier-1 is retained — the open questions are answerable in-repo by
measurement plus at most one Tier-A probe. **Phase 1 is NOT clean**: stale assumption 2 shows the
detector shape specified in § Scope does not cover a known instance in this corpus, which is a scope
question rather than a threshold question. Per the procedure's branch, **Phases 2-4 run**.

### Research Questions (2026-08-25)

Phase 1 moved the centre of gravity of this round. As drafted, the PR treated the **threshold** as the
research and the **detector shape** as settled. Phase 1 inverted that: the shape is the open question
(a numeric-mask skeleton provably misses case 2), and the threshold now looks comfortable rather than
tight. The questions below are scoped accordingly.

**Must-answer:**

1. **Q1 — What mask class makes a skeleton detector cover both known instances without
   false-positiving on legitimate structured description?**
   *Success criteria:* a named, implementable predicate (a Rust `char` classification or an explicit
   token rule — not a hand-listed glyph set), measured over the guard-era corpus with a faithful port,
   reporting (a) the maximum legitimate skeleton repeat, (b) the repeat count for case 1 and case 2
   under that rule, and (c) the separation between them. A rule that leaves case 2 inside the
   legitimate band fails the criterion.
   *Candidates already on the table from Phase 1:*
   - **ASCII-digit mask** (`is_ascii_digit`, as § Scope specifies) — measured: case 1 = 267, case 2 =
     **14**, legitimate max 10. Fails on case 2.
   - **Unicode-numeric mask** (`char::is_numeric()`, i.e. Unicode `Nd|Nl|No` — covers `①`, `㊶`, roman
     numerals, fullwidth digits). The accidental Unicode-aware port in Phase 1 scored case 2 at **89**
     with legitimate max unchanged at 10; `is_numeric()` is *broader* than that port, so the true
     figure needs measuring and is expected to be higher.
   - **Vocabulary-free structural** (group sentences sharing a long common prefix, or sentences
     differing in exactly one token). Covers any varying slot including non-numeric ones, at the cost
     of a second threshold and more false-positive surface.

2. **Q2 — Where does the threshold go, and does a gap exist on the guard-era pool?**
   *Success criteria:* the full distribution over **every guard-era visual segment on disk** measured
   with the implementation's own tokenizer (the tokenizer named explicitly, per PR-025's near-miss),
   reporting max legitimate, min degenerate, the chosen value and how many segments it truncates. Two
   additional constraints Phase 1 surfaced that the value must satisfy:
   - **It must preserve case 1's own legitimate head.** The reproducing segment opens with **20**
     sentences of the same skeleton carrying irregular, plausibly-real levels (71,836 / 69,358 /
     68,993 … 31,145) before the round-number ramp begins at 30,000. A cap at or below 20 truncates
     real content in the very segment the guard exists to fix — which is verification criterion 4.
     Phase 1's corpus legitimate max is **10**, so a cap in roughly **[21, 88]** appears to satisfy
     both ends; Q2 must confirm that with the measurement rather than the estimate.
   - **The pool must be partitioned by guard era** using the `>=3 verbatim repeats` marker from Phase 1.
     Pre-guard segments are reported as a corpus-wide rate but excluded from threshold-setting.

3. **Q3 — Is templated repetition with a varying slot a recognised degeneration mode with an
   established detector?**
   *Success criteria:* at least two independent primary sources (peer-reviewed work, production system
   documentation, or OSS with meaningful adoption) that name the phenomenon and describe any detector
   used against it — or an explicit, recorded statement that a genuine search found none, with the
   queries listed. If an established detector exists and is cheap, it is preferred over inventing one;
   if it is expensive or unvalidated at this scale, that is recorded as the reason for not adopting it.
   *Constrained by prior art:* compression ratio is already measured and rejected on this corpus
   (PR-025) and is not to be re-opened without new justification; retry/re-prompt is settled by
   `ollama.temperature = 0`.

4. **Q4 — How is the guard verified, given that the metric is blind to case 2 and the prompt that
   produced case 1 is not deployed?**
   *Success criteria:* a concrete, runnable verification path per case. Phase 1 establishes that
   `rescore` recomputes fidelity offline from stored segment text, so case 1 can be verified by
   applying the guard to a copy of the preserved job directory and re-scoring — no GPU re-run, and no
   dependence on PR-026's still-open v2-vs-v3.1 decision. Case 2 produces only 25 scored facts from
   15,905 characters, so precision cannot verify it; the criterion for that case must be restated in
   terms the evidence supports (characters and sentences removed, head preserved, verified by reading).

**Dependencies:**

- **Q2 depends on Q1** (sequential — the threshold is meaningless until the mask class is fixed;
  this is exactly the failure PR-025 recorded).
- **Q3 independent** of Q1/Q2, and may feed back into Q1 if it surfaces a better-validated detector.
- **Q4 depends on Q1** only weakly (it needs to know which cases the guard is expected to catch).

**Research plan (depth tiers):**

- **Q1 — internal: measurement, no web round.** Rationale: the question is "which predicate separates
  the classes *on this corpus*", which no external source can answer. Cross-checked against Q3's probe
  before locking. This follows PR-025's precedent, where the threshold was measured rather than
  researched.
- **Q2 — internal: measurement, no web round.** Same rationale; PR-025's method transfers directly.
- **Q3 — Tier A** (probe), **escalate to Tier B if inconclusive**. Rationale: this is the only question
  with a genuine external answer, and it is option-driving — if a validated detector for this exact
  mode exists, inventing a third bespoke guard would be the wrong call. Escalation trigger: only
  secondary sources found, or sources conflict on whether masked-skeleton detection is sound, **and**
  the answer would change Q1's recommendation.
- **Q4 — internal — no web round** (resolved in Phase 4 synthesis).
- **Rounds:** Round 1 = Q3 Tier-A probe and Q1 measurement, run in parallel (independent). Round 2 =
  Q2 measurement, after Q1 locks the mask class. Round 3 = Q4 decided in synthesis.
- **Default tier:** A. **Escalations above A:** none planned; Q3 carries a conditional B trigger stated
  above.

**Resolved in Phase 1 — no round needed:**

- *Original § Research backing Q1* ("does a masked-skeleton match false-positive on legitimate
  structured description? Candidate counter-case: six drawn levels in six sentences"). **Answered:**
  that counter-case exists in this corpus and is ordinary — the legitimate top of the distribution is
  6-10 repeats (`- a candlestick near the # level`, `a horizontal line is drawn at #, labeled "cme
  gap"`). It bounds the threshold from below; it does not defeat the detector. Q2 carries it forward
  as a constraint.
- *Original § Research backing Q3* ("does the guard interact with `truncate_repetition`, and in what
  order?"). **Answered by construction:** `truncate_repetition` caps verbatim repeats at 2 and cannot
  mask, so it can never pre-empt a skeleton guard. The skeleton guard runs third, on its output — the
  slot `truncate_numeric_run` already occupies.

**Explicitly excluded from this round** (nice-to-have):

- **Whether the fidelity metric should score non-numeric fabricated claims.** Phase 1 finding 4 shows
  it under-counts case 2 by roughly two orders of magnitude (25 scored facts from 144 fabricated
  sentences). That is a defect in the *diagnostic*, not in the guard, and changing it is a metric
  change with its own calibration burden. **Flagged as a candidate follow-up PR**, not researched here.
- **Corpus-wide rate of this mode in the pre-PR-020 archive** (§ Notes). The archive is scanned as part
  of Q2's pool for reporting, but pre-guard segments are excluded from threshold-setting, so the rate
  is a recorded number rather than a research question.
- **Fixing `truncate_repetition`'s doc comment** (says "second occurrence", cuts at the third).
  Cosmetic, and § Scope forbids touching that function.
- **Retry / re-prompt of a degenerate batch.** Settled by PR-025 at `temperature = 0`; recorded so it
  is not re-proposed.
- **Compression ratio as the detector.** Measured and rejected in PR-025; not re-opened.

### Findings (2026-08-25)

**Measurement pool.** 113 timeline files across three locations, deduplicated by content SHA-256 to
**94 distinct timelines** (19 appear in more than one pool — the archive is largely a backup of laptop
cache entries, so the un-deduplicated 13,307 would have double-counted). Four timelines are pre-guard
by the Phase-1 marker and are excluded, leaving the threshold pool at **90 timelines / 11,108 visual
segments** — 4.6x PR-025's 2,433. Every figure below is measured with a faithful Python port of
`vtt-core/src/vision.rs`'s own tokenizer, parameterised only on the digit predicate.

**A third degenerate segment was found during the sweep** (see Q1 disconfirming evidence): job
`37a3242c-5194-4500-8dde-52c41eec5228`, segment 130, 13,194 chars, in which the model looped on
`"You're not."` **878 times**. It is a *Back to the Future* clip and the loop reproduces a line of
dialogue. This is a **fourth** degeneration mode, not the templated one: there is no varying slot, it
is verbatim. `truncate_repetition` misses it because `"You're not"` is **10 characters**, below that
function's `key.len() < 15` gate, which pushes short sentences through unconditionally. It matters
here only because the proposed detector catches it or not depending on one constant — see Q2b.

---

**Q1: What mask class makes a skeleton detector cover both known instances without false-positiving
on legitimate structured description?**

*Options considered* (all measured over the same 11,108-segment pool):

- **Option A — ASCII-digit mask** (`is_ascii_digit()`, as § Scope specifies)
  - Source: the shipped tokenizer, `vtt-core/src/vision.rs:442` (`f45ce72`).
  - Pros: zero change to an already-shipped, already-tested predicate; cheapest possible port.
  - Cons: **fails the criterion.** Ranked repeat counts are `267, 14, 13, 11, 10, 10, 10, 9, …` — case 2
    lands at **14**, inside the legitimate band, one place above a legitimate 13. There is no threshold
    that separates it.
- **Option B — Unicode-numeric mask** (`char::is_numeric()`, General_Category `Nd|Nl|No`)
  - Sources: [Rust std `char::is_numeric`](https://doc.rust-lang.org/std/primitive.char.html#method.is_numeric);
    in-repo measurement.
  - Pros: ranked counts become `267, 143, 13, 11, 10, 10, 10, 9, …` — **legitimate max 13, degenerate
    143 and 267**, a gap of more than an order of magnitude. It is a one-predicate change to the
    existing `number_end`, so the grouping/suffix logic is inherited rather than rewritten. It is a
    *principled* class (a Unicode category), not a hand-listed glyph set, so it also covers Arabic-Indic
    and Bengali digits, fractions and roman numerals without enumerating them.
  - Cons: broader masking could in principle merge two genuinely different sentences that differ only
    in a non-ASCII numeric character. Not observed: the legitimate maximum is **13** under both A and B,
    i.e. widening the mask moved **no** legitimate segment.
- **Option C — Vocabulary-free "one-hole" grouping** (largest set of sentences identical except at
  exactly one whitespace-delimited token)
  - Source: in-repo measurement.
  - Pros: catches any varying slot whatever its character class — numbers, glyphs, colour words, dates —
    with no digit predicate at all. Scores both known cases identically to B (267 and 143).
  - Cons: **measurably noisier.** Ranked counts are `267, 143, 28, 19, 10, …` — it introduces two new
    high scorers that B rates at 2, both ordinary discursive prose (`ecc53c3f` seg72 at 28, "the visual
    content is also consistent with the goal of the video, …"; `84149f3b` seg121 at 19, "the presenter
    is also likely looking for potential areas of trend …"). Legitimate max rises 13 → 28, halving the
    headroom below the smallest degenerate count. It also needs an extra minimum-token-count knob.

*Disconfirming evidence sought:* the specific counter-case the PR names — legitimate structured
description that lists drawn levels — was searched for directly and **is present and common**: the
legitimate top of the distribution is exactly that (`- a candlestick near the # level` at 10,
`a horizontal line is drawn at #, labeled "cme gap"` at 9, `- a yellow line at #, labeled "cme gap"`
at 10, a per-frame OHLC listing at 13). So the false-positive risk is real and was quantified rather
than assumed; it tops out at 13. Against Option B specifically, the search was for a legitimate segment
whose skeleton count *rises* under the wider mask; none exists in 11,108 segments.

*Recommendation:* **Option B — `char::is_numeric()`.**
- **Status: proven — by direct measurement on 11,108 visual segments**, with the separation stated
  above and the tokenizer named.
- **Why:** it is the only option that both covers case 2 and leaves the legitimate maximum where
  Option A puts it. Option C covers case 2 too but pays for it with a doubled false-positive ceiling
  and a second threshold, for no measured gain.
- **Risks accepted:** a legitimate segment whose sentences differ only in a non-ASCII numeric character
  more than 24 times would be truncated. None exists in this corpus, and the residue-vs-false-positive
  trade is the same one PR-025 accepted.

---

**Q2: Where does the threshold go?**

*Measured, using the guard implementation's own sentence splitter* (not the measurement path — the two
were run separately and agree):

| cap | segments truncated of 11,108 | which |
|---|---|---|
| 14 | 2 (0.018%) | both known cases |
| 21 | 2 (0.018%) | both known cases |
| 24 | 2 (0.018%) | both known cases |
| 30 | 2 (0.018%) | both known cases |
| 40 | 2 (0.018%) | both known cases |

The false-positive side is **flat across the whole admissible band** — legitimate max is 13, the
smallest degenerate count is 143, so every cap in **[14, 142]** truncates exactly the same two
segments. Unlike PR-025, where the cap sat in a 38-vs-166 gap and the choice was delicate, here the
false-positive edge is not the binding constraint.

**What binds instead is head preservation, and it is now evidence-backed rather than eyeballed.**
Reading `fidelity.json` for the reproducing segment, the drawn-level values are OCR-supported at
positions **1–18 contiguously**, unsupported at 19, supported again at 20 (30,000), and unsupported
from 21 onward, where the round-number ramp begins; negative values start at position 51. So the last
genuinely supported level sits at position **20** and any cap ≤ 20 destroys real, OCR-confirmed content
in the very segment the guard exists to fix — which is verification criterion 4.

*Options for the value:* **21** (minimum residue, one place of margin), **24** (recommended), **40**
(numerical parity with `max_numeric_run`, maximum residue).

*Recommendation:* **cap = 24.**
- **Status: proven — measured.**
- **Why:** it clears the last OCR-supported level (position 20) with four places of margin, sits at
  1.8x the legitimate corpus maximum (13) and 6x below the smallest degenerate count (143), and keeps
  residue low. Measured effect on the reproducing case: 267 lines → 25 kept, of which **all 20
  OCR-supported values survive** and the residue is **4** fabricated ramp lines (29,000 / 28,000 /
  27,000 / 26,000) instead of 247. On case 2, the legitimate 15-item yellow-line list survives intact
  and the 143 white-line claims are cut to 25.
- **Risks accepted:** ~4 fabricated values still reach the timeline in a truncated segment, exactly as
  PR-025 accepted ~29. 40 is *not* chosen despite matching `max_numeric_run`, because it triples the
  residue for no false-positive benefit.

**Q2b: the minimum skeleton length is load-bearing and is a genuine decision, not a constant to
inherit.** `truncate_repetition` hardcodes 15; the natural move is to mirror it. Measured:

| min skeleton length | segments truncated | legitimate max | cases caught |
|---|---|---|---|
| 8 | 3 | 13 | 878, 267, 143 |
| **10** | **3** | **13** | 878, 267, 143 |
| 12 | 2 | 13 | 267, 143 |
| 15 (prior-art parity) | 2 | 13 | 267, 143 |
| 40 | 1 | 11 | **loses case 1** (its skeleton is 31 chars) |

**A value of 10 catches all three degenerate segments at exactly the same legitimate maximum (13) and
the same false-positive count as 15.** It is strictly better on this corpus. Choosing it means the
guard also covers the sub-threshold verbatim loop, which is *not* what § Scope describes — hence it is
carried to Phase 4 as an **Amend** decision rather than taken here. It is also a hardcoded constant
either way, which under the project's config-over-hardcoding rule is the user's call.

---

**Q3: Is templated repetition with a varying slot a recognised degeneration mode with an established
detector?**  *(Tier A probe — 4 searches, 4 primary fetches; not escalated, see below.)*

*Options considered:*

- **Option A — Decode-time samplers: DRY, `no_repeat_ngram_size`, repetition/presence/frequency
  penalties.**
  - Sources: [*Don't Repeat Yourself: Stopping Verbatim Loops at Sampling Time*](https://arxiv.org/html/2608.22761);
    [NVIDIA TensorRT-LLM `no_repeat_ngram_size`](https://github.com/NVIDIA/TensorRT-LLM/issues/492).
  - Pros: prevents the waste rather than trimming it afterwards; DRY reports a 47% reduction in
    suffix-extension across 1.5B–120B models and is implemented in real serving stacks.
  - Cons: **the DRY paper explicitly places this failure mode outside its scope** — "DRY targets exact
    surface-form continuation loops and does not address semantic repetition, discourse-level looping,
    or hallucination." The template-with-varying-slot mode is not exact surface-form continuation, so
    the state-of-the-art decode-time method does not cover it. The same paper states that token-level
    controls "cannot distinguish benign reuse … from the start of a verbatim loop, so they often
    suppress structurally necessary tokens and flatten output diversity" — which is precisely the
    false-positive problem a measured post-hoc threshold avoids. Separately, Ollama does not expose DRY
    or `no_repeat_ngram_size` through the API this project uses, so the option is not available here
    regardless of merit.
- **Option B — Post-hoc n-gram repetition metrics (`rep-n`).**
  - Sources: [*Neural Text Generation with Unlikelihood Training*](https://arxiv.org/pdf/1908.04319)
    (defines `rep-n` = 100 x (1 − |unique n-grams| / |total n-grams|));
    [Dharma AI, *Text Degeneration: A Production Failure Mode*](https://huggingface.co/blog/Dharma-AI/text-degeneration-a-production-failure-mode).
  - Pros: standard, cheap, well-defined; the production write-up's heuristic — "a request that hits the
    configured token cap with n-gram repetition at its tail is a degenerate request" — describes both
    of this corpus's template cases exactly, since **both run to the token cap and end mid-sentence**.
  - Cons: it is a *whole-segment ratio*, so it cannot express "keep the legitimate head, cut the
    excess" — it flags, it does not locate the cut. The same write-up concedes that "heuristics broad
    enough to catch pathological loops will also penalize legitimate outputs that contain natural
    repetition." This is the family compression ratio belongs to, and PR-025 already measured that
    family failing to separate the classes on this corpus.
- **Option C — Bespoke post-hoc masked-skeleton cap (this PR).**
  - Sources: in-repo measurement; prior art `truncate_numeric_run` (`f45ce72`).
  - Pros: separates cleanly on the actual data (13 vs 143/267), locates the cut so the head survives,
    and fits the slot the project already uses.
  - Cons: bespoke, so it carries no external validation; its threshold is corpus-specific and would
    need re-measuring for different content.

*Disconfirming evidence sought:* the search deliberately looked for an existing detector that would
make this PR redundant — queries covering `rep-n` / n-gram repetition thresholds, DRY and
`no_repeat_ngram_size`, degenerative-repetition monitoring, and VLM chart-reading degradation. The
closest hits either explicitly exclude the mode (DRY) or belong to the family already rejected on this
corpus (ratio metrics). One promising source, *Monitor Degenerative Repetition in LLM Agents*
(OpenReview), **could not be retrieved** — the PDF is behind a verification page — so it is recorded as
surfaced-but-unread rather than cited. Corroboration for the *phenomenon* (not the detector) comes from
[*Losing the Plot: How VLM responses degrade on imperfect charts*](https://arxiv.org/pdf/2509.18425),
which reports VLMs continuing numeric sequences and fabricating intermediate values to complete
apparent sequences when reading gridlines and axis markers — the mechanism this corpus exhibits.

*Recommendation:* **Option C**, with Option A recorded as unavailable rather than rejected on merit.
- **Status: convention** for "post-hoc truncation of degenerate generation is standard practice"
  (cited in the Dharma AI production write-up and by this repo's own `6f10acd` prior art);
  **best-guess-given-constraints** for the stronger claim that *no* published detector covers
  templated-slot repetition — a negative that four searches support but cannot establish.
- **Why:** the one state-of-the-art decode-time method names this mode as out of scope, the metric
  family that would otherwise apply was already measured failing on this corpus, and neither can
  preserve a legitimate head.
- **Risks accepted:** a bespoke detector with a corpus-specific threshold and no external validation.
  Mitigated by the size of the measured gap (13 vs 143) and by logging every truncation.
- **Not escalated to Tier B:** the escalation trigger required the answer to change Q1's
  recommendation. It does not — the only alternative that covers this mode is unavailable in Ollama —
  so a deeper round would not alter the decision.

---

**Q4: How is the guard verified, given the metric is blind to case 2 and the prompt that produced
case 1 is not deployed?**

*Answered by executing it.* The guard was implemented offline, applied to copies of both preserved job
directories, and both were re-scored with the **real scorer** (`vtt-client` release build, `rescore`,
which reads `timeline.json` + `ocr.json` and calls `score_segments`):

| | stated | supported | precision | recall | F0.5 |
|---|---|---|---|---|---|
| **case 1** (`2fc10c93`) before | 556 | 294 | **0.529** | 0.330 | 0.472 |
| **case 1** after, cap 24 | 313 | 282 | **0.901** | 0.324 | 0.664 |
| **case 2** (`84149f3b`) before | 310 | 281 | **0.9065** | 0.3201 | 0.6634 |
| **case 2** after, cap 24 | 310 | 281 | **0.9065** | 0.3201 | 0.6634 |

- **Case 1 meets verification criterion 1**: precision recovers to **0.901** against the 0.915 the rest
  of that run scores. Supported facts fall only 294 → 282 and recall only 0.330 → 0.324, so the guard
  removes fabricated residue and leaves real content nearly untouched — the same shape PR-025 measured.
- **Case 2 proves the criterion cannot be used for it.** Removing **12,236 characters** and 118
  fabricated trend-line claims changes **not one** fidelity number. This is Phase 1 finding 4,
  demonstrated rather than argued: the metric scores numbers and labels, and those sentences carry
  neither.

*Recommendation:* keep criterion 1 for case 1 as written and **restate the criterion for case 2** in
terms the evidence supports — characters and sentences removed, legitimate list preserved, verified by
reading. **Status: proven — measured end-to-end with the shipped scorer.**

---

**A provenance finding that changes the PR's premise for the better.** Reading `capture` from both
timelines:

| case | job | prompt sha | PR-026 version |
|---|---|---|---|
| 1 | `2fc10c93` | `c0921846` | **v3** |
| 2 | `84149f3b` | `a4a133fc` | **v2** |

§ Motivation attributes the mode to v3's `Say "a line is drawn at 71,700"` template and says PR-026
"removed that template, which addresses *this* trigger". **Case 2 was produced under v2, which never
contained that template** — and v2 is the version PR-026 is currently weighing as the one to ship.
So the mode is not confined to the v3 prompt, a prompt fix demonstrably did **not** prevent it, and the
PR's own argument that "a prompt fix is not a guard" is stronger than it was written to be. The third
segment (`37a3242c`, the `"You're not."` loop) predates the chart prompts entirely.

**Corpus-wide rate, answering § Notes.** The pre-PR-020 archive contributes 8,611 visual segments across
36 timelines and, contrary to the note's expectation, contains **zero** instances of this mode; its
manifest timestamps (2026-03-31) postdate `6f10acd` (2026-03-29), so it is guard-era for
`truncate_repetition` purposes. All three degenerate segments are in the desktop pool. Corpus-wide rate:
**3 in 11,108 (0.027%)**, or 2 in 11,108 (0.018%) counting only the templated mode. **Scope of that
figure, corrected after the live run (2026-08-25):** it is the rate of *this mode*, detected by *this
detector*. It is **not** the rate of vision degeneration, which is materially higher — a later sweep of
the same 11,108 segments for consecutive repetition of a single word found **423 segments at >=5 and
93 at >=10**, none of which this detector scores above 13. See § Notes.

### Group D: MCP Verification (2026-08-25)

**Schema-Integrity Probe:**

| Claim | Identifier | Canonical documenter | Verified? | Notes |
|---|---|---|---|---|
| Q1 recommendation | `char::is_numeric()` covers `①` | [Rust std `primitive.char`](https://doc.rust-lang.org/std/primitive.char.html#method.is_numeric) | **yes** | Doc defines it as General_Category `Nd\|Nl\|No` and the official example asserts `assert!('①'.is_numeric());` **verbatim**. The single load-bearing identifier in the recommendation, confirmed against the language's own documenter rather than inferred from the measurement. |
| Q1 baseline | `is_ascii_digit()` is ASCII-only | same page + `vtt-core/src/vision.rs:442` | **yes** | Doc directs ASCII parsing to `is_ascii_digit`; the shipped `number_end` uses it, which is why Option A cannot see `①`. |
| Config surface | `vision.max_numeric_run` | `vtt-core/src/config.rs:380`, default `:581` | **yes** | New key sits alongside it under `[vision]`; no name collision. |
| Verification surface | `rescore` reads stored segments | `vtt-client/src/review.rs:282` | **yes** | Confirmed by reading and then by running it — see Q4. |

**Synthesis-Verification Probe:** the recommendation combines four elements — Unicode-numeric masking,
sentence splitting that treats `.` as a terminator only outside a numeric token, a cap on the
most-repeated skeleton, and cut-at-the-(cap+1)-th-occurrence with head preservation and a logged count.

| Combined elements | Cited working example | Verified? | Notes |
|---|---|---|---|
| cap-a-count + cut past the cap + preserve head + log observed count | `truncate_numeric_run`, `vtt-core/src/vision.rs:442` @ `f45ce72` | **yes** | A single working file containing the exact guard shape, in the exact slot, in this repo. |
| Unicode-numeric masking + skeleton keying + that guard shape | — | **no** | No external source uses this exact combination. Labeled **best-guess-given-constraints by citation**, and the gap is flagged. It is instead validated by direct measurement (11,108 segments, 3 caps, plus an end-to-end rescore against the shipped scorer), which is the stronger evidence for a threshold decision and is the standard PR-025 set. |

**Live-state probe (Probe 3 equivalent).** The guard registers behaviour at generation time and is read
back much later from stored timelines, so the analogue of binding-at-creation is: does the guard
actually run in the live path, observably? Phase 1 established the method on the incumbent —
`truncate_repetition` was confirmed live by observing its structural ceiling (max verbatim repeat = 2)
across 1,572 post-guard segments, independently of any log line. **The same probe is the
post-implementation check for this PR**: after deployment, no stored visual segment may exhibit a
masked-skeleton repeat above the configured cap. That is a corpus-level invariant, not an end-to-end
success claim, and it is stated here so implementation carries it.

**Reconciliations:**
- Q1's specified detector (§ Scope, "numeric tokens") was **amended** from `is_ascii_digit` to
  `char::is_numeric` on measurement; the original would have missed case 2.
- The Q3 claim "no published detector covers this mode" was **downgraded** from convention to
  **best-guess-given-constraints** — four searches support it, but one relevant paper was unretrievable
  and a negative of this kind cannot be established by search.
- A **fourth degeneration mode** (sub-threshold verbatim loop, 878 repeats) surfaced during the sweep
  and is **not** covered by § Scope as written; whether to cover it is a one-constant decision carried
  to Phase 4.
- § Motivation's attribution of the mode to the v3 prompt was **corrected** by provenance: case 2 is v2.

### Synthesis (2026-08-25)

**Outcome: Amend** — the detector predicate specified in § Scope misses a known case, and the
threshold the PR left open is now measured. Amendments A1–A6 below were presented to the user and
**approved on 2026-08-25**; the synthesis steps were run only after that decision.

**Changes to this PR** from research:

- **A1 — detector predicate amended.** `is_ascii_digit()` -> **`char::is_numeric()`** (Unicode
  `Nd|Nl|No`). Measured: the ASCII predicate rates case 2 at 14, inside the legitimate band; the
  Unicode predicate rates it 143 against a legitimate maximum of 13, with **no** legitimate segment
  moving. The one-hole alternative covers the same cases but doubles the legitimate ceiling (13 -> 28)
  for no measured gain.
- **A2 — `vision.min_skeleton_chars = 10`, configurable.** Chosen over prior-art parity (15) because it
  catches all three degenerate segments at *identical* legitimate maximum and *identical* false-positive
  count. Made configurable rather than hardcoded, per the project's config-over-hardcoding rule. This
  widens what the PR delivers to include sub-threshold verbatim loops; recorded explicitly in § Scope
  rather than left implicit. The PR file is **not renamed** — the filename is referenced from
  `ROADMAP.md`, `RESEARCH-BACKLOG.md` and `PR-026`, and stability is worth more than a tighter title.
- **A3 — `vision.max_skeleton_repeat = 24` locked.** The PR deliberately left this open. Set from head
  preservation, not from the false-positive gap: the reproducing segment's levels are OCR-supported
  through position 20, so 24 clears the last real value with four places of margin while sitting 6x
  below the smallest degenerate count.
- **A4 — verification criterion 1 split into 1a/1b/1c.** Proven for the numeric variant
  (0.529 -> 0.901); **impossible** for the non-numeric variant, where removing 12,236 characters moved
  no fidelity number. Criterion 1b is restated in terms the evidence can supply.
- **A5 — § Motivation and § Notes corrected.** The mode was attributed to the v3 prompt's template;
  case 2 is **v2** (`a4a133fc`), the version PR-026 is weighing for shipping, which never contained it.
  § Notes expected the pre-PR-020 archive might contain the mode; it contains zero instances.
- **A6 — follow-up recorded, not opened as a blocking PR.** See below.

**Changes to `docs/ARCHITECTURE.md`:** **none in this commit, deliberately.** The § Fidelity Diagnostic
line "**Two** run at generation time" becomes three — but that documents behaviour that does not exist
yet, and `docs/CONSTRAINTS.md` forbids phantom implementations. The edit is carried as a verification
criterion so it lands in the **implementation** commit, alongside the code it describes.

**Changes to `docs/CONSTRAINTS.md`:** none. No new hard rule; the two operative ones
(Segments Are Immutable After Merge, config-over-hardcoding) already cover this work and both are
satisfied — the guard edits at generation time, and both thresholds are config keys.

**New PRs that must come first:** **none.** Both dependencies are landed and unchanged. One
**follow-up** is recorded in `docs/0.0/RESEARCH-BACKLOG.md`: the fidelity diagnostic extracts only
numbers and labels, so it scored 25 facts from a segment carrying 144 fabricated trend-line claims and
did not move at all when they were removed. That under-counting is a defect in the *diagnostic*;
nothing in this PR blocks on it, and fixing it carries its own calibration burden.

**Research-backed details now locked in this PR:**

| detail | value | basis |
|---|---|---|
| mask predicate | `char::is_numeric()` (`Nd\|Nl\|No`) | measured 11,108 segments; Rust std docs verified in Group D |
| sentence split | `.` is a terminator only outside a numeric token; work in original offsets | measured (naive mask collapses 275 sentences to 8) |
| `vision.max_skeleton_repeat` | **24** (`0` disables) | legitimate max 13, degenerate 143/267/878; head OCR-supported through position 20 |
| `vision.min_skeleton_chars` | **10** | catches 3 of 3 at the same legitimate max as 15; 40 loses case 1 |
| order in the guard chain | third, after `truncate_repetition` and `truncate_numeric_run` | settled by construction in Phase 1 |
| expected effect, case 1 | precision 0.529 -> 0.901, recall 0.330 -> 0.324 | offline `rescore` with the shipped scorer |
| expected effect, case 2 | 15,905 -> 3,669 chars; fidelity unchanged | offline `rescore` with the shipped scorer |
| corpus-wide rate **of this mode** | 3 in 11,108 (0.027%) | deduplicated sweep across all three pools; NOT the rate of degeneration generally |

**Invented specifics removed:** the original § Scope's "numeric tokens masked" (wrong predicate) and its
unspecified "threshold ... choosing a value in the gap" (the gap is real but is not what sets the value)
are both replaced with the measured details above.

### Gate Check (2026-08-25)

- **Premise still valid: ✓ — strengthened, not weakened.** Research did not undercut the PR; it found
  two further instances (three degenerate segments, not one), showed the specified detector would have
  missed one of them, and established by provenance that a prompt fix does **not** prevent the mode —
  the second case was produced under v2, which never contained the enumeration template. The PR's own
  argument that "a prompt fix is not a guard" is better supported than when it was written.
- **No prerequisite PRs surfaced: ✓.** Both dependencies (PR-023 `f570aec`, PR-025 `f45ce72`) are
  landed and unchanged. One non-blocking follow-up — the fidelity diagnostic's blindness to
  non-numeric fabrication — is recorded in `docs/0.0/RESEARCH-BACKLOG.md`, not opened as a PR.
- **Downstream contracts: ✓ none** (verified by `grep -rl` in Phase 1 and unchanged since).
- **User approved updated spec: ✓ (2026-08-25)** — amendments A1–A6 presented at the Phase 4 outcome
  branch and approved; Gate Check approved.
- **Implementation cleared: ✓**

**Carried into implementation** (each already has a verification criterion above):

1. Reuse `number_end` with `char::is_numeric()`; do **not** mask the whole string and split that —
   masking is not length-preserving, and a naive mask that swallows the sentence-final period collapses
   275 sentences into 8. Both failure modes were reproduced in Phase 1 and are the reason the criterion
   exists.
2. `docs/ARCHITECTURE.md` § Fidelity Diagnostic "**Two** run at generation time" -> three, **in the
   implementation commit**, not before.
3. The post-implementation live-state probe from Group D: after deployment, no stored visual segment
   may show a masked-skeleton repeat above the configured cap — a corpus-level invariant, checked the
   way `truncate_repetition` was checked in Phase 1, independently of any log line.
4. Expect case 2's fidelity numbers to be **identical** before and after truncation. That is the
   measured, correct outcome — not evidence the guard failed.

### Implementation Validation (2026-08-25)

`truncate_skeleton_repeat` runs third at generation time, after `truncate_repetition` and
`truncate_numeric_run`, and logs every cut. **241 tests pass** (baseline 233 + 8 new), including one
that feeds the function the reproducing segment's shape verbatim and one that pins the new char-based
tokenizer against `truncate_numeric_run`'s byte-based one on ASCII input, so the two cannot drift.

**Verified with the shipped Rust guard, not the research model.** Both preserved jobs were re-scored
offline with the release `vtt-client`; the research figures were reproduced to the digit:

| | stated | supported | precision | recall | F0.5 |
|---|---|---|---|---|---|
| case 1 (`2fc10c93` seg 6) before | 556 | 294 | **0.5288** | 0.3301 | 0.4720 |
| case 1 after | 313 | 282 | **0.9010** | 0.3240 | 0.6644 |
| case 2 (`84149f3b` seg 7) before | 310 | 281 | **0.90645** | 0.32006 | 0.66337 |
| case 2 after | 310 | 281 | **0.90645** | 0.32006 | 0.66337 |

Log lines: `[vision] batch N repeated one sentence skeleton 267 times (cap 24), truncated from 11323
to 1778 chars` and `... 143 times (cap 24), truncated from 16381 to 3719 chars`.

- **Case 1 recovers to 0.901** against the 0.915 the rest of that run scores. Supported facts fall only
  294 -> 282 and recall only 0.330 -> 0.324: the guard removes fabricated residue and leaves real
  content nearly untouched, the same shape PR-025 measured.
- **Case 2 is unchanged in every fidelity number** — the predicted, correct outcome. 12,662 bytes of
  fabricated trend-line claims removed, and the metric that exists to catch fabrication did not move.
  This is why criterion 1b is phrased in characters and reading rather than precision.
- Case 2's byte counts (16,381 -> 3,719) exceed the research round's character counts (15,905 ->
  3,669) because the guard logs `str::len()` in bytes and the circled glyphs are 3-byte UTF-8. Same
  behaviour, different unit; consistent with `truncate_numeric_run`'s existing log.

**Corpus-wide false-positive check, re-run with the Rust implementation** over the same 11,108
guard-era visual segments (94 timelines, deduplicated, 4 pre-guard timelines excluded by the Phase-1
`>=3 verbatim repeats` marker):

| setting | segments truncated | which |
|---|---|---|
| `max_skeleton_repeat = 24`, `min_skeleton_chars = 10` (defaults) | **3** (0.027%) | 878, 267, 143 |
| `max_skeleton_repeat = 24`, `min_skeleton_chars = 15` | 2 (0.018%) | 267, 143 |
| `max_skeleton_repeat = 0` | 0 | guard disabled corpus-wide |

The Rust guard reproduces the research measurement exactly, including the third case that
`min_skeleton_chars = 10` exists to catch. No legitimate segment is touched at any setting.

**Residue, stated honestly.** Cap 24 keeps 24 occurrences, so the reproducing segment still carries
**4** fabricated ramp values (29,000 / 28,000 / 27,000 / 26,000) instead of 247, and case 2 still
carries 24 fabricated trend-line claims instead of 143. A lower cap would remove more residue at the
cost of the OCR-supported head, which ends at position 20. The cap is set for the head-preservation
side of that trade, and the log line records every cut.

**A known property, not a defect.** The cut discards everything after the (cap+1)-th occurrence,
including any legitimate prose that follows the repeated block — the same behaviour
`truncate_repetition` (which breaks) and `truncate_numeric_run` (which cuts) already have. In case 2
the legitimate 15-item yellow-line list happens to precede the ramp and survives; had it followed, it
would have been lost. Ordering the guard any other way would require deleting from the middle of model
output, which is a larger change to what "faithful to model output" means than this PR should make.

### Live Deployment (2026-08-25)

Deployed to the RTX 4090 desktop and run end-to-end. Job `046cd326-5473-4c74-ae17-56afed903b2e`,
`2024_6_24_5-20.mp4`, `market-research` profile, prompt `cfab896e` (v3.1), 459.7s, adaptive sampling
at 16-20 frames per chunk.

**The Group D post-deployment invariant is discharged.** Worst masked-skeleton repeat across the
stored timeline's 10 visual segments is **9**, against the cap of 24 and the measured legitimate
maximum of 13. All three guards are live: `truncate_repetition` fired 4x and `truncate_numeric_run`
once (41 numbers at cap 40, cutting one batch 13,130 -> 479 chars). Whole-video precision 0.863,
recall 0.351, F0.5 0.668.

**Two degeneration modes this guard does NOT cover were found in that same run**, and they are the
reason the rate figure above has been scoped rather than left standing:

- **Mode 5 — intra-sentence word repetition.** Segment 6, 16,644 chars: *"The presenter draws a light
  light light light light light light light light light light light light light light yellow line from
  point ④ to point ③."* **1,140 of the segment's 3,140 words are the token `light`**, across 126
  sentences. Every sentence is unique, no numeric run exists, and masking numbers does not collapse
  the sentences (the repeat count differs), so all three guards are blind *by construction* — the
  same structural argument this PR makes about the other two, now applied to it.
- **Mode 6 — templated ramp with a WORD slot.** Segment 8, 13,701 chars: *"the presenter's cursor
  moves to a point near the end of April 2033, and then to a point near the end of May 2033"* —
  `near the end of` appears **215** times and the year marches **2024 -> 2033** on a 2024 video.
  This is *this PR's own mode* with a non-numeric, non-glyph slot: `char::is_numeric()` cannot mask a
  month name, so the skeletons differ and the count stays at 9. Widening the mask further is not the
  answer (the vocabulary is unbounded); this needs a different detector.

Both are under-counted by the fidelity diagnostic exactly as case 2 was — 16,644 chars yield 22 scored
facts (precision 0.909) and 13,701 yield 28 (0.643) — which is further evidence for the follow-up
already recorded in `docs/0.0/RESEARCH-BACKLOG.md`.

**Mode 5 is systemic, not a one-off.** Re-sweeping the same 11,108 guard-era segments for the longest
run of one word repeated consecutively: p50 **1**, p99 **8**, max **1,214**, with **423 segments at
>=5** and **93 at >=10**; seven segments exceed 400. The p99-of-8 against a degenerate floor in the
hundreds suggests a clean threshold, but setting it is its own research round.

**What this does and does not say about PR-028.** The guard does what it was measured to do: it is
installed in the right place, it fires on nothing legitimate, and the invariant holds. It does not
make vision output trustworthy, and this run is direct evidence that two further modes remain live at
the shipped configuration.

**Not claimed.** The guard has still never been observed *firing* on a live job — this run produced no
segment above the cap, so the live evidence is of correct non-intervention, not of correct
intervention. Firing is evidenced only offline, against the two preserved jobs, because the prompt that
produced case 1 (v3, `c0921846`) is not deployed and its file no longer exists in git or on the desktop.

---

## Motivation

**A third degeneration mode exists that both shipped guards are structurally blind to.** Found
2026-08-25 while measuring a vision-prompt revision (PR-026), on job
`2fc10c93-ec66-4602-8259-ee016ee0de1e`, clip `2024_4_8_5-20`, visual segment 6:

> "A horizontal line is drawn at 38,720. A horizontal line is drawn at 33,907. A horizontal line is
> drawn at 32,276. A horizontal line is drawn at 31,145. A horizontal line is drawn at 30,000. A
> horizontal line is drawn at 29,000. … A horizontal line is drawn at 16,000. …"

It marches down in round steps and runs past zero into **negative prices** (`-2,000` … `-16,000`) for
an asset trading near 70,000. That one segment stated **284 facts of which 239 are unsupported**,
dragging whole-video precision from **0.926 to 0.529**. Excluding it, the same run scores 0.915.

**Why each existing guard misses it, structurally rather than by tuning:**

- **`truncate_numeric_run`** (PR-025, `vision.max_numeric_run = 40`) counts *consecutive numeric
  tokens*. Here the longest consecutive run is **2** — every number is separated by prose. Raising the
  cap cannot help; the signal is absent.
- **`truncate_repetition`** (from `6f10acd`) cuts where a sentence of ≥15 characters recurs a third
  time. Here **every sentence is unique**, because the number differs. Verified live in the same job's
  log: the guard fired on *other* batches (`truncated from 21063 to 1284 chars`), so it is working —
  it simply cannot see this shape.

So the mode is a **repeated sentence template with a varying numeric slot**. PR-025 documented two
modes — an arithmetic ramp of bare numbers, and one value repeated — and this is a third that defeats
both detectors by construction.

**It is prompt-inducible, and a prompt fix demonstrably does not stop it.** The PR-026 prompt that
triggered the case above contained the example `Say "a line is drawn at 71,700"` — handing the model a
canned sentence form for an enumerable feature — and v3.1 removed it. *That is not sufficient*, because
research (Phase 3, provenance) found **a second instance produced under v2** (`a4a133fc`), a prompt
that never contained the template, and which is the version PR-026 is weighing as the one to ship:

> job `84149f3b-4fe3-4390-940a-acc52f29c182`, segment 109, 15,905 chars — **144** sentences of
> `- A white line is drawn from the bottom left to the top right, passing through the point labeled "X".`
> where X ramps through circled glyphs `①②③` … `㊶㊷㊸` … `⑪②⑪③⑪④`.

Two things follow. The varying slot is **not always numeric**, so a detector that masks only ASCII
digits sees this segment at a repeat count of 14 and cannot distinguish it from legitimate
description. And the fidelity diagnostic is nearly **blind** to it: those 144 fabricated claims yield
25 scored facts, and the job's whole-video precision is 0.906 — so this variant costs quality without
moving the metric that would flag it.

**A third segment, and a fourth mode.** The corpus sweep also found job
`37a3242c-5194-4500-8dde-52c41eec5228`, segment 130, in which the model looped on `"You're not."`
**878 times** (a *Back to the Future* clip, reproducing a line of dialogue). That one is verbatim, not
templated — `truncate_repetition` misses it only because the sentence is **10 characters**, under its
`key.len() < 15` gate. It is caught by the same detector at `min_skeleton_chars = 10`, at no measured
false-positive cost, so this PR covers it rather than leaving a known defect to a future round.

## Scope

*Amended by research — see `## Research findings` § Synthesis for what changed and why.*

**In scope:**
- **`truncate_skeleton_repeat`** in `vtt-core/src/vision.rs`: cap how many times one sentence
  **skeleton** may recur within a visual segment. The skeleton is the sentence with its numeric tokens
  masked using **`char::is_numeric()`** (Unicode `Nd|Nl|No`), **not** `is_ascii_digit()` — measured, the
  ASCII predicate rates the known circled-glyph case at 14, inside the legitimate band. Applied
  **third**, after `truncate_repetition` and `truncate_numeric_run`, cutting at the start of the
  (cap+1)-th occurrence so the legitimate head survives, and logging the observed repeat count —
  the shape `truncate_numeric_run` already uses.
- **Sentence splitting must treat `.` as a terminator only when it is not inside a numeric token**, by
  reusing `number_end` and working in original offsets. Masking is **not length-preserving**, so
  masking the whole string and splitting that loses the mapping back to the cut position; and a naive
  mask that swallows the sentence-final period (`71,836.` -> `#`) collapses the reproducing segment
  from 275 sentences to 8 and reports a repeat count of 1. Both measured in Phase 1.
- **`vision.max_skeleton_repeat`** — default **24**, `0` disables. Measured over **11,108** guard-era
  visual segments: legitimate skeleton repeats top out at **13**, degenerate ones are **143** and
  **267** (and **878** for the verbatim variant), so every cap in [14, 142] truncates exactly the same
  segments. 24 is chosen not from that gap but from **head preservation** — the reproducing segment's
  drawn levels are OCR-supported through position 20, so a lower cap destroys real content.
- **`vision.min_skeleton_chars`** — default **10**. Skeletons shorter than this are ignored, as
  `truncate_repetition` ignores sentences under 15 characters. Measured: 10 catches all three
  degenerate segments at the same legitimate maximum (13) and the same false-positive count as 15,
  while 40 would lose the reproducing case entirely (its skeleton is 31 characters).
- A log line per truncation stating the observed repeat count, matching existing guard behaviour.

**Consequence of `min_skeleton_chars = 10`, stated plainly:** the guard covers both the templated mode
this PR is named for **and** verbatim loops that fall under `truncate_repetition`'s 15-character gate.
That is a parameter choice, not a second mechanism — but it is wider than the PR's title, and it is
recorded here rather than left implicit.

**Explicitly out of scope:**
- Changing `truncate_repetition` or `truncate_numeric_run`. Both work on what they were built for;
  this is a third detector, not a replacement.
- Retrying generation. `ollama.temperature = 0` means the existing retry loop reproduces the same
  output — PR-025 established this.
- Editing after merge (forbidden by Segments Are Immutable After Merge). The guard runs at generation
  time, where the other two already live.
- A general-purpose repetition metric. Compression ratio was measured and rejected as a degeneration
  detector in PR-025; re-opening it needs its own justification.

## Dependencies

- **PR-025** — the existing numeric-run guard and the measurement method for setting its threshold.
  Landed `f45ce72`.
- **PR-023** — the fidelity diagnostic, which is how this class becomes visible at all. Landed `f570aec`.

## Architecture section implemented

`docs/ARCHITECTURE.md` — the **Vision output guards** subsection of § Fidelity Diagnostic, which
currently documents two guards and will document three.

## Verification criteria

Criterion 1 is split because research proved it is **not** applicable to all three cases: the fidelity
metric cannot see the non-numeric variant, so demanding a precision recovery there would be demanding
evidence the diagnostic cannot produce.

- [x] **1a — numeric variant.** Job `2fc10c93`, segment 106 is truncated and whole-video precision
      recovers toward the 0.915 the rest of that run scores. *Research measured **0.529 -> 0.901** at
      cap 24 via offline `rescore`; the implementation must reproduce it.*
- [x] **1b — non-numeric variant.** Job `84149f3b`, segment 109 is truncated. Verified by
      **characters and claims removed and by reading**, not by precision: research measured 15,905 ->
      3,669 chars and 143 -> 25 white-line claims, with the legitimate 15-item yellow-line list intact
      and **every fidelity number unchanged**. That last fact is recorded as evidence, not treated as a
      failure.
- [x] **1c — sub-threshold verbatim variant.** Job `37a3242c`, segment 130 (`"You're not."` x878) is
      truncated at the cap.
- [x] Skeleton extraction masks numeric tokens with `char::is_numeric()` and is pinned by tests,
      including a percentage, a suffixed value and a non-ASCII numeric slot (`1.738T`, `-0.70%`, `①`)
- [x] Sentence splitting does not split inside a decimal (`1.738`), pinned by a test
- [x] Threshold measured over every visual segment on disk, with the separation recorded and the
      tokenizer stated explicitly — **done in Phase 3**: 11,108 guard-era segments (94 timelines,
      deduplicated, 4 pre-guard timelines excluded); legitimate max **13**, degenerate **143 / 267 /
      878**; tokenizer = `number_end` with `char::is_numeric()`
- [x] A legitimate list of drawn levels survives intact — specifically, **all 20 OCR-supported values**
      in the reproducing segment's head, and case 2's 15-item yellow-line list
- [x] Truncations are logged with the observed repeat count
- [x] `0` disables the guard; defaults are the measured values (`max_skeleton_repeat = 24`,
      `min_skeleton_chars = 10`)
- [x] `docs/ARCHITECTURE.md` § Fidelity Diagnostic updated from "**Two** run at generation time" to
      three, **in the implementation commit** (not before — documenting an unimplemented guard would be
      a phantom implementation)
- [x] `cargo test --workspace` passes

## Research backing

**Tier-1, all five phases run** (Phase 1 was not clean — the specified detector missed a known case).
Full round in `## Research findings` above. The three questions this section originally posed are
answered:

1. *Does a masked-skeleton match false-positive on legitimate structured description?* **Measured, no.**
   The named counter-case exists and is ordinary — legitimate skeleton repeats top out at **13** across
   11,108 segments (`- a candlestick near the # level` at 10, a per-frame OHLC listing at 13) — and the
   degenerate ones sit at 143 and above.
2. *What separation exists?* **13 vs 143/267/878.** Every cap in [14, 142] truncates the same segments,
   so the false-positive edge is not the binding constraint; head preservation is.
3. *Does the guard interact with `truncate_repetition`, and in what order?* **Settled by construction,
   in Phase 1.** `truncate_repetition` caps verbatim repeats at 2 and cannot mask, so it can never
   pre-empt a skeleton guard; the new guard runs third, on its output.

One external round was run (Q3, Tier A): the state-of-the-art decode-time remedy explicitly excludes
this mode, and is unavailable through Ollama in any case.

## Notes

- Found by PR-026's A/B, which is a point in favour of running prompt changes through the fidelity
  diagnostic even when it is used only as a guardrail: a metric with no calibration still surfaced a
  correctness bug that produced negative Bitcoin prices.
- The v3 run that produced the case was **cancelled mid-flight by user direction** once the defect was
  understood, so only `2024_4_8` has a v3 timeline; `2024_6_24` was cancelled and `2025_05_26` never
  started. The reproducing job is preserved and named above.
- ~~Worth checking during implementation whether the March 2026 pre-PR-020 timelines contain this mode
  too.~~ **Checked in Phase 3: they do not.** The archive contributes 8,611 visual segments across 36
  timelines and contains **zero** instances; its manifest timestamps (2026-03-31) postdate `6f10acd`
  (2026-03-29), so it is guard-era for `truncate_repetition`. All three degenerate segments are in the
  desktop pool. Corpus-wide rate **of this mode**: **3 in 11,108 (0.027%)**, or 2 counting only the
  templated mode. This is not the rate of vision degeneration — see the live-run findings below.
- The fidelity diagnostic scoring 25 facts from 144 fabricated sentences is a defect in the
  **diagnostic**, not the guard. Recorded as a follow-up in `docs/0.0/RESEARCH-BACKLOG.md`; nothing in
  this PR blocks on it.
