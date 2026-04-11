---
name: review-draft-proposal
description: Review a draft proposal, analyze implications, and (if approved) integrate into the roadmap. Covers purity analysis, dependency checking, conflict detection, and downstream propagation.
argument-hint: "[proposal-name]"
---

# Review Draft Proposal

Review a draft proposal, analyze its implications, and (if approved) execute the full approval workflow: status update, roadmap integration, propagation audit, and spec/grammar sync.

**Usage:** `/review-draft-proposal [proposal-name]`
- With argument: `/review-draft-proposal as-conversion` reviews the matching draft
- Without argument: Auto-selects the best draft to review

**Argument resolution**: exact path > `<arg>.md` > `<arg>-proposal.md` > basename search in `docs/ori_lang/proposals/drafts/`. This handles legacy filenames that don't follow the `-proposal.md` convention.

## Execution Rules

- **NEVER enter plan mode** (`EnterPlanMode`) — execute everything inline
- **NEVER launch background agents** — all sub-skills run inline (foreground)
- **Read `.claude/rules/proposals.md` FIRST** — it is the SSOT for proposal format, lifecycle, and purity principles
- **Sub-skill invocations** use the `Skill` tool directly and wait for completion

---

## Phase 1: Selection & Reading

### Step 1: Select Proposal

If no argument provided, list drafts in `docs/ori_lang/proposals/drafts/` and evaluate:
- **Completeness**: Has Summary, Motivation/Problem Statement, Design sections?
- **Dependencies**: Are blockers already approved?
- **Impact**: Does it unblock other work?
- **Simplicity**: Simpler proposals are easier to review first

Present selection and confirm with `AskUserQuestion` before proceeding.

### Step 2: Read and Understand

Read the proposal and related spec files. Identify:
- What changes (syntax, types, patterns, stdlib)
- The problem being solved
- Which compiler phases affected
- Dependencies on other proposals

---

## Phase 2: Analysis

### Step 3: Purity Analysis

Apply the purity principle from `.claude/rules/proposals.md`. For each proposed feature, classify:

| Category | Requires Compiler? |
|----------|-------------------|
| New syntax/keywords | YES |
| Static analysis | YES |
| Built-in type | MAYBE |
| Built-in method | MAYBE |
| Stdlib addition | NO |

**Ask**: "Can this be implemented in pure Ori using existing or planned language features?"
- YES: Should be library, not compiler
- NO: Identify the missing language feature that would enable it

Present findings:
```
## Purity Analysis
**Can be pure Ori?** [YES/NO/PARTIALLY]
**If not, why:** [reasons]
**Missing features that would enable purity:** [list with status]
**Recommendation:** [Proceed/BLOCKED/Revise to library]
```

### Step 4: Dependency Analysis

**Check explicit dependencies** (from proposal's `Depends On:` or legacy `Depends on:` field — both are equivalent per `.claude/rules/proposals.md` legacy variants):
- [approved] — in `proposals/approved/`
- [draft] — in `proposals/drafts/` (review that first)
- [missing] — BLOCKER

**Check implicit dependencies**: uses syntax that doesn't exist? Assumes undefined traits? Requires unimplemented type features?

**If blockers exist**: Cannot approve. Offer options via `AskUserQuestion`:
1. "Draft blocking proposals" — create drafts first, then return
2. "Defer this proposal" — leave in drafts, work on dependencies
3. "Mark as blocked" — add BLOCKED status

### Step 5: Conflict Check

Search `docs/ori_lang/proposals/approved/` for conflicts:
- Accessor patterns (`.0`, `.1`, `.value`, `.inner`)
- Capability naming overlaps
- Syntax that overlaps with approved proposals

If conflicts found, present each and ask user to resolve via `AskUserQuestion`.

---

## Phase 3: Recommendation

### Step 6: Present Analysis

Present structured analysis (NO recommendation yet):

- **Summary**: 2-3 sentences on what the proposal does
- **Purity Assessment**: Appropriately in compiler vs library?
- **Dependency Status**: All satisfied? Any blockers?
- **Strengths**: Alignment with Ori philosophy, user benefits
- **Concerns**: Consistency, complexity, edge cases, ambiguity, implementation burden, alternatives, breaking changes

### Step 7: Ask Clarifying Questions

**Before any recommendation**, use `AskUserQuestion` to resolve:
- Unclear requirements or edge cases
- Design trade-offs with multiple valid approaches
- Scope clarifications
- Purity trade-offs (library vs compiler)

For each question, list recommended option first with "(Recommended)" suffix.

### Step 8: Present Recommendation

**STOP if unresolved blockers exist.** Blocked proposals cannot be approved.

| Recommendation | Meaning |
|---------------|---------|
| **APPROVE** | Ready as-is |
| **APPROVE WITH CHANGES** | Good but needs adjustments (list them) |
| **BLOCKED** | Unresolved dependencies |
| **DEFER** | Needs more work |
| **REJECT** | Fundamentally flawed |

### Step 9: Interactive Change Review

For each recommended change, walk through one-by-one:

```
### Change N: [Topic]
**Current:** [code from proposal]
**Recommended:** [suggested change]
**Rationale:** [why better]
**Alternatives:** [if any]
```

Use `AskUserQuestion` for each.

### Step 10: Summarize and Confirm

Present decision summary table, then ask user via `AskUserQuestion`:
- If blocked: Draft blockers / Defer / Mark as blocked
- If no blockers: Approve / Show updated proposal / Defer / Reject

Only proceed to Phase 4 if user confirms approval.

---

## Phase 4: Approval Workflow

Execute only after user confirms approval AND no blockers exist.

### Step 11: Update and Move Proposal

Use the **resolved draft path** from argument resolution (Step 1) — do NOT re-derive the filename.

- Apply all approved changes
- Update `Status:` from `Draft` to `Approved`
- Add `Approved: YYYY-MM-DD`
- Remove any `## Blockers` section
- `git mv <resolved-draft-path> docs/ori_lang/proposals/approved/<resolved-basename>`

For example, if `/review-draft-proposal parallel-iterators` resolved to `docs/ori_lang/proposals/drafts/parallel-iterators-proposal.md`, then the move is:
```
git mv docs/ori_lang/proposals/drafts/parallel-iterators-proposal.md docs/ori_lang/proposals/approved/parallel-iterators-proposal.md
```

### Step 12: Add to Roadmap via `/create-plan`

**Do NOT manually add sections to the roadmap.** Invoke `/create-plan` inline:

```
Skill(skill: "create-plan", args: "add <proposal-name> to roadmap")
```

Before invoking, provide context:
- Reference the approved proposal path
- Summarize key decisions from Step 10
- Note implementation constraints or dependencies

After completion, verify:
- New section(s) added with proper numbering and dependencies
- Existing sections updated where affected
- Overview dependency graph updated
- `/review-plan` completed

### Step 13: Propagation Audit

**When a proposal changes language semantics, naming, behavior, or invariants, the change must propagate to EVERY document that references the old assumptions.**

**Audit scope** — search ALL of these for references to old behavior:

| Location | How to search |
|----------|---------------|
| Other approved proposals | Grep in `proposals/approved/` |
| Spec files | Grep in `docs/ori_lang/v2026/spec/` |
| Roadmap | Grep in `plans/roadmap/` |
| CLAUDE.md files | Check root and `.claude/` |
| Rules files | Grep in `.claude/rules/` |
| Compiler error messages | Grep in `compiler/` |
| Test skip reasons | Grep `#skip` in `tests/` |
| Ori syntax reference | Read `.claude/rules/ori-syntax.md` |

**Procedure:**
1. **Identify key terms** — 3-5 terms/phrases the proposal changes or invalidates
2. **Search broadly** — for each term, grep the entire repo
3. **Classify**: STALE (contradicts new proposal, must fix), ADJACENT (review), UNRELATED (skip)
4. **Fix all STALE references** — update spec, add errata to approved proposals (per `.claude/rules/proposals.md` errata format), update error messages, update test skip reasons
5. **Document changes** — list all files changed in the propagation audit

If the audit reveals more than 10 stale references, use `AskUserQuestion` to confirm before making changes.

### Step 14: Sync Spec and Grammar

If the proposal affects language semantics/types/behavior:
```
Skill(skill: "sync-spec")
```

If it introduces or modifies syntax:
```
Skill(skill: "sync-grammar")
```

Update `.claude/rules/ori-syntax.md` if syntax/types/patterns affected. Verify consistency between spec, `grammar.ebnf`, and `ori-syntax.md`.

**Formatting**: Spec files follow `.claude/rules/spec.md` (ISO/IEC Directives style: `shall`/`NOTE`/`EXAMPLE`). Proposal files do NOT follow spec formatting — they use tutorial/motivational tone. Do not apply spec formatting rules to proposals.

### Step 15: Commit and Push

```
Skill(skill: "commit-push")
```

Commit message format:
```
docs(proposal): approve <proposal-name>

- Move from drafts/ to approved/
- Add implementation plan to Section X
- Update roadmap tracking
- Propagation audit: updated N files with stale references
- Update spec ([affected files])
- Update ori-syntax.md with [feature]

Key decisions:
- [Decision 1]
- [Decision 2]

Proposal: docs/ori_lang/proposals/approved/<resolved-basename>
```

---

## Checklist

**Analysis Phase:**
- [ ] `.claude/rules/proposals.md` read (SSOT)
- [ ] Purity analysis completed
- [ ] Dependency analysis completed
- [ ] Conflicts with approved proposals checked
- [ ] Strengths and concerns documented
- [ ] Clarifying questions asked BEFORE recommendation
- [ ] Each change reviewed one-by-one with user
- [ ] User confirmed approval

**Approval Phase:**
- [ ] Proposal updated and moved to `approved/`
- [ ] `/create-plan` invoked for roadmap integration
- [ ] Propagation audit completed
- [ ] Errata added to affected approved proposals
- [ ] `/sync-spec` invoked (if affects semantics)
- [ ] `/sync-grammar` invoked (if affects syntax)
- [ ] `.claude/rules/ori-syntax.md` updated (if affects syntax/types/patterns)
- [ ] Spec formatting verified against `.claude/rules/spec.md`
- [ ] Committed and pushed via `/commit-push`

## Quick Reference

### Blocker Resolution

| Option | Action |
|--------|--------|
| Draft blockers | Create missing proposals, then return to this review |
| Defer | Leave in drafts, work on dependencies separately |
| Mark blocked | Add BLOCKED status with dependency list |

### Proposal Status Lifecycle

`Draft` > `Blocked` (if deps missing) > `Approved` (after deps resolved) > `Implemented`
`Draft` > `Rejected` (if fundamentally flawed)
`Draft` > `Superseded` (if replaced by newer proposal)
