# Procedure: Design Planning

## When to use

Before starting any non-trivial feature, component, or architectural change. The user may say "let's design X", "let's discuss X", or "let's plan X".

## Phases

### Phase 1: Idea

**Goal**: Understand what the user wants to build and why.

1. Let the user describe the idea freely
2. Ask clarifying questions — focus on intent, not implementation
3. Restate your understanding back to them
4. Do NOT propose solutions yet

**Exit criteria**: User confirms you understand the problem/goal.

### Phase 2: Decisions

**Goal**: Work through architectural and design choices one at a time.

1. Identify the key design questions (list them for the user)
2. For each question:
   - Present the options with tradeoffs
   - Give your recommendation with reasoning
   - Let the user decide (they may override your recommendation)
   - Record the decision
3. Flag any contradictions between decisions as they emerge
4. If a decision invalidates a previous one, surface it immediately

**Rules**:
- One decision at a time — don't rush through
- If you don't have enough info to present options, ask for more context
- Distinguish between decisions that are easy to change later and those that are hard to reverse

**Exit criteria**: All identified design questions have answers.

### Phase 3: Convergence

**Goal**: Confirm the full design before touching any docs or code.

1. Present a complete summary of all decisions
2. Call out any remaining ambiguity
3. Ask: "Does this capture everything? Anything you'd change?"
4. Iterate until the user confirms

**Exit criteria**: User explicitly confirms the design.

### Phase 4: Docs

**Goal**: Capture decisions in the project's living documents.

1. Update `docs/ARCHITECTURE.md` with architectural decisions
2. Update `docs/CONSTRAINTS.md` if new non-negotiables were established
3. Create/update PR files in `prs/` for any new work items
4. Update `docs/ROADMAP.md` to reflect new/changed PRs
5. Append design conversation summary to `docs/DESIGN-log.md`

**Rules**:
- Docs describe what IS decided, not the exploration process
- Include rationale for non-obvious decisions (the "why")
- If a decision changes existing docs, update them — don't just append

**Exit criteria**: All docs reflect the confirmed design.

### Phase 5: Implementation (optional)

**Goal**: Begin building if the user wants to proceed immediately.

1. Identify the first PR to implement
2. Follow the PR's scope and verification criteria
3. Do not exceed the PR's scope (one PR, one thing)

**Exit criteria**: PR is complete with passing verification criteria.

## Anti-patterns

- **Designing while building** — finish the design session before writing code
- **Skipping convergence** — always confirm the full picture before docs/code
- **Scope creep during implementation** — if you discover a new design question while building, pause and run a mini design session
- **Docs after the fact** — update docs during the design session, not after
