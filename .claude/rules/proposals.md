---
paths:
  - "**/proposals/**"
  - "**proposal**"
---

# Ori Language Proposals

Single source of truth for proposal format, naming, lifecycle, and required sections. Referenced by `/create-draft-proposal` and `/review-draft-proposal` skills.

## Proposal Directories

| Directory | Purpose |
|-----------|---------|
| `docs/ori_lang/proposals/drafts/` | In-progress proposals not yet reviewed |
| `docs/ori_lang/proposals/approved/` | Reviewed and approved proposals |
| `docs/ori_lang/proposals/rejected/` | Proposals rejected after review |
| `docs/ori_lang/proposals/superseded/` | Proposals replaced by newer ones |

## Naming Convention

**Filename**: `<topic>-proposal.md` (kebab-case, `-proposal` suffix).

**Exceptions**: Some legacy proposals use `-language-feature`, `-syntax`, or `-revision` suffixes. New proposals MUST use `-proposal.md`. The review skill resolves arguments via: exact path > `<arg>.md` > `<arg>-proposal.md` > basename search in drafts/.

## Required Header

```markdown
# Proposal: <Title>

**Status:** Draft
**Author:** <name> (with <assistant>)
**Created:** YYYY-MM-DD
**Affects:** <comma-separated: Compiler, runtime, type system, standard library, spec (Clause N), grammar>
```

**Optional header fields** (add when applicable):
- `**Approved:** YYYY-MM-DD` (set by review skill on approval)
- `**Depends On:** <filename>, <filename>` (proposals that must be approved first)
- `**Amends:** <filename>` (existing proposal this modifies)
- `**Supersedes:** <filename>` (existing proposal this replaces)

## Required Sections

| Section | Purpose |
|---------|---------|
| `## Summary` | 2-5 sentences. What the proposal does. |
| `## Motivation` or `## Problem Statement` | Why this is needed. Concrete examples of the problem. |
| `## Design` | The solution. Syntax, semantics, type rules, error handling. |

## Recommended Sections

| Section | When to include |
|---------|-----------------|
| `## Alternatives Considered` | When multiple approaches were evaluated |
| `## Spec & Grammar Impact` | When new syntax or semantic changes are proposed |
| `## Roadmap Impact` | When implementation affects the roadmap |
| `## Migration / Breaking Changes` | When existing code would be affected |
| `## Prior Art` | When other languages have solved this problem |

## Lifecycle

```
Draft --> Blocked (if deps missing)
  |          |
  v          v (after deps resolved)
Approved --> Implemented
  |
Draft --> Rejected (if fundamentally flawed)
Draft --> Superseded (if replaced by newer proposal)
```

On approval:
1. Status changes from `Draft` to `Approved`
2. `Approved: YYYY-MM-DD` field added
3. File moves from `drafts/` to `approved/`
4. Roadmap updated via `/create-plan`
5. Propagation audit run for stale references
6. Spec/grammar synced if applicable

## Errata on Approved Proposals

Approved proposals are NOT rewritten. When a later proposal changes assumptions, add an errata section:

```markdown
## Errata (added YYYY-MM-DD)

> **Superseded by [proposal-name]**: [Brief description of what changed
> and why the original reasoning is stale.]
```

## Purity Principle

> **Lean core, rich libraries** -- Compiler implements only constructs requiring special syntax or static analysis. Everything else belongs in stdlib.

| Category | Requires Compiler? |
|----------|-------------------|
| New syntax/keywords | YES |
| Static analysis | YES |
| Built-in type | MAYBE -- could be library with operator traits? |
| Built-in method | MAYBE -- could be extension/impl? |
| Stdlib addition | NO |

For each proposed feature, ask: "Can this be implemented in pure Ori using existing or planned language features?" If YES, it should be library, not compiler.

## Formatting

- Proposal files follow Markdown conventions, NOT spec formatting rules
- Spec files updated as part of approval follow `.claude/rules/spec.md` (ISO/IEC Directives style)
- These are distinct: proposals use tutorial/motivational tone; specs use formal/declarative tone
