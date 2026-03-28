# CLAUDE.md

<!-- STATUS: uninitialized -->
<!-- When status is "uninitialized", Claude runs the onboarding flow below. -->
<!-- After onboarding, Claude replaces this entire file with project-specific guidance. -->

## Onboarding Instructions (for Claude)

This is a fresh project created from the **vibe-rails** template. The project has not been initialized yet.

**Trigger**: When the user sends "init", "start", "begin", "hello", "hi", or any first message, check the status marker above. If it says `uninitialized`, ask the user **"What do you want to build?"**

Then follow these steps exactly:

### Step 1: Understand the vision

Ask clarifying questions until you understand:
- What the project does (core purpose)
- Who it's for (target users/audience)
- What tech stack they want (language, frameworks, infrastructure)
- Any hard constraints they already know about

Do NOT start suggesting architecture yet. Just listen and ask questions.

### Step 2: Run the design planning procedure

Once you have enough context, follow `PROCEDURE-design-planning.md` exactly:
- **Idea phase**: restate what you heard, confirm understanding
- **Decisions phase**: work through architectural choices one at a time
- **Convergence phase**: summarize all decisions, get user confirmation
- **Docs phase**: populate the doc templates (see Step 3)

### Step 3: Populate project docs

Based on the design session, fill in:

1. **`docs/ARCHITECTURE.md`** — replace the skeleton with actual architecture decisions:
   - System overview
   - Component/service descriptions
   - Data flow
   - Key abstractions/traits/interfaces
   - Storage strategy
   - Testing strategy

2. **`docs/CONSTRAINTS.md`** — keep the structural constraints, add domain-specific non-negotiables the user defined

3. **`docs/ROADMAP.md`** — create phased PR plan with dependencies. Each PR gets a file in `prs/` with:
   - Scope (what's included)
   - Dependencies (what must be done first)
   - Verification criteria (how to prove it works)

4. **`docs/DESIGN-log.md`** — capture the design conversation (decisions + rationale)

### Step 4: Rewrite this file

Replace this entire `CLAUDE.md` with project-specific guidance. Use this structure:

```markdown
# CLAUDE.md

<!-- STATUS: initialized -->

## What Is [Project Name]

[One paragraph description]

## Build & Test Commands

[Language/framework specific commands]

## Architecture

[Summary + pointer to docs/ARCHITECTURE.md]

## Implementation Status

See `docs/ROADMAP.md` for the ordered PR plan. Full PR descriptions in `prs/`.

## Design References

- `docs/ARCHITECTURE.md` — canonical architecture reference
- `docs/CONSTRAINTS.md` — hard rules
- `docs/ROADMAP.md` — PR index with phases and dependencies
- `prs/` — full PR descriptions
- `docs/DESIGN-log.md` — design conversation log
- `PROCEDURE-design-planning.md` — how to run design sessions
- `PROCEDURE-code-audit.md` — post-design-session code audit

## Constraints

[List non-negotiables from docs/CONSTRAINTS.md]
```

### Step 5: Confirm completion

Tell the user: "Project initialized. Here's what I set up: [summary]. Ready to start on PR-001?"

---

## Ongoing Behavior (post-initialization)

Once initialized, Claude should:

- **Follow the constraints** in `docs/CONSTRAINTS.md` at all times
- **Update docs with code** — every code change includes doc updates in the same commit
- **Use the PR workflow** — work is tracked in `prs/`, status in `docs/ROADMAP.md`
- **Run code audits** after design sessions (per `PROCEDURE-code-audit.md`)
- **Run design planning** for new features (per `PROCEDURE-design-planning.md`)
- **Never create phantom implementations** — if it's not tested, it's not done
- **All configurable values from config files** — zero hardcoded parameters
