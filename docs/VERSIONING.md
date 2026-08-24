# Versioning

How this project is versioned, how versions are documented, and what triggers a version change.

This doc is **flat (not version-scoped)** — it describes how versioning itself works, not a snapshot of any one version. Edits supersede in place.

The default policy below is a starting point — replace the bracketed sections during onboarding (`CLAUDE.md` Step 3) once the user has decided their project's bump rules and changelog needs.

---

## 1. Bump-rule triggers

Default: **ZeroVer pre-1.0** with a project-specific user-facing rule for choosing minor vs patch. The bump boundary is **human-decided, never tool-promoted** — every minor bump triggers a design session (§2), so tools that auto-promote patches to minors based on commit messages are forbidden.

| Bump | When | Triggers design session? |
|------|------|--------------------------|
| **MAJOR** (`x → x+1`) | [Project-specific — e.g. public API break, major distribution change, schema break that invalidates persisted user data] | Yes — full `PROCEDURE-design-planning.md` session + migration plan. |
| **MINOR** (`0.x → 0.x+1`) | [Project-specific — typically any user-visible feature] | **Yes** — see §2. |
| **PATCH** (`0.x.y → 0.x.y+1`) | Bug fixes, copy/typo edits, internal refactors with no behavioural change, dependency bumps that don't change output, doc-only edits. | No. |

**v1.0 criteria:** [Project-specific — e.g. public-release milestone, API stabilization, paid-tier launch.]

## 2. Design-planning at every minor/major cut

Every minor or major bump is treated as **starting a fresh project**. Before any implementation PR for the new version lands, run `PROCEDURE-design-planning.md` from Phase 1 — Idea / Decisions / Convergence / Docs — producing a fresh set of PR stubs for the new version's work.

Patch bumps do not require a design session.

Workflow for a minor cut (example: 0.0.x → 0.1.0):

1. Surfacing — a user-visible feature is proposed or a meaningful behavioural change is needed.
2. **Design session** — run `PROCEDURE-design-planning.md` Phases 1-3.
3. **Phase 4 (Docs)** — snapshot the *current* (pre-cut) state into `docs/<current-x.y>/` per §3, then write fresh `docs/<new-x.y>/{DESIGN-log, ROADMAP, RESEARCH-BACKLOG}.md` for the new version. Create PR stubs under `prs/` with monotonic numbers per §4. Update root files (`ARCHITECTURE.md`, `CONSTRAINTS.md`, `CONVENTIONS.md`, etc.) in place — they are SSOT, not snapshots.
4. **Migrate cross-references** — re-point `docs/X.md` references to the new `docs/<x.y>/X.md` paths. A one-shot script (`scripts/rewrite-doc-refs.ts` or equivalent — language and runtime depend on the project's stack) is the recommended tool; see §5.
5. **Implement PRs** — each runs `PROCEDURE-pr-research.md` before its implementation.
6. **Cut the version** — bump the version source-of-truth files (`package.json`, project-specific version constants), add `## [x.y.z] - YYYY-MM-DD` to `/CHANGELOG.md`, push the merge commit, and tag at that commit (§7).

## 3. Doc-versioning layout (hybrid)

Docs split into **SSOT** (single-source-of-truth — describe the current system; rewritten in place) and **temporal** (append-log or plan-of-record — frozen at each version cut).

| File | Placement | Why |
|------|-----------|-----|
| `docs/ARCHITECTURE.md` | flat | SSOT — describes current system; historical states recoverable from `git log` |
| `docs/CONSTRAINTS.md` | flat | SSOT — non-negotiables apply to current code |
| `docs/CONVENTIONS.md` | flat | SSOT — applies to current codebase |
| `docs/DEPLOYMENT.md` | flat | SSOT operational how-to (when applicable) |
| `docs/VERSIONING.md` | flat | This doc — meta-rule, not a snapshot |
| `docs/<x.y>/DESIGN-log.md` | versioned | Temporal append-log; per-version decisions are the canonical record of that era |
| `docs/<x.y>/RESEARCH-BACKLOG.md` | versioned | Tied to in-flight PR decisions for that version |
| `docs/<x.y>/ROADMAP.md` | versioned | Plan-of-record for that version |

**Default for a new doc added mid-version:** flat. Move to version-scoped only if the doc is explicitly temporal (a log, a roadmap, a research backlog).

## 4. PR-file numbering

Flat monotonic across all versions. `prs/PR-NNN.md` files keep their numbers forever; numbers are never reused.

- A PR scoped pre-cut but landing post-cut keeps its number. Use the `Landed-in: vX.Y.Z` header field for version attribution.
- A PR split mid-implementation across a version cut: the original is marked closed/superseded; new work gets fresh monotonic numbers.
- The `Landed-in:` header is required on every numbered PR file. Use `(not yet landed)` for in-flight or dormant PRs, or `superseded by PR-XXX` for replaced ones.

## 5. Cross-reference migration at a version cut

When temporal docs move into a new `docs/<x.y>/` dir, all references to the old paths across the repo must be updated. Recommended approach: a small one-shot script (idiomatic for the project's stack — Node/TS, Python, Go, etc.) with a canonical mapping table:

```
docs/DESIGN-log.md       → docs/<x.y>/DESIGN-log.md
docs/RESEARCH-BACKLOG.md → docs/<x.y>/RESEARCH-BACKLOG.md
docs/ROADMAP.md          → docs/<x.y>/ROADMAP.md
```

Plus a second pass for the moved files themselves, since their internal relative paths shift one directory deeper (e.g. `../prs/` becomes `../../prs/`).

Author the script the first time it's needed; keep it under `scripts/` and update the mapping table at every subsequent cut. Make it idempotent and provide a `--dry-run` mode so the migration commit's diff is reviewable before applying.

Split the migration into a rename-only commit + a content-fixup commit if any moved file's internal links drop below git's 50% rename-similarity threshold (`git log --follow` won't trace renames otherwise).

No automated link checker watches between cuts in the default workflow. Doc-link rot is detected at the next cut by the script's unmapped-path surface. Add CI-level checks (e.g. `markdown-link-check`, `lychee`) only when the project gains contributors or already has CI for another reason.

## 6. Changelog

Default: [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/), hand-written, at `/CHANGELOG.md` (repo root).

Sections per release (in order, omit empty sections):

- `Added` — new features
- `Changed` — changes in existing functionality
- `Deprecated` — soon-to-be-removed features
- `Removed` — removed features
- `Fixed` — bug fixes
- `Security` — vulnerability-related changes

Entries are user-facing. Each entry ends with a PR number in parentheses. The `[Unreleased]` section accumulates entries as PRs merge; at a version cut, it's renamed to `[x.y.z] - YYYY-MM-DD` and a fresh `[Unreleased]` opens.

### Entry template

```markdown
## [0.2.0] - YYYY-MM-DD

### Added
- [User-facing feature description] (#NN).

### Changed
- [Existing-behavior change description] (#NN).

### Fixed
- [Bug-fix description] (#NN).

[0.2.0]: https://github.com/<owner>/<repo>/compare/v0.1.0...v0.2.0
```

If the project wants tooling instead of hand-written: `changesets` (per-PR changeset files, no auto-promotion) is the leading option that's compatible with the design-planning-per-cut rule. `release-please` is **forbidden** because it auto-promotes minors on `feat:` commits, bypassing the design session requirement.

## 7. Tag and deploy alignment

Version source-of-truth files (`package.json` `version`, language/framework-specific version constants, the git tag, and `CHANGELOG.md`) must always be in sync.

**Tag points at the merge commit that ships the version** — never at an arbitrary earlier commit. If `main` advances past the tag with no version bump, that's fine — the tag still points at the merge commit that shipped the version. Re-tagging is only justified if a version bump is missed; prefer a corrective patch PR over re-tagging.

At every cut:

1. Merge the version-cut PR (containing the version-bump + changelog + relevant doc updates).
2. `git tag v<x.y.z>` at the resulting `main` HEAD.
3. `git push --tags`.
