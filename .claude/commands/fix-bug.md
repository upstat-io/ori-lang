---
name: fix-bug
description: Fix a bug with full plan-section rigor — root cause analysis, TDD matrix, implementation, TPR review, and impl-hygiene review. Creates a fix-BUG-XX-NNN.md file in the bug tracker. TRIGGER when picking up a bug for fixing from /review-bugs or when explicitly told to fix a specific bug.
allowed-tools: Read, Grep, Glob, Edit, Write, Bash, Agent, AskUserQuestion, TaskCreate, TaskUpdate, TaskList, Skill
argument-hint: "[BUG-XX-NNN or description]"
---

# Fix Bug

Fix a bug with the same rigor as a plan section: investigation, root cause analysis, TDD-first testing, implementation, and full completion checklist including `/tpr-review` and `/impl-hygiene-review`.

## Usage

```
/fix-bug BUG-04-033
/fix-bug BUG-02-005
/fix-bug [description of the bug if ID unknown]
```

## Workflow

### Phase 0: Locate the Bug

1. **Find the bug entry** — read the bug-tracker section files to find the bug:
   ```
   plans/bug-tracker/section-{NN}-*.md
   ```
   If the user gave a description instead of an ID, search all section files for a match.

2. **Extract context** from the entry: severity, repro, subsystem, source, any notes or cross-refs.

3. **Check for an existing fix file** — if `plans/bug-tracker/fix-BUG-XX-NNN.md` already exists, resume from where it left off instead of creating a new one.

### Phase 1: Investigation (Research Before Writing)

**Do NOT start coding yet.** Understand the bug first.

1. **Read the affected code** — not just the file listed in the bug entry, but the surrounding context. Understand the data flow, the invariants, and the phase boundaries. Read at least 2-3 files that interact with the buggy code.

2. **Reproduce the bug** — run the repro from the bug entry. If it passes now, the bug may be OBE — verify and mark resolved if so.

3. **Consult the spec** — check `docs/ori_lang/v2026/spec/` for the intended behavior. The spec is authoritative.

4. **Root cause analysis** — trace the bug to its *root cause*, not just the symptom. Follow the chain:
   - What was observed? (symptom)
   - What code produced this? (proximate cause)
   - Why did that code do the wrong thing? (root cause)
   - Is the root cause localized or systemic? (blast radius)

5. **Check reference compilers** (if the bug involves a design question) — consult `~/projects/reference_repos/lang_repos/` for prior art on how other compilers handle this case.

6. **Identify all affected code paths** — the fix may need changes in multiple places. List every file and function that needs to change.

### Phase 2: Create the Fix Section File

Create `plans/bug-tracker/fix-BUG-{section}-{ordinal}.md` using the template below. This file is the plan section for this bug fix — it documents the investigation, drives the TDD process, and tracks completion.

**IMPORTANT:** Write the fix file BEFORE writing any code. The fix file is the plan; the plan comes before the implementation.

```markdown
---
bug: "BUG-{section}-{ordinal}"
title: "{Bug title from the bug entry}"
severity: "{critical|high|medium|low}"
status: not-started
goal: "{One-line measurable goal — not 'fix X' but 'X correctly produces Y under conditions Z'}"
success_criteria:
  - "{Criterion 1 — specific, testable}"
  - "{Criterion 2 — with verification command}"
subsystem: "{crate/file path}"
found: "{YYYY-MM-DD}"
source: "{tpr-review|code-journey|manual|continue-roadmap|review-work}"
third_party_review:
  status: none
  updated: null
---

# Fix: BUG-{section}-{ordinal} — {Title}

**Status:** Not Started
**Severity:** {severity}
**Goal:** {Expanded goal — what must be true when this fix is complete.}

**Success Criteria:**
- [ ] {Criterion — specific behavioral outcome with verification method}
- [ ] {Criterion — test name, command, or observable result}

**Context:** {Why this bug exists. What pain point it causes. How it was discovered.
Cite the original bug entry. 2-4 sentences.}

---

## 1. Root Cause Analysis

- **Symptom**: {What was observed — error message, wrong output, crash}
- **Proximate cause**: {What code produced the wrong behavior}
- **Root cause**: {Why that code does the wrong thing — the architectural/logical flaw}
- **Blast radius**: {What else is affected — other tests, features, code paths}
- **Affected files**:
  - `{file1}` — {what needs to change and why}
  - `{file2}` — {what needs to change and why}

**Reference implementations** (if applicable):
- **{Language}** `{file path}`: {How they handle this case}

---

## 2. TDD — Test Matrix

Write ALL tests BEFORE the fix. Verify they fail against current code.

### Exact failing case
- [ ] {The specific input that triggered the bug — from the repro}

### Edge cases
- [ ] {Empty, single-element, boundary conditions relevant to this bug}
- [ ] {Additional edge cases}

### Cross-type coverage (if type-dependent)
- [ ] {Test with str}
- [ ] {Test with [int]}
- [ ] {Test with Option<T>}
- [ ] {Test with structs, closures, maps, sets as applicable}

### Cross-pattern coverage (if pattern-dependent)
- [ ] {Test each relevant control-flow pattern}

### Cross-feature interactions
- [ ] {Test interaction with closures, generics, ?, pattern matching, traits as applicable}

### Semantic pin
- [ ] {Test that ONLY passes with the correct/new semantics — the permanent regression guard}

### Negative pin
- [ ] {Test that REJECTS the old/broken behavior — proves the compiler actively prevents regression}

### Verify tests fail before fix
- [ ] All new tests fail against current code (confirming they test the right thing)

---

## 3. Implementation

- [ ] {Describe the fix approach — what changes, where, why}
  ```rust
  // Code sketch showing the target fix (types, signatures, key logic)
  ```
- [ ] {Additional implementation steps if multi-file}
- [ ] {Any co-implementation requirements with other subsystems}

---

## 4. Completion Checklist

- [ ] All new tests pass unchanged after fix (no test modifications needed)
- [ ] Matrix completeness verified — every cell in type x pattern x feature grid has a test
- [ ] Debug AND release builds pass (`cargo b && cargo b --release`)
- [ ] Interpreter and LLVM produce identical results for all new tests (dual-execution parity)
- [ ] `ORI_CHECK_LEAKS=1` reports zero leaks on affected test programs (for memory-touching fixes)
- [ ] `timeout 150 ./test-all.sh` green — no regressions
- [ ] `timeout 150 ./clippy-all.sh` green
- [ ] `cargo test -p {affected_crate}` green
- [ ] Bug entry in `plans/bug-tracker/section-{NN}-*.md` updated: `- [x]` with resolution details
- [ ] Fix section frontmatter `status` updated to `complete`
- [ ] Bug-tracker `00-overview.md` Quick Reference open bug count updated
- [ ] `/tpr-review` passed — independent Codex review found no critical or major issues
- [ ] `/impl-hygiene-review last commit` passed — MUST run AFTER `/tpr-review` is clean

**Exit Criteria:** {Paragraph describing the measurable, testable condition
that proves this fix is complete. Include specific test names, commands,
and what output they produce. Not "X works" but "X produces Y output
when Z command is run, with 0 regressions across the full test suite."}
```

### Phase 3: TDD — Write Tests First

Follow the TDD discipline from the fix section:

1. **Write all matrix tests** from the fix section's TDD plan
2. **Run them and verify they fail** — if any pass, either the bug is OBE or the test doesn't test what you think
3. **Do NOT proceed to implementation until all tests are written and verified failing**

Update the fix section: check off each test as written, note test file paths.

### Phase 4: Implementation

1. **Implement the fix** as described in the fix section
2. **Run the test matrix** — all previously-failing tests should now pass WITHOUT modification
3. If tests need modification after the fix, either the tests were wrong or the fix was wrong — investigate
4. **Run the full suite**: `timeout 150 ./test-all.sh`

Update the fix section: check off implementation tasks, note any discoveries.

### Phase 5: Completion Checklist

Work through the completion checklist in order:

1. **Verify all matrix items** — tests, builds, leak checks
2. **Update the bug entry** in the section file — mark `- [x]` with resolution details
3. **Update the fix section** — set status to `complete`, fill in exit criteria
4. **Update the overview** — adjust open bug count
5. **Run `/tpr-review`** — independent third-party review of the fix
6. **Handle TPR findings** — fix any issues found, re-run if needed
7. **Run `/impl-hygiene-review last commit`** — AFTER TPR is clean

### Phase 6: Report

Report the fix to the user:
```
Fixed: [BUG-{section}-{ordinal}][{severity}] {title}
  Fix section: plans/bug-tracker/fix-BUG-{section}-{ordinal}.md
  Tests added: {count} ({test file paths})
  Files changed: {list}
  TPR: {passed | findings resolved}
  Hygiene: {passed | findings resolved}
```

## Scaling Rules

### Simple bugs (1-2 files, obvious fix)
- The fix section is still MANDATORY — but sections 1-3 can be brief
- TDD matrix can be smaller if the bug is narrowly scoped — but semantic + negative pins are ALWAYS required
- Completion checklist is NEVER shortened

### Complex bugs (3+ files, architectural)
- Full investigation with reference compilers
- Multiple design approaches with tradeoffs documented
- TPR checkpoints during implementation if it spans multiple logical steps
- Consider whether the fix belongs in a proper plan section instead of a bug fix

### Clusters (multiple related bugs)
- If 2+ bugs share a root cause, create a single fix section covering all of them
- Name it after the primary bug: `fix-BUG-04-033.md` with the others listed as "Also fixes: BUG-04-034, BUG-04-035"
- The TDD matrix covers ALL bugs in the cluster

## Integration Points

- **`/add-bug`** — files bugs; references `/fix-bug` for when they're picked up
- **`/review-bugs`** — triages bugs; recommends `/fix-bug` for selected bugs
- **`/create-plan`** — for bugs that turn out to need architectural change, escalate to a proper plan
- **`/tpr-review`** — called during completion checklist
- **`/impl-hygiene-review`** — called during completion checklist, AFTER TPR
