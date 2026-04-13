---
name: create-draft-proposal
description: Create a new Ori language draft proposal with consistent structure and required sections. TRIGGER when the user wants to propose a new language feature, stdlib addition, or design change.
argument-hint: "<topic> [one-line description]"
---

# Create Draft Proposal

Create a new draft proposal in `docs/ori_lang/proposals/drafts/` with the canonical format defined in `.claude/rules/proposals.md`.

**Usage:** `/create-draft-proposal <topic> [description]`
- `/create-draft-proposal parallel-iterators` — creates `parallel-iterators-proposal.md`
- `/create-draft-proposal size-binary-units Add IEC binary size units` — creates with description context

## Execution Rules

- **NEVER enter plan mode** — execute inline in the main conversation
- **Read `.claude/rules/proposals.md` FIRST** — it is the SSOT for format, sections, and lifecycle
- **Interactive** — use `AskUserQuestion` to gather requirements before writing

---

## Workflow

### Step 1: Gather Context

If the user provided only a topic name, use `AskUserQuestion` to clarify:
- What problem does this solve? (1-2 sentences)
- Does it require compiler changes or can it be stdlib/library?
- Any known dependencies on other proposals?
- What areas does it affect? (compiler, type system, runtime, stdlib, spec, grammar)

If the user provided a rich description, extract these from context.

### Step 2: Check for Duplicates

Search existing proposals for overlap:

```
Grep for the topic in docs/ori_lang/proposals/ (all subdirectories)
```

If a similar proposal exists:
- **In drafts/**: Ask if the user wants to revise the existing draft or create a new one
- **In approved/**: Ask if this amends or supersedes the existing proposal
- **In rejected/**: Note the prior rejection and ask if circumstances have changed

### Step 3: Dependency Check

Search `docs/ori_lang/proposals/approved/` for any proposals the new one depends on. If dependencies are in `drafts/` (not yet approved), note them in the `Depends On:` field.

### Step 4: Purity Analysis (GATE — before file creation)

**Before creating any file**, apply the purity principle from `.claude/rules/proposals.md`:
- Can this be implemented in pure Ori? If YES, recommend library approach.
- If it requires compiler support, identify the minimal compiler change.
- If it's purely a stdlib addition, the proposal should document that and avoid requesting compiler changes.

Present the purity assessment to the user. If the analysis suggests the feature belongs in stdlib rather than the compiler, use `AskUserQuestion` to confirm the user still wants a proposal (vs. just implementing the library feature directly).

### Step 4.5: CONDITIONAL — Intelligence Prior Art Query

Query the intelligence graph for cross-language prior art relevant to the proposed feature. Results populate a DRAFT Prior Art section that must be verified before inclusion.

1. **Check availability**: Run `scripts/intel-query.sh status` via Bash and parse the JSON output. If the `status` field is not `"ok"`, skip to Step 5 — omit the Prior Art section or populate it manually.

2. **Run queries** (output visible in Claude's context — do NOT capture into shell variables):
   ```
   scripts/intel-query.sh --human search "<proposal topic>" --limit 5
   scripts/intel-query.sh --human compare "<feature concept>" --limit 5
   scripts/intel-query.sh --human symbols "<Ori keyword>" --repo ori --limit 15
   scripts/intel-query.sh --human file-symbols "<likely path fragment>" --repo ori
   scripts/intel-query.sh --human similar "<feature concept or Ori symbol>" --repo rust,swift,go,koka --limit 5
   ```
   Use `symbols` and `file-symbols` to map current Ori surface area and `similar` to find concrete reference implementations to verify in Step 4.5.4.

3. From the results, draft a `## Prior Art` section with structured entries:
   - Which languages implemented this feature
   - What issues arose (link to specific issue numbers from results)
   - What approaches were rejected and why

4. **MANDATORY VERIFICATION** — The drafted Prior Art section is a STARTING POINT, not a finished product. Before including it in the proposal:
   - Verify each referenced issue/PR actually exists and says what the summary claims (intelligence results are for DISCOVERY, not replacement — per `.claude/rules/intelligence.md`)
   - Check referenced source files in `~/projects/reference_repos/lang_repos/` to confirm implementation details
   - Remove any entries that cannot be verified
   - Add entries discovered through manual inspection that intelligence missed

If intelligence is unavailable or returns no results, skip silently. The Prior Art section can be populated manually or omitted for small proposals.

### Step 5: Generate the Proposal

Create the file at `docs/ori_lang/proposals/drafts/<topic>-proposal.md`.

Include the purity assessment in the proposal's Design section or as a dedicated `## Purity Analysis` section.

**Template** (adapt sections based on scope — small proposals may not need every recommended section):

```markdown
# Proposal: <Title>

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** <YYYY-MM-DD>
**Affects:** <comma-separated areas>
**Depends On:** <filenames, if any>

---

## Summary

<2-5 sentences: what this proposal does>

---

## Motivation

<Why this is needed. Concrete examples of the problem. Code samples showing current pain point.>

### The Problem in Practice

```ori
// Show what's painful or impossible today
```

### When This Matters

<Use cases, frequency, who encounters this>

---

## Design

<The solution. Syntax, semantics, type rules, error cases.>

### Syntax

```ori
// Proposed syntax with examples
```

### Semantics

<How it works at compile time and runtime>

### Error Handling

<What errors are produced and when>

---

## Alternatives Considered

### Alternative 1: <name>

<Why this was rejected>

### Alternative 2: <name>

<Why this was rejected>

---

## Purity Analysis

**Can be pure Ori?** <YES/NO/PARTIALLY>
**If not, why:** <reasons requiring compiler support>
**Missing features that would enable purity:** <list>
**Recommendation:** <Proceed as compiler feature / Revise to library / Hybrid>

---

## Spec & Grammar Impact

<Which spec clauses and grammar productions are affected>

---

## Prior Art

<How other languages handle this: Rust, Swift, Gleam, Koka, etc.>
```

### Step 6: Present for Review

After creating the file:
1. Show the user the full proposal
2. Ask if any sections need revision
3. Suggest running `/review-draft-proposal <topic>` for formal review when ready

### Step 7: Commit

If the user approves, invoke `/commit-push` with message:
```
docs(proposal): create <topic> draft proposal

<one-line summary of what the proposal proposes>
```
