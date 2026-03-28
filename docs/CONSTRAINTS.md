# Constraints

Hard rules that must never be violated. These are enforced by Claude at all times.

---

## Structural Constraints

These apply to every project built with this template.

### No Phantom Implementations (NON-NEGOTIABLE)

A step is NOT complete if:
- A function exists but returns a default, stub, or placeholder value
- A module is declared but never called from the main flow
- Tests only verify something exists, not that it works correctly

Every PR must include:
1. A test that exercises the **actual behavior**
2. Explicit listing of any stubs or TODOs in the PR description
3. Proof of end-to-end data flow where applicable

### Documentation Accuracy (NON-NEGOTIABLE)

Every code change must include corresponding doc updates in the same commit. Before writing docs, check the actual diff — base doc updates on what changed, not memory. Docs must never describe behavior that doesn't exist in code.

### One PR, One Thing

Each PR is a single, reviewable change. No "while I'm here I'll also add..." — that's scope creep. Each PR references the specific section of the architecture it implements.

### Config Over Hardcoding

All configurable values come from config files. Zero hardcoded parameters for behavior that might change. If a value could reasonably vary between environments or over time, it belongs in config.

---

## Domain Constraints

<!-- Define your project-specific non-negotiables here. -->
<!-- These are the rules that, if violated, would make the system fundamentally broken. -->
<!-- Examples: -->
<!-- - No unauthorized access to user data -->
<!-- - All monetary calculations use decimal, never floating point -->
<!-- - Every API endpoint must be authenticated -->
<!-- - No external network calls during unit tests -->
