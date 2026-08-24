# Procedure: PR Research

## When to use

**Before every PR implementation.** No exceptions. This procedure runs even for PRs that were research-backed at design time — state drifts between design and implementation, and research assumptions must be validated before code.

The research-backed-decisions constraint in `docs/CONSTRAINTS.md` requires every architectural and significant design decision to be backed by research from reputable sources. This procedure is how that requirement is enforced per-PR.

## Output location

Research findings are appended to the PR file itself under the `## Research findings` section. The research travels with the PR in git history. Do not discard research output.

## Two distinct "tier" axes (read first — they are different things)

This procedure uses the word "tier" for **two orthogonal axes**. Do not conflate them:

1. **PR-path tier (Tier-1 / Tier-2)** — *how much of this 5-phase procedure a given PR runs.*
   - **Tier-1 (light path):** Phase 1 (State Assessment) is the whole requirement. Phases 2–5 run
     **only if** Phase 1 surfaces drift, an unsatisfiable downstream contract, or a genuinely
     unresearched must-answer question. If Phase 1 is clean, the PR is cleared after Phase 1.
   - **Tier-2 (full path):** all five phases run; the load-bearing questions are researched at depth
     before implementation.
   - Assigned **per-PR** in the PR file's `## Before Implementation` header, and **validated in Phase 1**
     against `docs/0.0/ROADMAP.md` + any cut-plan. If those disagree with the header, or the PR's
     scope/dependents have grown since drafting, **re-tier and flag** — the newer, more specific source
     wins (e.g. a cut-plan that adds sensitive-data scope bumps Tier-1 → Tier-2).
   - SSOT for the light-vs-full rule: `~/.claude/CLAUDE.md` ("Tier-1 PRs get the light path … Tier-2
     PRs … get the full 5-phase path").

2. **Research-depth tier (Tier A / B / C / D / E)** — *how deep the web research goes for a single
   must-answer question.* Assigned in Phase 2, executed in Phase 3. SSOT = `~/.claude/CLAUDE.md`
   ("Research Method (Tiered)"). This is **per-question**, not per-PR.

Mnemonic: **numbers = PR path** (how many phases run), **letters = research depth** (how deep each
question goes). A Tier-1 PR that does reach Phase 3 still runs its questions at Tier-A by default; a
Tier-2 PR pre-assigns the load-bearing questions to Tier-B+.

## Phases

### Phase 1: State Assessment

**Goal**: Establish what is actually true *right now* — not what was assumed when the PR was drafted.

1. Read the PR file completely — scope, dependencies, verification criteria, any prior research findings
2. Read the current codebase around the PR's area — what exists vs. what was assumed
3. **Read the full dependency neighborhood — bidirectionally, found mechanically (not from memory).** Run `grep -rl "PR-<this>" prs/ docs/` to surface **every** PR/doc that references this one — downstream dependents *declare themselves*. Read both directions: **(a) upstream** — every PR this one **depends on** (its `Dependencies:` line); **(b) downstream** — every PR that **depends on this one** or that this PR **unblocks**. For each downstream dependent, extract the concrete **contract** it expects this PR to deliver: its verification criteria that name this PR, its `Dependencies:` rationale, and any scope item that assumes this PR's output. A linchpin PR is *defined by what depends on it* — that contract surface is load-bearing Phase-1 state, not a Phase-4 afterthought. Then also read the last N PRs that **landed** in related areas — what surfaced, what changed.
4. Re-read relevant `docs/ARCHITECTURE.md` + `docs/CONSTRAINTS.md` sections — current locked decisions
5. Check `docs/0.0/DESIGN-log.md` for any decisions that might have drifted
6. **Prior-art audit** — for every file the PR will touch (or every file in the area of code the PR will introduce), run `git log -p <file>` and skim the per-commit diffs. Identify prior empirically-validated patterns in the same area (e.g., a filter expression that was fixed once already — carry that fix forward, don't re-derive). Also survey the last 3–5 PRs that touched related infrastructure (CI workflows, deployment glue, payload shapes, etc.) for known-bad patterns + known-good patterns. The output of this step is a list of "things prior PRs already learned the hard way" that constrains Phase 3 (so research doesn't re-discover them) and Phase 4 (so synthesis doesn't drop them on the floor).

**Output** (appended to PR's `## Research findings` section):

```markdown
### State Assessment (YYYY-MM-DD)

**Current state**:
- [What code/decisions exist now that are relevant to this PR]

**Assumptions at PR draft time**:
- [What the PR spec assumed when it was written]

**Stale assumptions** (where current state disagrees with PR assumptions):
- [Assumption X turned out to be Y]

**New constraints** (learned from prior PRs or codebase evolution):
- [Constraint Z that restricts this PR's options]

**Downstream contracts** (what PRs that depend on this one expect it to deliver — from the bidirectional `grep` sweep in step 3):
- **PR-XXX** → [the concrete contract: the dependent's verification criterion / scope item that consumes this PR's output, AND whether this PR's current scope satisfies it]
- [If genuinely none after the sweep: state "**none** — no PR depends on this one (verified via `grep -rl`)" explicitly. An empty or absent section is a hard miss, not a pass.]
```

**Exit criteria**: stale assumptions flagged, new constraints documented, **and the Downstream-contracts section populated** (every dependent's expectation of this PR recorded with a satisfies-yes/no, or an explicit "none" backed by the `grep` sweep). If stale assumptions are severe enough to change the PR's premise — **or if a downstream contract is not satisfiable by the current scope** — STOP and surface it (loop to `PROCEDURE-design-planning.md` if the premise itself is in doubt).

**Path-tier checkpoint (decides whether Phases 2–5 run):** confirm the PR's Tier-1/Tier-2 assignment
(see "Two distinct tier axes" above) against the PR header, `docs/0.0/ROADMAP.md`, and any cut-plan —
**re-tier and flag if they have drifted**. Then branch:
- **Tier-1 AND Phase 1 is clean** (no premise-changing drift, all downstream contracts satisfiable, no
  unresearched must-answer question surfaced) → the PR is **cleared after Phase 1**; record that and skip
  to the **Phase 5 Gate Check**. Phases 2–4 do not run.
- **Tier-2, OR Phase 1 surfaced research needs** (drift, an open mechanism question, a thin/unverified
  design assumption) → proceed to **Phase 2**.

### Phase 2: Scope the Research

**Goal**: Define exactly what must be answered before this PR can be implemented.

1. List decisions this PR requires that aren't already made
2. Mark each as **MUST-ANSWER** (blocks implementation) or **NICE-TO-HAVE** (explicitly excluded)
3. For each must-answer: define what form a good answer would take (library name + reasoning, concrete threshold, code pattern, etc.)
4. Identify dependencies between questions — if Q2 depends on Q1's answer, sequence them
5. **Plan the research process explicitly, including a depth tier per must-answer question.** This is the bridge to Phase 3: Phase 2 must hand Phase 3 a runnable plan, not just a list of questions.
   - **Assign a research depth tier (A / B / C / D / E)** to every must-answer question, using the **5-tier A–E method** as the SSOT (`~/.claude/CLAUDE.md`, "Research Method (Tiered)"). The tier determines the *mechanism* used in Phase 3:
     - **Tier A** *(probe — default)*: inline main-loop `WebSearch` (1–3 targeted queries) + `WebFetch` of top primary sources; no harness, no sub-agents. Synthesize inline, label findings `proven` / `convention` / `best-guess`, actively search the counter-case. Run Tier A FIRST on every question — even one expected to need a higher tier — as a scoping pass that confirms the right tier.
     - **Tier B / C / D**: the 5-phase research harness at minimum / mid / broad depth — `Workflow({scriptPath: "~/.claude/research-tiers/{light,mid,broad}-research.js", args: "<question>"})`.
     - **Tier E**: the full `deep-research` skill — **operator-approved only** (~$40/run).
   - **Choose tiers by stakes, not by habit.** Default Tier A. Escalate a question to B/C/D only when its answer is load-bearing AND Tier A leaves genuine uncertainty (sources conflict / only secondary sources found / claim unconfirmable). A Tier-1 PR is usually all-Tier-A; a Tier-2 PR typically pre-assigns the load-bearing questions to B+ and runs the full 5-phase path. Record the **reason** for any escalation above A.
   - **Group questions into research rounds** where it helps (e.g. "Round 1: Q1–Q3 parallel Tier-A probes; Round 2: Q4 Tier-A after Q1–Q3; escalate Q2→B if probe inconclusive"). State which round runs which tier.
   - Pure-internal design questions that need no external evidence are marked **"internal — no web round"** and resolved in Phase 4 synthesis (still listed, so the boundary is explicit).
   - **Escalation rule (carried from the global method):** never silently settle on a best-guess on a load-bearing point — flag it and propose escalation to the next tier. Tier E always requires explicit operator approval.

**Output** (appended to PR's `## Research findings` section):

```markdown
### Research Questions

**Must-answer:**
1. [Question] — success criteria: [what a good answer looks like]
2. [Question] — success criteria: [...]

**Dependencies:**
- Q2 depends on Q1 (sequential)
- Q3, Q4 independent (parallel)

**Research plan (depth tiers):**
- Q1 — **Tier A** (probe); rationale: [why A suffices / why escalation reserved]
- Q2 — **Tier A → B if inconclusive**; rationale: [load-bearing + likely-uncertain trigger]
- Q5 — **internal — no web round** (resolved in Phase 4 synthesis)
- Rounds: [e.g. "Round 1 = Q1–Q3 parallel Tier-A; Round 2 = Q4 Tier-A after Round 1"]
- Default tier: A. Escalations above A and their reasons: [list, or "none"]

**Explicitly excluded from this round** (nice-to-have):
- [Question deferred to later PR or later research round]
```

**Exit criteria**: a ranked list of must-answer questions with clear success criteria **and an explicit per-question depth-tier assignment + round plan** that Phase 3 executes verbatim.

### Phase 3: Run the Research

**Goal**: Answer the must-answer questions using reputable sources, presenting evidence for AND against each candidate option without bias.

**Rules** (non-negotiable):

- **Web search is mandatory** for every must-answer question (except those marked "internal — no web round" in the Phase 2 plan). **Run each question at the depth tier assigned in Phase 2**, using that tier's mechanism: **Tier A** = inline main-loop `WebSearch` / `WebFetch` (the driver is the researcher — no sub-agent); **Tier B/C/D** = the harness scripts (`light/mid/broad-research.js`); **Tier E** = the `deep-research` skill (operator-approved). For dependent questions, sequence per the plan; for independent ones at Tier B+, run their harness rounds in parallel. Internal codebase reading, prior design-log citations, or LLM general knowledge alone is never sufficient. If a Tier-A probe leaves a load-bearing point uncertain, escalate that question to the next tier (per the Phase 2 escalation rule) rather than settling on a best-guess.
- **Reputable sources only** (per `docs/CONSTRAINTS.md`): production system documentation, official framework source on GitHub, RFCs, published post-mortems / engineering blog posts from serious teams, battle-tested OSS with meaningful adoption. Not sufficient: marketing pages, random StackOverflow answers, personal blogs without engineering weight, LLM intuition dressed up as "common knowledge."
- **Unbiased presentation**: every finding MUST include at least one alternative / competing option with its own cited evidence — not just support for a preferred answer. Actively search for disconfirming evidence against the leaning option. If genuinely none exists after search, say so explicitly and record the search attempts made.
- **Pros/cons for each option** — structured, cited, with concrete implications (performance, correctness, ergonomics, maintainability, security). Avoid vague adjectives ("clean", "simple") — describe the specific technical tradeoff.
- Parallel agents for independent questions; sequential for dependent ones.
- **Cite-or-flag** (non-negotiable; agent prompts must include this clause verbatim): every specific identifier in a recommendation (payload field name, configuration input, env var, flag, API endpoint, schema field, etc.) must include a source-of-truth file path + line/SHA citation. Every **combination** in a recommendation (filter conjunctions, multi-step procedures, configuration tuples, parameter+value pairings) must include at least one cited working example using the **exact** combination — not a synthesis from multiple files that each contribute one element. If either citation is missing, the finding is labeled `best-guess-given-constraints` and the gap is flagged in the output. Rationale: a common research-agent failure mode is **synthesis**, not fabrication — the agent's individual identifiers are real, but the combination it recommends has no cited example using all of them together.
- **Consumer cross-check (mandatory before labeling any question resolved):** for each must-answer question, identify which downstream PR(s) consume the answer (from the Phase-1 **Downstream contracts** section) and verify the recommendation actually **satisfies that dependent's stated verification criterion**. A finding is not "resolved" because it is internally coherent — only when it meets the independent requirement of whoever depends on it. A tidy, self-consistent answer with no external check is the over-confidence failure mode; check it against the consumer's criterion, not just the sources. (Compare a finding against the independent requirement that consumes it — the same discipline a verification test applies to a formula.)
- Every finding labeled with its epistemic status:
  - **Proven** — widely deployed with direct cited evidence (production source, RFC, post-mortem showing the choice works / fails).
  - **Convention** — common practice in cited production systems, but without rigorous proof. **Must cite at least 2 independent systems using this convention.** Single-source "convention" is not allowed.
  - **Best-guess-given-constraints** — explicitly flagged when evidence is thin or unavailable after genuine search. Not a default. Also the required label whenever the cite-or-flag rule above is not fully satisfied.

**Output** (appended to PR's `## Research findings` section):

```markdown
### Findings

**Q1: [question text]**

*Options considered:*
- **Option A: [name]** — [one-line summary]
  - Sources: [cited URLs]
  - Pros: [concrete list, each tied to a specific technical consequence]
  - Cons: [concrete list, same standard]
- **Option B: [name]** — [one-line summary]
  - Sources: [cited URLs]
  - Pros: [...]
  - Cons: [...]
- [Option C, ...]

*Disconfirming evidence sought:*
- [What counter-arguments / failure modes / known drawbacks were searched for, and what was found — or "none found after searching [specific queries / sources]"]

*Recommendation:* Option [X]
- **Status**: proven / convention / best-guess-given-constraints
- **Why**: [reasoning grounded in the pros/cons above]
- **Risks accepted**: [explicit cons of the chosen option that we're living with]
```

#### Group D: MCP-Verification Round (mandatory before locking Phase 4)

After all dispatched agent groups (A/B/C — whatever the round is organized by) complete, run a **Group D MCP-Verification Round** before locking Phase 4 Synthesis. Group D is the driver's (Claude-the-driver, with MCP tools in-conversation) ground-truth pass on the load-bearing claims from agent research. It is bounded (≤30 min), structured (per the probes below), and recorded explicitly in the PR file.

**Scope filter** — only run probes against claims that are (a) implementation-specific or identifier-specific, (b) option-driving (i.e., the recommendation would change if the claim is false), and (c) not pure methodology. Pure-methodology claims ("long-lived release branches are a known pattern") are out of scope.

**Probe 1 — Schema-Integrity Probe.** For every recommendation that names a specific identifier (payload field name, env var, API endpoint, configuration input, flag, CLI argument, schema field, etc.), verify the identifier exists by reading the **canonical schema documenter** for that surface — the declarative source-of-truth file or interface that the consuming system reads at runtime, pinned at the version the recommendation targets.

If the identifier is NOT present in the canonical documenter, downgrade the recommendation to `best-guess-given-constraints` (or remove the identifier) and re-derive the spec from the documenter. Record the divergence in the PR file's Group D output.

**Probe 2 — Synthesis-Verification Probe.** For every recommendation that **combines** multiple identifiers or steps (filter conjunctions, multi-step procedures, configuration tuples, parameter+value pairings, env+secret combinations), find at least one cited working example that uses the EXACT combination. Independent citations of each element are not sufficient — synthesis from disparate sources is the most pernicious agent-research failure mode (the individual identifiers spot-check pass; only the combination fails).

- Acceptable evidence: a single working file (configuration sample, integration test, production source, runtime declaration) at a named commit SHA that contains all the combined elements together AND is in a context where the combination is required (not coincidental).
- If no such cite is found after a genuine search, the combination is labeled `best-guess-given-constraints` and the gap is flagged. The recommendation may still ship if the user accepts the risk, but the BGGC label is honest about what the research couldn't prove.

**Probe 3 — Binding-at-creation (or equivalent live-state probe).** For PRs that introduce or modify state that gets registered at one lifecycle moment and read back at another (e.g., factory registrations, hook subscriptions, external-system bindings, indexer entries, persisted configuration), include a probe that confirms the binding occurs at the expected lifecycle moment — separately from end-to-end success. Mechanism: introspect the live registered state immediately after the registration event, via whatever surface reflects that state for the implementation in question (MCP tool, API query, in-process inspector, persistence reader).

**Output** (appended to PR's `## Research findings` section):

```markdown
### Group D: MCP Verification (YYYY-MM-DD)

**Schema-Integrity Probe:**

| Claim | Identifier | Canonical documenter | Verified? | Notes |
|---|---|---|---|---|
| Q3 recommendation | `<identifier>` | `<repo/path/to/canonical-documenter>` @ `<SHA>` | yes / no | [if no: how the recommendation was amended] |

**Synthesis-Verification Probe:**

| Claim | Combined elements | Cited working example | Verified? | Notes |
|---|---|---|---|---|
| Q7 filter | `field_a` + `field_b` | `<repo>/<path>` @ `<SHA>` | yes / no | [if no: labeled BGGC + risk accepted] |

**Binding-at-creation (if applicable):** [observation method + result]

**Reconciliations:** [any agent claim downgraded from proven/convention → BGGC, or any recommendation amended in response]
```

**Exit criteria**: every must-answer question has ≥2 cited options with pros/cons, an explicit disconfirming-evidence section, and a recommendation with status label backed by the sources cited. Group D probes have run on every load-bearing claim; any divergences are recorded and either resolved or carried into Phase 4 as Amend candidates.

### Phase 4: Synthesis

**Goal**: Reconcile findings with the PR spec, the codebase (if backfilling an already-merged PR), and the broader project docs.

**Research Outcome Branch** (determine BEFORE running the synthesis steps below):

- **Confirm**: findings support the current spec / shipped code. Proceed with the synthesis steps; update doc sections with the cited findings.
- **Amend**: findings reveal a significant drawback or a better alternative to what's in the spec / shipped code. **STOP the synthesis. Present findings + amendment options to the user** (keep as-is with documented risk / change per cited recommendation / escalate further). Resume only after an explicit user decision.
- **Escalate**: findings invalidate the PR's premise entirely. **STOP.** Loop to `PROCEDURE-design-planning.md`.

Synthesis steps below execute only on a **Confirm** outcome, or on an **Amend** outcome after the user has approved the specific amendment.

1. What changed vs. the PR's original spec? Update the PR file's scope/verification sections
2. Does anything propagate back to `docs/ARCHITECTURE.md` or `docs/CONSTRAINTS.md`? Update them in the same commit
3. Are there new PRs that must come first? Update `docs/0.0/ROADMAP.md` and create new PR files with this procedure marked pending
4. Remove any invented specifics from the PR file — replace with research-backed details

**Output** (appended to PR's `## Research findings` section):

```markdown
### Synthesis

**Outcome**: Confirm / Amend / Escalate — [one-line rationale]

**Changes to this PR** from research:
- [Specific change, e.g., "Library choice locked to X instead of Y", or "none — findings confirm original spec"]

**Changes to ARCHITECTURE.md**:
- [None, or specific section updates committed alongside]

**Changes to CONSTRAINTS.md**:
- [None, or new constraint added]

**New PRs that must come first**:
- [None, or PR-XXX added to roadmap]

**Research-backed details now locked in this PR**:
- [List the specific choices research answered]
```

**Exit criteria**: PR file updated with research-backed specifics; related docs updated if needed.

### Phase 5: Gate Check

**Goal**: Confirm the PR is actually ready to implement.

1. Does research invalidate the PR's premise? → loop to `PROCEDURE-design-planning.md`
2. Did research surface prerequisite PRs? → update `docs/0.0/ROADMAP.md`, implement those first
3. Does the user approve the updated PR spec?
4. If all clear → PR is ready to implement

**Output** (appended to PR's `## Research findings` section):

```markdown
### Gate Check

- Premise still valid: ✓
- No prerequisite PRs surfaced: ✓
- User approved updated spec: ✓ (YYYY-MM-DD)
- Implementation cleared
```

**Exit criteria**: explicit user approval to begin implementation, or user directs a replan.

## Anti-patterns

- **Researching without state assessment** — findings are generic and not grounded in current reality. Phase 1 is non-negotiable.
- **Re-running research that's already locked** — wastes effort. If prior research exists and state assessment shows no drift, skip Phase 3 for that question.
- **Accepting "this is convention" without a source** — convention + no source ≠ research-backed. Label as best-guess-given-constraints and make the risk explicit.
- **Padding scope** — researching nice-to-haves during a PR-focused round. Defer them to their own research round.
- **Drift between research and docs** — when research changes a decision, `ARCHITECTURE.md` / `CONSTRAINTS.md` / `DESIGN-log.md` MUST update in the same commit.
- **Skipping state assessment for research-backed PRs** — state may have drifted. Phase 1 always runs; Phases 2-4 may be light if no drift is found.
- **Confirmation bias** — cherry-picking sources that support a predetermined answer. Phase 3 requires ≥1 alternative with pros/cons, an explicit disconfirming-evidence search, and honest reporting when the preferred option has real drawbacks.
- **LLM intuition dressed as "convention"** — labeling a decision "convention" without cited production-system examples. If only general knowledge supports a claim, it must be labeled `best-guess-given-constraints` and the risk flagged.
- **Silent amendment** — updating code or docs to align with research without explicit user decision when research reveals an amendment is warranted. Phase 4's Outcome Branch is mandatory; the user must be informed and approve any code change the research suggests.
- **One-directional dependency reading** — studying only what a PR depends on (upstream) and not what depends on it (downstream dependents / what it unblocks). A linchpin PR's contract surface is *defined* by its dependents; missing them yields findings that don't satisfy the PRs the work is meant to unblock — and the gap surfaces late (Phase 4 / review) instead of in state assessment. Phase 1 step 3's bidirectional `grep -rl` sweep + the Downstream-contracts section are non-negotiable.

## Time-decay policy

If a PR's research was completed more than the project-defined staleness threshold before implementation begins, re-run **Phase 1 (State Assessment)**. If state assessment surfaces stale assumptions, re-run the relevant parts of Phases 2-4 to update.

The threshold is project-defined. Typical range: weeks to months. Faster-moving ecosystems warrant shorter thresholds. Record the chosen threshold in `docs/CONSTRAINTS.md` or `docs/0.0/RESEARCH-BACKLOG.md`.

## Relationship to other procedures

- `PROCEDURE-design-planning.md` — used when the PR's premise itself is in doubt (Phase 5 loops here). Design planning sets direction with integrated research; PR research validates specifics within that direction.
- `PROCEDURE-code-audit.md` — used after design changes to catch drift between docs and code. PR research catches drift between the PR spec and current state; code audit catches drift between current state and the docs.
