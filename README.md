# vibe-rails

Structured vibe coding with Claude. Clone it, start Claude, and build.

> Vibe coding, but on rails — so you don't derail.

## What is this?

A project template that turns Claude Code into a disciplined development partner. No boilerplate code, no language lock-in — just the methodology that makes AI-assisted development actually work.

You clone this repo, start Claude Code, and it asks: **"What do you want to build?"**

From there, Claude guides you through structured design planning, sets up your architecture docs, defines PRs with verification criteria, and keeps everything aligned as the project evolves. You bring the ideas, Claude handles the structure.

## What's included

- **Onboarding flow** — Claude bootstraps your project from a single conversation
- **Design planning procedure** — structured idea → decisions → convergence → docs → implementation
- **PR workflow** — one file per PR with scope, dependencies, and verification criteria
- **Code audit procedure** — post-design alignment checks to catch drift
- **Architecture docs** — living documents that Claude maintains as the project evolves
- **Constraints system** — define your non-negotiables and Claude enforces them

## Quick start

```bash
# Clone the template
git clone https://github.com/YOUR_USERNAME/vibe-rails.git my-project
cd my-project

# Remove template git history, start fresh
rm -rf .git
git init

# Start Claude Code
claude

# Kick it off
> init

# Claude will ask "What do you want to build?" — go from there.
```

## How it works

1. **Clone & init** — you get a skeleton with procedures and doc templates
2. **First conversation** — Claude detects the project is uninitialized, runs the design planning procedure, and asks what you want to build
3. **Design session** — Claude guides you through architecture decisions, captures everything in docs
4. **CLAUDE.md rewrites itself** — the bootstrap instructions are replaced with your project-specific guidance
5. **Build** — PRs are defined, roadmap is set, Claude knows the plan. Start coding.
6. **Iterate** — new design sessions update docs, code audits catch drift, PRs track progress

## Philosophy

- **Enforce structure, not opinions** — the template defines *how* to plan and build, not *what* to build
- **One PR, one thing** — no scope creep, every change is reviewable
- **Docs are living** — architecture docs update with the code, in the same commit
- **No phantom implementations** — if it's not tested and verified, it's not done
- **Design before code** — think first, build second, audit third

## File structure

```
├── CLAUDE.md                        # the brain — drives onboarding + ongoing behavior
├── docs/
│   ├── ARCHITECTURE.md              # project architecture (populated during onboarding)
│   ├── CONSTRAINTS.md               # non-negotiable rules
│   ├── ROADMAP.md                   # PR index with phases and dependencies
│   └── DESIGN-log.md               # design conversation log (created during sessions)
├── prs/                             # one file per PR (created during planning)
├── PROCEDURE-design-planning.md     # how to run a design session
├── PROCEDURE-code-audit.md          # how to audit code after design changes
└── .gitignore
```

## License

MIT
