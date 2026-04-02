# Review Draft Proposal Command

Review a draft proposal, analyze implications, and (if approved) integrate into the roadmap.

**Usage:** `/review-draft-proposal [proposal-name]`
- With argument: `/review-draft-proposal as-conversion` reviews `as-conversion-proposal.md`
- Without argument: Auto-selects best draft to review

## Execution Rules — MANDATORY

- **NEVER enter plan mode** (`EnterPlanMode`) — execute everything inline in the main conversation
- **NEVER launch background agents** — all sub-skills and commands run inline (foreground), showing progress and waiting for results
- **Sub-skill invocations** use the `Skill` tool directly (e.g., `Skill(skill: "create-plan", args: "...")`) — call them inline, wait for completion, then continue

---

## Core Principle

> **Lean core, rich libraries** — Compiler implements only constructs requiring special syntax or static analysis. Everything else belongs in stdlib.

---

## Workflow Overview

1. Select proposal (if no argument)
2. Read and understand
3. Language purity analysis
4. Dependency analysis
5. Conflict check with approved proposals
6. Present analysis and ask questions
7. Walk through recommendations one-by-one
8. Confirm approval
9. Execute approval workflow (update status, move file)
10. **Add to roadmap** — invoke `/create-plan` in roadmap mode for full research + section creation ← NEW
11. **Propagation audit** — find and fix ALL stale references across repo ← CRITICAL
12. **Sync spec and grammar** (invoke `/sync-spec` and `/sync-grammar`)
13. Verify documentation formatting (final step)

---

## Phase 1: Selection & Reading

### Step 1: Select Proposal (if no argument)

List drafts in `docs/ori_lang/proposals/drafts/` and evaluate:
- **Completeness**: Has Summary, Problem Statement, Design sections?
- **Dependencies**: Are blockers already approved?
- **Impact**: Does it unblock other work?
- **Simplicity**: Simpler = easier to review first

Present selection and confirm with `AskUserQuestion` before proceeding.

### Step 2: Read and Understand

Read the proposal and related spec files. Identify:
- What changes (syntax, types, patterns, stdlib)
- The problem being solved
- Which compiler phases affected
- Dependencies on other proposals

---

## Phase 2: Analysis

### Step 3: Language Purity Analysis

**Classification:**
| Category | Requires Compiler? |
|----------|-------------------|
| New syntax/keywords | YES |
| Static analysis | YES |
| Built-in type | MAYBE — could be library with operator traits? |
| Built-in method | MAYBE — could be extension/impl? |
| Stdlib addition | NO |

**Ask for each feature:** "Can this be implemented in pure Ori using existing or planned language features?"
- YES → Should be library, not compiler
- NO → Identify missing language feature that would enable it

**Present findings:**
```
## Purity Analysis
**Can be pure Ori?** [YES/NO/PARTIALLY]
**If not, why:** [reasons]
**Missing features that would enable purity:** [list with status: exists/draft/missing]
**Recommendation:** [Proceed/BLOCKED/Revise to library]
```

### Step 4: Dependency Analysis

**Check explicit dependencies** (from proposal's `Depends On:` field):
- [approved] — in `proposals/approved/`
- [draft] — in `proposals/drafts/` (review that first)
- [missing] — BLOCKER

**Check implicit dependencies:**
- Uses syntax that doesn't exist?
- Assumes undefined traits?
- Requires unimplemented type features?

**If blockers exist:** Cannot approve. Offer options via `AskUserQuestion`:
1. "Draft blocking proposals" — create drafts, then return
2. "Defer this proposal" — stop, work on dependencies first
3. "Mark as blocked" — add BLOCKED status to proposal

### Step 5: Check Conflicts with Approved Proposals

Grep the draft for potential conflicts:
- Accessor patterns: `.0`, `.1`, `.value`, `.inner`
- Capability naming: `uses Async` (should be `uses Suspend`)
- Any syntax that might overlap with approved proposals

Search `docs/ori_lang/proposals/approved/` for related topics. If conflicts found, present each:
```
### Conflict: [Topic]
**Draft says:** [code]
**Approved `<name>` says:** [code]
```
Ask user to resolve via `AskUserQuestion` before proceeding.

---

## Phase 3: Recommendation

### Step 6: Present Analysis

Present structured analysis (NO recommendation yet):

**Summary:** 2-3 sentences on what proposal does

**Purity Assessment:** Appropriately in compiler vs library?

**Dependency Status:** All satisfied? Any blockers?

**Strengths:**
- Alignment with Ori design philosophy
- Benefits to users
- What it does well

**Concerns:** (any of these that apply)
- Consistency with existing patterns
- Unnecessary complexity
- Unhandled edge cases
- Ambiguous specification
- Implementation burden
- Overlooked alternatives
- Breaking changes
- Incomplete spec/grammar/examples

### Step 7: Ask Clarifying Questions

**Before any recommendation**, use `AskUserQuestion` to resolve:
- Unclear requirements or edge cases
- Design trade-offs with multiple valid approaches
- Scope clarifications
- Purity trade-offs (library vs compiler)

For each question: List recommended option first with "(Recommended)" suffix.

### Step 8: Present Recommendation

**STOP if unresolved blockers exist.** Blocked proposals cannot be approved.

Recommendations:
- **APPROVE** — Ready as-is
- **APPROVE WITH CHANGES** — Good but needs adjustments (list them)
- **BLOCKED** — Has unresolved dependencies
- **DEFER** — Needs more work
- **REJECT** — Fundamentally flawed

### Step 9: Interactive Change Review

For each recommended change, walk through one-by-one:

```
### Change N: [Topic]
**Current:** [code from proposal]
**Recommended:** [suggested change]
**Rationale:** [why better]
**Alternatives:** [if any]
```

Use `AskUserQuestion` for each. Continue until all addressed.

### Step 10: Summarize and Confirm

Present decision summary:
```
## Decisions
| Aspect | Decision |
|--------|----------|
| [Topic] | [Choice] |

## Final Status
**Dependencies:** [All satisfied / BLOCKED by X, Y]
**Purity:** [Pure library / Justified compiler / Needs revision]
**Recommendation:** [APPROVE / BLOCKED / DEFER / REJECT]
```

Ask user via `AskUserQuestion`:
- If blocked: Draft blockers / Defer / Mark as blocked
- If no blockers: Approve / Show updated proposal / Defer / Reject

Only proceed to approval workflow if user confirms approval.

---

## Phase 4: Approval Workflow

Execute only after user confirms approval AND no blockers exist.

### Step 11: Update and Move Proposal

- [ ] Apply all approved changes
- [ ] Update `Status:` from `Draft` to `Approved`
- [ ] Add `Approved: YYYY-MM-DD`
- [ ] Remove any `## Blockers` section
- [ ] `git mv docs/ori_lang/proposals/drafts/<name>-proposal.md docs/ori_lang/proposals/approved/`

### Step 12: Add to Roadmap via `/create-plan` (MANDATORY — Full Process)

**CRITICAL: Do NOT manually add sections to the roadmap.** Invoke `/create-plan` inline using the Skill tool. This runs the full research-first, architecture-second process.

**Invoke inline** — call the Skill tool directly and wait for completion:
```
Skill(skill: "create-plan", args: "add <proposal-name> to roadmap")
```

**Before invoking**, provide context to help the research phase:
- Reference the approved proposal: `proposals/approved/<name>-proposal.md`
- Summarize the key decisions made during proposal review (from Step 10)
- Note any implementation constraints or dependencies identified during analysis

**After it completes** (inline — you will see the output), verify:
- [ ] New section(s) added to roadmap with proper numbering and dependencies
- [ ] Existing sections updated where affected
- [ ] Overview dependency graph and implementation sequence updated
- [ ] Index keywords updated
- [ ] `/review-plan` completed successfully

### Step 13: Propagation Audit — Find and Fix All Downstream/Upstream Drift

**CRITICAL STEP.** When a proposal changes language semantics, naming, behavior, or invariants, the change must propagate to EVERY document and code location that references the old assumptions. Failure to propagate creates silent contradictions that compound over time.

**Why this matters:** The `simplified-bindings-proposal` changed Ori's mutability model from "immutable by default" to "mutable by default with `$` for immutable". That fundamental change was never propagated — leaving contradictions in 5+ documents, 2 approved proposals with stale reasoning, phantom API references (`list.set()`), and misleading compiler error messages. All discovered months later.

**Audit scope — check ALL of these for references to the old behavior:**

| Location | What to check | How to search |
|----------|--------------|---------------|
| **Other approved proposals** | Reasoning, examples, API references that assume old behavior | `grep -r` in `proposals/approved/` for key terms |
| **Spec files** | Sections describing affected semantics | `grep -r` in `docs/ori_lang/v2026/spec/` |
| **Roadmap** | Overview, design pillars, section descriptions | `grep -r` in `plans/roadmap/` |
| **CLAUDE.md files** | Project instructions referencing old behavior | Check root and `.claude/` CLAUDE.md files |
| **Rules files** | `.claude/rules/*.md` files with stale assumptions | `grep -r` in `.claude/rules/` |
| **Compiler error messages** | Error factories, help text, diagnostic strings | `grep -r` in `compiler/` for key terms |
| **Test skip reasons** | `#skip("...")` comments referencing old behavior | `grep -r '#skip'` in `tests/` |
| **Ori syntax reference** | `.claude/rules/ori-syntax.md` | Read affected sections |

**Procedure:**

1. **Identify key terms** — Extract 3-5 terms/phrases that the proposal changes or invalidates (e.g., "immutable by default", "set() method", "mutable reference")

2. **Search broadly** — For each term, search the entire repo:
   ```bash
   grep -r "term" docs/ plans/ .claude/ compiler/ tests/ --include="*.md" --include="*.rs" --include="*.ori" -l
   ```

3. **Classify each hit:**
   - **STALE** — Directly contradicts the new proposal → must fix
   - **ADJACENT** — References the area but doesn't contradict → review, may need updating
   - **UNRELATED** — Same term, different context → skip

4. **Fix all STALE references** before proceeding:
   - Update spec language to match new semantics
   - Add errata sections to approved proposals that had stale reasoning (do NOT rewrite approved proposals — add errata)
   - Update roadmap descriptions
   - Update compiler error messages and help text
   - Update test skip reasons

5. **Document what was updated** — List all files changed in the propagation audit. This becomes part of the commit message.

**Errata format for approved proposals:**
```markdown
## Errata (added YYYY-MM-DD)

> **Superseded by [proposal-name]**: [Brief description of what changed and why the original reasoning is stale. Reference the new proposal.]
```

**If the audit reveals more than 10 stale references**, the proposal is touching a fundamental invariant. Use `AskUserQuestion` to confirm the user wants to proceed with all fixes before making changes.

### Step 14: Update Spec, Grammar, and Ori Syntax Reference

If proposal introduces new syntax/types/semantics, **invoke the sync skills**:

**Step 14a: Sync Spec (if semantics changed)**

If the proposal affects language semantics, types, or behavior, invoke inline and wait:
```
Skill(skill: "sync-spec")
```

**Step 14b: Sync Grammar (if syntax changed)**

If the proposal introduces or modifies syntax, invoke inline and wait:
```
Skill(skill: "sync-grammar")
```

**Step 14c: Update Ori Syntax Reference (manual)**

- [ ] Update `.claude/rules/ori-syntax.md` if syntax/types/patterns affected
- [ ] Follow rules in `.claude/rules/ori-lang.md`
- [ ] Verify consistency between spec, grammar.ebnf, and ori-syntax.md

### Step 15: Verify Documentation Formatting

**Before committing**, verify all modified documentation follows formatting rules.

**Check spec files against `.claude/rules/spec.md`:**
- [ ] Uses formal, declarative language (not "you can..." or tutorial tone)
- [ ] Technical terms in _italics_ on first use
- [ ] Syntax in `backticks`
- [ ] Normative keywords used correctly (must, shall, may, error)
- [ ] Informative sections marked with `> **Note:**`
- [ ] No inline EBNF (references grammar.ebnf instead)
- [ ] Examples follow `// Valid` / `// Invalid - reason` format

**Check ori-syntax.md and proposal files against `.claude/rules/ori-lang.md`:**
- [ ] Consistent with spec (no contradictions)
- [ ] Syntax/types/patterns synchronized across all docs

**If formatting issues are found**, present the problem and ask for resolution:

```
### Formatting Issue: [Location]
**Current text:**
> [problematic text]

**Issue:** [what rule is violated]

**Option A (Recommended):** [formal/correct version]
**Option B:** [alternative if applicable]

Examples of correct format:
- Spec style: "An associated type default applies when the impl omits the type."
- Tutorial style (AVOID): "You can omit the associated type if you want to use the default."
```

Use `AskUserQuestion` to resolve each formatting issue before proceeding.

**Common formatting scenarios:**

| Wrong | Right |
|-------|-------|
| "You can use `Self` to..." | "`Self` refers to..." |
| "This is useful for..." | "This enables..." |
| "When you write `type T = Self`..." | "The syntax `type T = Self`..." |
| "Don't forget to..." | "It is an error if..." |

### Step 16: Commit and Push

Invoke inline and wait:
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

Proposal: docs/ori_lang/proposals/approved/<name>-proposal.md
```

---

## Final Checklist

**Analysis Phase:**
- [ ] Purity analysis completed — features pushed to library when possible
- [ ] Dependency analysis completed — no unresolved blockers
- [ ] Conflicts with approved proposals checked and resolved
- [ ] Strengths and concerns documented
- [ ] Clarifying questions asked BEFORE recommendation
- [ ] Each change reviewed one-by-one with user
- [ ] Decisions summarized
- [ ] User confirmed approval

**Approval Phase:**
- [ ] Proposal updated with approved changes
- [ ] Moved from `drafts/` to `approved/`
- [ ] Status updated to `Approved`, date added
- [ ] **`/create-plan` invoked in roadmap mode** — full research, architecture, section creation, cohesion check, and `/review-plan` ← CRITICAL
- [ ] Roadmap section(s) created with proper dependencies, sync points, matrix testing strategy
- [ ] Existing roadmap sections updated where affected
- [ ] Overview dependency graph and implementation sequence updated
- [ ] **Propagation audit completed** — searched ALL docs/specs/proposals/roadmap/compiler for stale references ← CRITICAL
- [ ] Errata added to any approved proposals with stale reasoning
- [ ] Compiler error messages updated if they reference old behavior
- [ ] Test skip reasons updated if they reference old behavior
- [ ] **`/sync-spec` invoked** (if affects semantics)
- [ ] **`/sync-grammar` invoked** (if affects syntax)
- [ ] `.claude/rules/ori-syntax.md` updated (if affects syntax/types/patterns)

**Formatting Verification (final step):**
- [ ] Spec files use formal/declarative language (no tutorial tone)
- [ ] Technical terms in italics, syntax in backticks
- [ ] Informative sections marked with `> **Note:**`
- [ ] All docs synchronized (no contradictions between spec/ori-syntax.md/proposal)
- [ ] Formatting issues resolved with user via `AskUserQuestion`
- [ ] Committed and pushed via `/commit-push` (Step 16)

---

## Quick Reference

### Blocker Resolution

If blockers exist, offer three options:
1. **Draft blockers** — Create missing proposals, then return to this review
2. **Defer** — Leave in drafts, work on dependencies separately
3. **Mark blocked** — Add BLOCKED status with dependency list

### Proposal Status Lifecycle

`Draft` → `Blocked` (if deps missing) → `Approved` (after deps resolved) → `Implemented`
`Draft` → `Rejected` (if fundamentally flawed)

### Purity Example

```
Proposal: Duration factory methods as built-ins

Q: Can Duration.from_seconds(s:) be pure Ori?
- Requires Type.method() → Associated functions [approved]
- Requires Duration + Duration → Operator traits [missing]

Conclusion: BLOCKED by operator-traits-proposal
Duration should move to library once operator traits exist.
```
