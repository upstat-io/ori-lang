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

### Step 4: Generate the Proposal

Create the file at `docs/ori_lang/proposals/drafts/<topic>-proposal.md`.

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

## Spec & Grammar Impact

<Which spec clauses and grammar productions are affected>

---

## Prior Art

<How other languages handle this: Rust, Swift, Gleam, Koka, etc.>
```

### Step 5: Purity Analysis

Before writing, apply the purity principle from `.claude/rules/proposals.md`:
- Can this be implemented in pure Ori? If YES, recommend library approach.
- If it requires compiler support, identify the minimal compiler change.

Include the purity assessment in the proposal's Design section or as a note.

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
