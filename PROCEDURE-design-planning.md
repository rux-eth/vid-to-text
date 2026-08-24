# Procedure: Design Planning

## When to use

Before starting any non-trivial feature, component, or architectural change. The user may say "let's design X", "let's discuss X", or "let's plan X".

Also at every minor/major version cut — see [`docs/VERSIONING.md`](docs/VERSIONING.md). Each `x.y` bump is treated as a fresh project start and runs this procedure from Phase 1 before any implementation PR for the new version lands. Patch bumps do not require a design session.

## Phases

### Phase 1: Idea

**Goal**: Understand what the user wants to build and why.

1. Let the user describe the idea freely
2. Ask clarifying questions — focus on intent, not implementation
3. Restate your understanding back to them
4. Do NOT propose solutions yet

**Exit criteria**: User confirms you understand the problem/goal.

### Phase 2: Decisions (with research)

**Goal**: Work through architectural and design choices one at a time, backed by research.

Every significant design decision must be backed by research from reputable sources (see `docs/CONSTRAINTS.md`). Do not proceed to Phase 3 until every identified decision has research backing.

Structure per decision:

1. **Identify the decision** — what's being decided, why it matters, and what's downstream of it
2. **State what's already known** — prior decisions, settled constraints, trivial answers
3. **Scope the research**
   - What must be answered before the decision can be made? (MUST-ANSWER questions)
   - What form would a good answer take? (library name, concrete threshold, pattern, etc.)
   - What's the dependency graph between questions? (parallel vs sequential)
   - What's explicitly excluded from this round? (scope creep prevention)
4. **Run research rounds** — parallel where independent, sequential where dependent. Cite reputable sources. Label each finding as **proven**, **convention**, or **best-guess-given-constraints**.
5. **Iterate** — research may surface new questions. Spin up additional rounds until every MUST-ANSWER question has a cited answer.
6. **Present findings to the user** — options + tradeoffs, now research-backed
7. **User decides** — may override your recommendation, but overrides without research backing must be explicitly flagged
8. **Record the decision + research trail** — both go into `docs/0.0/DESIGN-log.md`

**Rules**:
- One decision at a time — do not rush through
- No decision is "made" until it's research-backed OR explicitly flagged as unresearched with user acknowledgment
- Flag contradictions between decisions as they emerge
- If a decision invalidates a previous one, surface it immediately
- Distinguish decisions that are easy to change later from those that are hard to reverse

**When research may be skipped**: if a decision is trivial, well-known, or covered by a prior research-backed decision, document the reasoning explicitly. The default is to research; the exception is documented.

**Exit criteria**: every identified design question has a research-backed answer. All decisions recorded with rationale + research trail.

### Phase 3: Convergence

**Goal**: Confirm the full design before touching any docs or code.

1. Present a complete summary of all decisions + the research that backs each
2. Call out any remaining ambiguity
3. Ask: "Does this capture everything? Anything you'd change?"
4. Iterate until the user confirms

Convergence cannot happen until Phase 2 is complete — every decision research-backed or explicitly flagged.

**Exit criteria**: User explicitly confirms the design.

### Phase 4: Docs

**Goal**: Capture decisions in the project's living documents.

1. Update `docs/ARCHITECTURE.md` with architectural decisions
2. Update `docs/CONSTRAINTS.md` if new non-negotiables were established
3. Create/update PR files in `prs/` for any new work items (start from `prs/PR-TEMPLATE.md`)
4. Update `docs/0.0/ROADMAP.md` to reflect new/changed PRs
5. Update `docs/0.0/RESEARCH-BACKLOG.md` to index each new PR's research status
6. Append design conversation summary + research log to `docs/0.0/DESIGN-log.md`

**Rules**:
- Docs describe what IS decided, not the exploration process
- Include rationale for non-obvious decisions (the "why")
- Include research citations when documenting decisions
- If a decision changes existing docs, update them — don't just append

**Exit criteria**: All docs reflect the confirmed design.

### Phase 5: Implementation (optional)

**Goal**: Begin building if the user wants to proceed immediately.

1. Identify the first PR to implement
2. **Run `PROCEDURE-pr-research.md`** before touching code — even if the PR is research-backed at design time, state may have drifted
3. Follow the PR's scope and verification criteria
4. Do not exceed the PR's scope (one PR, one thing)

**Exit criteria**: PR is complete with passing verification criteria.

## Anti-patterns

- **Designing while building** — finish the design session before writing code
- **Skipping research in Phase 2** — "I think" is not research. State-of-the-art may have changed since LLM training.
- **Converging with open questions** — if a decision doesn't have research backing, Phase 3 cannot begin
- **Skipping convergence** — always confirm the full picture before docs/code
- **Scope creep during research** — research only MUST-ANSWER questions, defer nice-to-haves
- **Scope creep during implementation** — if you discover a new design question while building, pause and run a mini design session
- **Docs after the fact** — update docs during Phase 4, not weeks later
