# Procedure: Code Audit (Post-Design Session)

## When to use

After every design planning session that produces updated design decisions. The user will say "do a code audit" or similar.

## Purpose

Verify that all existing code, docs, and PR definitions are aligned with the updated mental model from the design session. Catch contradictions, stale docs, missing types, and structural gaps before implementation begins.

## Steps

### 1. Gather design decisions

Collect all design decisions from the conversation. These are the "truth" for the audit.

### 2. Read ALL code

Read every source file, every config file, every test in the project. **No stone left unturned.** Do not skip files because they seem irrelevant — misalignments hide in unexpected places.

For each file, note:
- What it currently does (brief)
- Any misalignment with the updated design decisions
- TODOs, stubs, or dead code

### 3. Read ALL docs

Read every doc file in the project. Check for:
- Statements that contradict the updated design
- Descriptions of behavior that no longer matches the plan
- Missing sections for new concepts introduced in the design session

### 4. Produce audit report

Organize by component/module. For each:
- Files read (complete list)
- Misalignments found (with file + line references)
- Stubs and TODOs

End with a cross-cutting summary table of all misalignments, ranked by severity:
- **Critical**: architectural decisions not reflected in code or docs
- **Moderate**: doc/code mismatches, missing primitives
- **Minor**: stubs correctly labeled, cosmetic issues

### 5. Update PR files

If the design session changes the scope or ordering of PRs:
- Create new PR files in `prs/` for any new work items
- Update existing PR files if scope changed
- Renumber if dependencies shifted
- Update `docs/ROADMAP.md` to reference the new PR structure

### 6. Update docs

After audit and PR updates, update:
- `docs/ARCHITECTURE.md` — reflect any architectural changes
- `docs/CONSTRAINTS.md` — add any new constraints from the design session
- `CLAUDE.md` — reflect any structural changes

## Output

The audit report is presented to the user in conversation (not saved to a file). The PR files and doc updates are committed.
