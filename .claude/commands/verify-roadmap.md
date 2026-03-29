# Verify Roadmap Command

Systematically verify AND expand roadmap sections using parallel subagents. Two-phase process: (1) review agents audit existing items and identify gaps against each section's stated mission, (2) update agents apply findings back into the section files — which is the entire point of this command.

## Usage

```
/verify-roadmap [section]
```

- No args: Start from Section 1, Item 1
- `section-4`, `4`: Start from Section 4
- `continue`: Resume from last verified item (if tracking exists)

---

## Core Principle

**Verify what exists. Expand for what's missing. Write it all back.**

This command has two jobs:

1. **Verify** — Audit existing items: do tests pass, are they correct, is matrix coverage adequate, do semantic pins exist?
2. **Expand** — Compare what the section *lists* against what it would *actually take* to fulfill the section's stated goal/mission. Identify missing functionality, missing tests, and missing items. A section that checks every listed box but doesn't achieve its stated goal is incomplete.

For each existing item:
- **Can verify → verified** — Tests pass, feature works, matrix coverage adequate → mark `[x]`
- **Cannot verify → annotate + pending** — Insufficient tests, missing matrix dimensions, no semantic pins → add concrete test tasks, leave `[ ]`
- **Reopen if needed** — A previously-completed `[x]` item that fails matrix/pin checks gets reopened to `[ ]` with specific missing test tasks

For the section as a whole:
- **Read the section's stated goal/mission** — the title, description, and any goal statements
- **Ask: if every listed item were completed, would the goal be fulfilled?** If not, identify what's missing
- **Add missing items** — new `- [ ]` entries for functionality, tests, or work needed to actually achieve the goal
- **Never fix code, never write features** — only verify status, annotate deficiencies, and expand the plan

---

## Workflow

### Architecture: Two-Phase — Review Then Update

The command runs in two distinct phases:

**Phase 1 — Review**: Parallel subagents audit sections and write findings to temp files. Read-only — they do NOT modify section files.

**Phase 2 — Update**: After ALL reviews complete, a second set of parallel agents takes the review findings and applies them to the actual section files. This is the deliverable.

```
Main Context (Supervisor)
│
├── PHASE 1: REVIEW (read-only)
│   ├── Batch 1: Launch review agents for sections 0, 1, 2 (in background)
│   │   ├── Agent: section-00 → writes findings to .verify-results/section-00-results.md
│   │   ├── Agent: section-01 → writes findings to .verify-results/section-01-results.md
│   │   └── Agent: section-02 → writes findings to .verify-results/section-02-results.md
│   ├── Monitor: Verify agents loaded context, audited tests, assessed matrix/pins
│   ├── Batch 2: Next batch of review agents...
│   └── All reviews collected
│
├── PHASE 2: UPDATE (writes section files)
│   ├── Launch update agents in parallel (one per section with findings)
│   │   ├── Agent: reads section-00-results.md → updates section-00-parser.md
│   │   ├── Agent: reads section-01-results.md → updates section-01-type-system.md
│   │   └── Agent: reads section-02-results.md → updates section-02-type-inference.md
│   ├── Each agent: applies status changes, annotations, expansions, new items
│   └── Supervisor: validates updates, resolves conflicts
│
└── Final: Update frontmatter, commit checkpoint
```

**Batch size (Phase 1)**: 3-4 sections per batch (avoids overwhelming system resources with concurrent `cargo test` runs).

**Phase 2 parallelism**: All update agents can run in parallel since they each write to different section files — no test conflicts.

### Phase 1: Review

#### Step 1: Plan Batches

Read all section files. Group into batches of 3-4 sections, ordered by section number. If the user specified a single section, skip batching — just run one agent.

#### Step 2: Launch Review Agent Batch

For each batch, launch parallel `general-purpose` subagents using the Agent tool with `run_in_background: true`. Each agent receives:

1. The section file path
2. The spec directory path (`docs/ori_lang/v2026/spec/`)
3. Instructions to follow the verification protocol below
4. A results output path: `plans/roadmap/.verify-results/section-XX-results.md`

Each agent processes its section items sequentially (items within a section stay sequential to avoid test conflicts).

**MANDATORY: Every agent MUST begin by reading ALL project context.** Before verifying a single item, each agent must read — in full, every line — the following files:

1. `/home/eric/projects/ori_lang/CLAUDE.md` — project instructions (read ALL of it)
2. `/home/eric/projects/ori_lang/.claude/rules/ori-syntax.md` — syntax reference (read ALL of it)
3. Every file in `/home/eric/projects/ori_lang/.claude/rules/` — ALL rules files, every line
4. The spec files relevant to the section being verified

Include this as an explicit instruction in each agent's prompt:
```
BEFORE YOU START: Read these files in full — every single line, no skipping.
Do not start verifying items until you have read ALL of these files.

1. /home/eric/projects/ori_lang/CLAUDE.md (ALL of it — contains testing requirements,
   coding standards, matrix coverage rules, semantic pin requirements)

2. ALL 20 rules files in /home/eric/projects/ori_lang/.claude/rules/ — read every file,
   every line:
   - arc.md, aot.md, cargo.md, compiler.md, diagnostic.md, eval.md, impl-hygiene.md,
     ir.md, llvm.md, ori-lang.md, ori-syntax.md, parse.md, patterns.md, registry.md,
     roadmap.md, runtime.md, spec.md, tests.md, typeck.md, types.md

3. The spec files relevant to the section being verified (from docs/ori_lang/v2026/spec/)

These files contain CRITICAL context: matrix testing requirements, semantic pin expectations,
fix completeness criteria, type system rules, eval/codegen invariants, and test standards.
An agent that skips reading these files WILL produce incorrect verification results.

After reading, report what you loaded at the top of your results file:
  Context loaded: CLAUDE.md (read), rules/*.md (20 files read), spec/clause-N.md (read)
```

This is non-negotiable. An agent that skips reading these files will miss critical context about matrix testing requirements, semantic pin expectations, and fix completeness criteria. The supervisor MUST verify that agent results begin with the "Context loaded" line showing all files were read. If the line is missing or shows fewer than 20 rules files, the agent's results are unreliable — re-run the section.

#### Step 3: Supervisor Monitoring

While review agents run, the main context:

1. **Periodically checks agent output** using Read on the output files
2. **Verifies agents loaded full context** — look for "Context loaded: CLAUDE.md (read), rules/*.md (N files read)" at the top of results. If missing, the agent skipped context loading and its results are unreliable — re-run the section.
3. **Verifies agents are actually reading tests** — look for evidence of file reads, not just "tests pass"
4. **Verifies agents assess matrix coverage** — look for "Matrix assessment" blocks with type/pattern/backend dimensions. An agent that marks items verified without matrix assessment is REJECTED.
5. **Verifies agents check for semantic pins** — look for "Semantic pin:" lines. An agent that marks items verified without identifying a pin is REJECTED.
6. **Verifies agents performed gap analysis** — look for "Gap analysis" block. An agent that doesn't analyze whether listed items fulfill the section's stated goal is INCOMPLETE.
7. **Flags agents that appear to skip any of the above** — if an agent marks items verified without showing context loading, test reads, matrix assessment, pin identification, AND gap analysis, intervene and re-verify
8. **Collects completed results** as agents finish

#### Step 4: Next Batch or Collect

If more review batches remain, go to Step 2. Otherwise, proceed to Phase 2.

**Do NOT apply findings to section files during Phase 1.** Review agents are read-only. All findings accumulate in `.verify-results/` temp files.

---

### Phase 2: Update Section Files

**This is the entire point of the command.** After ALL Phase 1 review agents have completed, launch update agents to write findings back into the actual roadmap section files.

#### Step 5: Launch Update Agents

For each section that has a results file in `.verify-results/`, launch a parallel `general-purpose` agent. Each update agent receives:

1. The review results file: `plans/roadmap/.verify-results/section-XX-results.md`
2. The section file to update: `plans/roadmap/section-XX-*.md`
3. Instructions to apply ALL findings from the review

All update agents can run in parallel — they each modify a different section file, so there are no conflicts.

**Each update agent MUST:**

1. **Read the review results file** in full
2. **Read the current section file** in full
3. **Apply every finding** from the review:
   - Update checkbox status (`[x]` / `[ ]`) based on verification results
   - Add annotations (WEAK TESTS, INCOMPLETE MATRIX, NEEDS PIN, WRONG TEST, STALE TEST, NEEDS TESTS, BUG FOUND, REGRESSION) with specific sub-items
   - Reopen previously-completed `[x]` items that failed matrix/pin checks
   - **Add new `- [ ]` items** for missing functionality identified in gap analysis
   - **Add new `- [ ]` test items** for missing test coverage
   - Add verification dates on verified items: `(verified YYYY-MM-DD)`
4. **Update frontmatter** — status, last_verified date
5. **Preserve existing structure** — don't reorder sections, don't remove items, don't change item wording (only status and annotations)

#### Step 6: Supervisor Validates Updates

After update agents complete, the supervisor:

1. Reads each updated section file
2. Validates that all findings from the results file were applied
3. Checks frontmatter consistency (see TPR consistency checks below)
4. Reports the final summary

#### Step 7: Commit Checkpoint

After validation, commit all section file changes. Report summary to user.

---

## Phase 1: Review Agent Protocol

Each review subagent follows this protocol for its assigned section. Results go to `.verify-results/` — agents do NOT modify section files.

### For Each Item (Sequential within agent)

#### 2a. Identify Verification Method

For each item, determine how to verify it:

| Item Type | Verification Method |
|-----------|---------------------|
| `**Implement**: X` | Find and run related Ori tests |
| `**Rust Tests**: path` | Check if Rust tests exist at path, run them |
| `**Ori Tests**: path` | Run specific Ori test file |
| `**LLVM Support**: X` | Run LLVM-specific tests |
| Generic checkbox | Context-dependent verification |

#### 2b. Find and Run Tests

1. **Find related tests**:
   - Search `tests/spec/` for Ori tests
   - Search Rust test modules for `#[test]`
   - Check `tests/compile-fail/` for error tests

2. **Run tests**:
   ```bash
   # For specific Ori test file
   cargo st tests/spec/path/to/test.ori

   # For Rust tests in a module
   cargo test -p ori_types -- module_name

   # For LLVM tests
   ./llvm-test.sh
   ```

3. **Evaluate result**:
   - Tests exist AND pass → proceed to **2c. Audit Test Quality**
   - Tests exist but fail → **Not verified** (regression)
   - No tests exist → **Cannot verify**

#### 2c. Audit Test Quality

**Every test that passes must be explicitly read and audited.** A passing test is NOT sufficient for verification — the test itself must be correct AND have adequate matrix coverage. For each test file found:

1. **Read the test code** — Open and read every test. No exceptions, no skipping.

2. **Verify correctness against spec**:
   - Does each assertion match the spec's defined behavior?
   - Are expected values correct (not just copied from current output)?
   - Do error tests assert the right error type/message?

3. **Check for test quality issues**:
   - **False positives**: Tests that pass for the wrong reason (e.g., asserting `Ok(_)` without checking the value)
   - **Tautological tests**: Tests that can never fail (e.g., testing that `true == true`)
   - **Wrong assertions**: Expected values that don't match what the spec requires
   - **Missing coverage**: The feature has 5 behaviors but only 1 is tested
   - **Overly broad assertions**: `assert!(result.is_ok())` instead of checking the actual value
   - **Copy-paste errors**: Tests that are duplicates or test the wrong feature
   - **Stale tests**: Tests that reference outdated syntax or removed features

4. **Assess matrix coverage** (see **2c-matrix** below)

5. **Check for semantic pins** (see **2c-pins** below)

6. **Classify the test quality**:

   | Quality | Meaning | Action |
   |---------|---------|--------|
   | **Sound** | Tests correct, assertions match spec, matrix adequate, pins exist | Mark `[x]` |
   | **Weak** | Tests pass but coverage insufficient, assertions shallow, or missing matrix dimensions | Leave `[ ]`, annotate with specific gaps |
   | **No Matrix** | Tests pass for some types/patterns but matrix has uncovered dimensions | Leave `[ ]`, annotate as INCOMPLETE MATRIX |
   | **No Pin** | Tests pass but no semantic pin exists — regression could go undetected | Leave `[ ]`, annotate as NEEDS PIN |
   | **Wrong** | Tests have incorrect assertions or test wrong behavior | Leave `[ ]`, annotate as WRONG TEST |
   | **Stale** | Tests reference outdated syntax/features | Leave `[ ]`, annotate as STALE TEST |

#### 2c-matrix. Matrix Coverage Assessment

**Every feature that touches shared code paths MUST have matrix test coverage.** A test suite that only exercises one type or one pattern through a code path is incomplete, even if it passes.

For each item, identify the **matrix dimensions** that apply:

**Type dimension** — Which types flow through this code path?
- Primitives: `int`, `float`, `bool`, `str`, `char`, `byte`
- Collections: `[int]`, `{str: int}`, `Set<int>`
- Compound: `Option<str>`, `Result<int, str>`, `(int, str)` tuples
- User types: structs, sum types (enums)
- Functions: closures, function values
- Special: `void`, `Never`, `Duration`, `Size`

Not every item needs all types — identify which types are **relevant** to the code path being verified. A numeric operator only needs numeric types. A collection method needs all collection types. A pattern-matching feature needs all matchable types.

**Pattern dimension** — Which control-flow/usage patterns exercise this code path?
- Happy path (basic usage)
- Edge cases: empty, single-element, boundary conditions (0, -1, max_int)
- Error cases: invalid input, type mismatches, overflow
- Control-flow: `break`, `continue`, `yield`, guards, nested loops
- Multi-call: calling the same feature twice in different contexts
- Composition: feature combined with other features (e.g., map inside filter)

**Backend dimension** — Does behavior differ across backends?
- Interpreter (`cargo st`)
- LLVM debug (`cargo test -p ori_llvm`)
- LLVM release (if applicable — FastISel differences)

**How to assess**: For each item, build a mental matrix grid:

```
Example: list.map() method
              | [int] | [str] | [Option<int>] | [struct] | nested [[int]] |
  basic       |  ?    |  ?    |     ?          |    ?     |      ?         |
  empty list  |  ?    |  ?    |     ?          |    ?     |      ?         |
  single elem |  ?    |  ?    |     ?          |    ?     |      ?         |
  with break  |  ?    |       |                |          |                |
  chained     |  ?    |  ?    |     ?          |    ?     |      ?         |
```

Fill in `[x]` for tested, `[ ]` for untested. If >30% of relevant cells are untested, classify as INCOMPLETE MATRIX.

**Reporting matrix gaps**: When annotating, be explicit about which cells are missing:
```markdown
- INCOMPLETE MATRIX: list.map() — 4/20 cells covered
  - [ ] Add test: map with `[str]` (only `[int]` tested)
  - [ ] Add test: map with `[Option<int>]` for RC cleanup verification
  - [ ] Add test: map with empty list `[]`
  - [ ] Add test: map chained with filter (composition)
  - [ ] Add test: map with struct list for field access in transform
```

**Not all items need full matrices.** Simple items (e.g., "parser accepts `let` keyword") may only need a few tests. The matrix assessment scales with the complexity and breadth of the code path. Use judgment — but err on the side of more coverage, not less.

#### 2c-pins. Semantic Pin Verification

**Every non-trivial feature MUST have at least one semantic pin test** — a test that ONLY passes with the correct implementation and would FAIL if the feature were reverted, removed, or incorrectly implemented.

A semantic pin is NOT:
- `assert_eq(1 + 1, 2)` — this tests the `+` operator, not a specific feature's semantics
- `assert(result.is_ok())` — this is too broad; many wrong implementations also return Ok
- A test that could pass with a stub implementation

A semantic pin IS:
- A test that asserts the **specific** output/behavior that distinguishes this implementation from a naive/wrong/missing one
- A test that would fail if you commented out the feature's implementation
- A test that verifies an edge case that only the correct algorithm handles

**How to assess**: For each verified item, ask: "If I reverted the implementation commit for this feature, would at least one test fail with a *meaningful* error that identifies the regression?" If the answer is no, it needs a pin.

**Reporting missing pins**:
```markdown
- NEEDS PIN: iterator.zip() — tests exist but all would pass even if zip returned wrong pairs
  - [ ] Add semantic pin: zip([1,2,3], [4,5,6]) -> assert_eq(result, [(1,4), (2,5), (3,6)])
```

#### 2d. Update Item Status

**If Verified (tests pass, sound, matrix adequate, pins exist):**
```markdown
- [x] **Implement**: Feature X [done] (verified 2026-03-28)
```

**If Not Verified (regression — tests fail):**
```markdown
- [ ] **Implement**: Feature X
  - REGRESSION: Tests exist but fail. Needs investigation.
```

**If Tests Weak (pass but insufficient):**
```markdown
- [ ] **Implement**: Feature X
  - WEAK TESTS: Tests pass but coverage is insufficient
    - [ ] Add test: [specific missing coverage]
    - [ ] Strengthen assertion in [test file]: assert actual value, not just Ok
```

**If Matrix Incomplete (tests pass for some types/patterns but not all relevant ones):**
```markdown
- [ ] **Implement**: Feature X
  - INCOMPLETE MATRIX: [N]/[M] cells covered — missing [specific dimensions]
    - [ ] Add test: [feature] with [missing type] (only [tested type] covered)
    - [ ] Add test: [feature] with [missing pattern] (e.g., empty input, break, chained)
    - [ ] Add test: [feature] with [missing composition] (e.g., nested, combined with Y)
```

**If No Semantic Pin (tests pass but no regression guard):**
```markdown
- [ ] **Implement**: Feature X
  - NEEDS PIN: Tests exist but none would uniquely fail if feature reverted
    - [ ] Add semantic pin: [specific assertion that only correct implementation satisfies]
```

**If Tests Wrong (incorrect assertions):**
```markdown
- [ ] **Implement**: Feature X
  - WRONG TEST: [test file] — [what's wrong]
    - Expected per spec: [correct behavior]
    - Test asserts: [what test currently checks]
```

**If Tests Stale (outdated syntax/features):**
```markdown
- [ ] **Implement**: Feature X
  - STALE TEST: [test file] — references removed/changed syntax
```

**If Cannot Verify (no tests):**
```markdown
- [ ] **Implement**: Feature X
  - NEEDS TESTS: Add verification tests before marking complete
    - [ ] Add test: [specific test description]
    - [ ] Add test: [edge case description]
```

#### 2d-reopen. Reopening Previously Completed Items

**A `[x]` item that fails matrix or pin checks MUST be reopened to `[ ]`.**

Previously-verified items are NOT exempt from matrix and pin requirements. If a section was marked complete but its tests lack matrix coverage or semantic pins, the item is reopened and the section status changes accordingly.

When reopening:
1. Change `[x]` to `[ ]`
2. Remove any `[done]` or `(verified ...)` annotation
3. Add the specific deficiency annotation (INCOMPLETE MATRIX, NEEDS PIN, etc.)
4. Add concrete `- [ ]` sub-items for each missing test
5. Update section frontmatter status from `complete` to `in-progress`

```markdown
# Before (previously verified):
- [x] **Implement**: list.filter() [done] (verified 2026-02-15)

# After (reopened — missing matrix):
- [ ] **Implement**: list.filter()
  - INCOMPLETE MATRIX: 3/15 cells covered — only [int] tested
    - [ ] Add test: filter with [str] list
    - [ ] Add test: filter with [Option<int>] list (RC cleanup on None)
    - [ ] Add test: filter with empty list []
    - [ ] Add test: filter with struct list for field predicate
    - [ ] Add test: filter chained with map (composition)
  - NEEDS PIN: no test uniquely identifies filter vs. identity
    - [ ] Add semantic pin: filter([1,2,3,4], x -> x > 2) == [3, 4]
```

**This is not punitive — it's protective.** An item without matrix coverage is a future regression waiting to happen. Reopening ensures the test gaps are visible and tracked in the planning system, not buried as invisible assumptions.

#### 2e. Section-Level Gap Analysis

**After verifying all individual items, step back and analyze the section as a whole.** This is where expansion happens.

1. **Re-read the section's title, description, and goal/mission statement** — what does this section claim to accomplish?

2. **Ask: "If every listed item (even the unchecked ones) were completed, would this section's stated goal be fulfilled?"**
   - If YES → gap analysis complete, note "No gaps found"
   - If NO → identify what's missing

3. **Identify missing items in these categories:**

   **MISSING FUNCTIONALITY** — features or capabilities not listed but required by the goal:
   ```markdown
   MISSING FUNCTIONALITY (gap analysis):
   - [ ] **Implement**: [feature] — required by section goal "[goal]" but not listed
   - [ ] **Implement**: [feature] — spec clause N.M requires this but section omits it
   ```

   **MISSING TESTS** — test coverage that doesn't exist for any listed item:
   ```markdown
   MISSING TESTS (gap analysis):
   - [ ] **Ori Tests**: tests/spec/path/to/new_test.ori — [what needs testing]
   - [ ] **Rust Tests**: compiler/crate/src/module/tests.rs — [what needs testing]
   ```

   **MISSING ITEMS** — work items needed to bridge gaps (integration, wiring, docs, etc.):
   ```markdown
   MISSING ITEMS (gap analysis):
   - [ ] Wire [feature X] to [feature Y] — both listed separately but integration not tracked
   - [ ] Add error handling for [edge case] — spec requires it, no item covers it
   ```

4. **Check the spec** — read relevant spec clauses for this section's domain. Does the spec define behaviors that no listed item covers?

5. **Check the codebase** — are there TODOs, FIXMEs, or `#[ignore]`/`#skip` markers in the code that relate to this section's domain but aren't tracked?

6. **Report gap analysis results:**
   ```
   Gap analysis for Section 4 — Modules:
     Section goal: "Complete module system with imports, exports, visibility, and resolution"
     Listed items cover: imports, exports, visibility
     GAPS FOUND:
       - Module resolution algorithm not listed (spec clause 8.4 requires it)
       - Re-export chains (`pub use` through multiple modules) — no item or test
       - Circular import detection — mentioned in spec but not tracked
       - 3 #skip tests in tests/spec/modules/ reference untracked issues
     Items to add: 4 functionality, 2 test, 1 integration
   ```

**This is the most important step.** A section where every checkbox is green but the stated goal isn't achievable is worse than one with honest gaps — it creates false confidence. Expand the section to match reality.

#### 2f. Report Progress

After all items + gap analysis, report per-item results and gap summary:
```
V 1.1.1 Primitive int type — VERIFIED (3 tests, matrix 8/8, pin: overflow_panics)
X 1.1.2 Duration arithmetic — INCOMPLETE MATRIX (tests pass but only int+int, 3/12 cells)
X 1.1.3 Size comparison — WRONG TEST (asserts Size > Size returns int, spec says bool)
X 1.1.4 Duration literals — NEEDS TESTS (no tests found)
X 1.1.5 list.map() — NEEDS PIN (15 tests pass but none uniquely identifies map behavior)
X 1.1.6 iterator.zip() — REOPENED (was [x], matrix 2/10 cells, no pin)
+ GAP: 3 missing functionality items, 2 missing test items (see gap analysis)
```

### Frontmatter Updates

Phase 2 update agents apply frontmatter changes:
- All items `[x]` → `status: complete`
- Mixed → `status: in-progress`
- All items `[ ]` → `status: not-started`
- **Any reopened items** → section status MUST change to `in-progress` (even if it was `complete`)
- **Any new items added from gap analysis** → section status MUST be `in-progress`

#### Third Party Review Consistency Checks

The supervisor must also validate `third_party_review` frontmatter consistency:

1. **`status: complete` + `third_party_review.status: findings`** = INVALID — a section cannot be complete with unresolved TPR findings. Set section `status` to `in-progress`.
2. **Unchecked TPR items exist + `third_party_review.status: none`** = INVALID — set `third_party_review.status` to `findings`.
3. **Unchecked TPR items exist + `third_party_review.status: resolved`** = INVALID — set `third_party_review.status` to `findings`.
4. **All TPR items checked + `third_party_review.status: findings`** = STALE — set `third_party_review.status` to `resolved`.
5. **No TPR block or empty (`- None.`) + `third_party_review.status: findings`** = INVALID — set `third_party_review.status` to `none`.

Report any TPR consistency fixes alongside normal frontmatter updates in the batch summary.

### Commit Checkpoint (Phase 2 only)

Commits happen after Phase 2 update agents have written findings into section files — never after Phase 1 alone.

```
Verification complete (Sections 0, 1, 2).
Phase 1 (review): All sections reviewed
Phase 2 (update): All section files updated

Section 0 — Parser:
  Verified: 95/115 | Reopened: 0 | New items from gap analysis: 3
Section 1 — Type System:
  Verified: 100/124 | Reopened: 5 | New items from gap analysis: 7
Section 2 — Type Inference:
  Verified: 30/38 | Reopened: 0 | New items from gap analysis: 2
```

---

## Verification Criteria

### What Counts as "Verified"

ALL of the following must be true:

1. **Tests exist** — At least one test directly exercises the feature
2. **Tests pass** — All related tests (Ori, Rust, LLVM) pass
3. **Tests are correct** — Every assertion has been READ and checked against the spec
4. **Tests have adequate coverage** — Happy path, edge cases, and error cases are covered
5. **Assertions are specific** — Tests check actual values, not just `is_ok()` / `is_some()`
6. **Matrix coverage adequate** — All relevant types and patterns through the code path are tested (see matrix assessment criteria below)
7. **Semantic pin exists** — At least one test would uniquely fail if the feature were reverted

### What Counts as "Weak Tests"

1. **Shallow assertions** — `assert!(result.is_ok())` without checking the value
2. **Single path only** — Only happy path tested, no edge cases or errors
3. **Missing feature coverage** — Feature has 5 behaviors, tests cover 2

### What Counts as "Incomplete Matrix"

1. **Single-type coverage** — Tests only exercise `int` through a path that handles all types
2. **Missing collection types** — Tests use `[int]` but not `{str: int}`, `Set<T>`, or nested `[[int]]`
3. **No compound type coverage** — Tests skip `Option<T>`, `Result<T, E>`, tuples, user structs
4. **Missing edge-case dimension** — No empty input, no single-element, no boundary conditions
5. **Missing control-flow dimension** — Only basic iteration tested; no `break`, `yield`, guard, nested, or composition patterns
6. **Missing backend coverage** — Only interpreter tested; no LLVM/AOT tests for features with codegen implications
7. **RC-sensitive gap** — Types with different RC behavior (heap-allocated str/list vs. value-type int) not all tested through the same path

**Threshold**: If >30% of relevant matrix cells are untested, classify as INCOMPLETE MATRIX. For RC-sensitive code paths (COW, iterator cleanup, collection mutation), even a single missing RC-bearing type (str, list, map, struct with heap fields) is a gap.

### What Counts as "Needs Pin"

1. **No regression guard** — All tests could pass with a trivially wrong implementation
2. **Too-broad assertions** — Tests check types or shapes but not specific values
3. **Redundant with simpler features** — Tests could pass if the feature delegated to a simpler built-in
4. **Pin criteria**: At least one test must assert a **specific computed value** that only the correct implementation of this exact feature produces

### What Counts as "Wrong Tests"

1. **Incorrect expected values** — Assertion doesn't match what the spec requires
2. **Testing wrong behavior** — Test name says "addition" but tests multiplication
3. **Copy-paste errors** — Test is a duplicate of another with no meaningful difference
4. **False positive** — Test passes for the wrong reason (e.g., error swallowed)

### What Counts as "Cannot Verify"

1. **No tests exist** — Feature claimed complete but no test coverage
2. **Tests don't cover claim** — Tests exist but don't test the specific feature

### Annotation Requirements

**Be specific.** Every annotation must say exactly what's wrong and what's needed. Every `- [ ]` sub-item must be a concrete, actionable test description that someone can implement without further research.

Good:
```markdown
- INCOMPLETE MATRIX: list.sort() — 4/16 cells covered
  - [ ] Add test: sort [str] list (only [int] tested)
  - [ ] Add test: sort [Option<int>] list (RC cleanup on None during swap)
  - [ ] Add test: sort empty list [] (edge case)
  - [ ] Add test: sort single-element list [42] (boundary)
  - [ ] Add test: sort already-sorted list (optimization edge case)
  - [ ] Add test: sort reverse-sorted list (worst case for some algorithms)
- NEEDS PIN:
  - [ ] Add semantic pin: sort([3,1,2]) == [1,2,3] (distinguishes sort from identity/reverse)
```

Bad:
```markdown
- NEEDS TESTS: Add more tests
- INCOMPLETE MATRIX: needs more types
```

---

## Important Constraints

### DO NOT:
- Fix bugs encountered during verification
- Implement missing features
- Modify test files
- Change any code outside `plans/roadmap/`

### DO:
- Run existing tests
- Read spec for expected behavior
- Annotate items with specific test requirements
- Update checkbox status based on verification
- Track what needs attention

### If You Find a Bug:

In the review results, note it:
```markdown
- [ ] **Implement**: Feature X
  - BUG FOUND: [brief description]
  - Should be fixed before marking complete
```

Additionally, invoke `/add-bug` to file it in the bug tracker. Do NOT fix it — just document and file.

---

## Progress Tracking

### During Session

Supervisor maintains batch-level tracking:
```
Batch 1: [COMPLETE] Sections 0, 1, 2 — committed
Batch 2: [RUNNING]  Sections 3, 4, 5
  - Section 3 agent: 180/225 items processed
  - Section 4 agent: 90/110 items processed
  - Section 5 agent: 73/73 items processed (done, waiting for batch)
Batch 3: [PENDING]  Sections 6, 7A-D
```

### Between Sessions

If verification is interrupted, the last batch commit shows progress. Resume using:
```
/verify-roadmap continue
```

This resumes from the first unverified section (based on frontmatter status).

Or specify where to start:
```
/verify-roadmap section-3
```

---

## Output Format

### Phase 1 Agent Output (in results file)

Each review agent writes its results in this format — per item, then gap analysis:
```
─── Verifying 1.1.1: int type ───
Context loaded: CLAUDE.md (read), rules/*.md (20 files read), spec/clause-3.md (read)
Tests found: tests/spec/types/primitives.ori (12 tests)
Tests run: all pass
Audit: READ tests/spec/types/primitives.ori
  - line 5: `assert 1 + 2 == 3` — correct per spec
  - line 8: `assert -1 == -(1)` — correct, tests unary negation
  - line 12: `assert int_max + 1` — tests overflow behavior
Matrix assessment:
  Types tested: int, float, bool (3/3 relevant — operator is numeric-only)
  Patterns tested: basic, negative, overflow, boundary (4/4)
  Backend: interpreter only (LLVM N/A for this item)
  Coverage: 12/12 cells
Semantic pin: line 12 `assert_panics(expr: () -> int_max + 1)` — uniquely fails without overflow check
Status: VERIFIED (sound, matrix 12/12, pin: overflow_panics)

─── ... more items ... ───

═══ Gap Analysis: Section 1 — Type System ═══
Section goal: "Complete type system with primitives, compounds, generics, and type rules"
Listed items cover: primitives, compounds, generics, basic rules
GAPS FOUND:
  MISSING FUNCTIONALITY:
  - [ ] **Implement**: Recursive type validation — spec clause 3.7 requires cycle detection
  - [ ] **Implement**: Type alias transparency rules — spec says aliases are structurally transparent
  MISSING TESTS:
  - [ ] **Ori Tests**: tests/spec/types/recursive_validation.ori — no tests for recursive types
  - [ ] **Rust Tests**: compiler/ori_types/src/check/recursive/tests.rs — cycle detection unit tests
  MISSING ITEMS:
  - [ ] Integration: generic type bounds with compound types — both listed but interaction untested
  #skip markers found: 2 in tests/spec/types/ referencing untracked issues
  TODOs found: 1 in compiler/ori_types/src/check/mod.rs:142 — "TODO: handle recursive newtypes"
Items to add: 2 functionality, 2 test, 1 integration
```

**Critical**: Agents MUST show evidence of (1) reading CLAUDE.md + rules, (2) reading test files, (3) matrix assessment, (4) pin identification, (5) gap analysis. A result like this is REJECTED by the supervisor:
```
─── Verifying 1.1.1: int type ───
Tests found: tests/spec/types/primitives.ori
Tests run: pass
Status: VERIFIED
```
(No context-loading evidence, no audit, no matrix, no pin, no gap analysis — supervisor will flag this agent and re-verify.)

### Final Summary (after Phase 2)
```
═══ Verification Complete (Sections 0, 1, 2) ═══
Phase 1: All sections reviewed | Phase 2: All section files updated

Section 0 — Parser:
  Verified:           95/115
  Weak tests:          4
  Incomplete matrix:   3
  Needs pin:           1
  Needs tests:        12
  Regressions:         0
  Reopened:            0
  New items (gaps):    3  <-- added from gap analysis

Section 1 — Type System:
  Verified:          100/124
  Weak tests:          3
  Incomplete matrix:   5
  Needs pin:           2
  Wrong tests:         1
  Needs tests:         6
  Regressions:         2
  Reopened:            5  <-- previously [x] items downgraded
  New items (gaps):    7  <-- added from gap analysis

  Items needing attention:
  - 1.1A.5: float precision — INCOMPLETE MATRIX (only 1.0+2.0, 2/8 cells)
  - 1.1A.8: Duration subtract — WRONG TEST (expects int, spec says Duration)
  - 1.1B.4: break/continue Never type — NEEDS TESTS
  - 1.1A.12: Duration LLVM arithmetic — REGRESSION
  - 1.2.3: list.map() — REOPENED, was [x] — INCOMPLETE MATRIX (only [int], 3/15 cells)
  - 1.2.7: iterator.zip() — REOPENED, was [x] — NEEDS PIN (no unique regression guard)
  - NEW: recursive type validation — MISSING FUNCTIONALITY (spec clause 3.7)
  - NEW: type alias transparency — MISSING FUNCTIONALITY (spec says structurally transparent)

Section 2 — Type Inference:
  Verified:      30/38
  Needs tests:    5
  Needs pin:      3
  New items (gaps): 2
```

---

## Files Modified

**Phase 1 creates (temp):**
- `plans/roadmap/.verify-results/section-XX-results.md` — Review agent findings (can be deleted after Phase 2)

**Phase 2 modifies (the deliverable):**
- `plans/roadmap/section-*.md` — Status updates, annotations, reopened items, AND new items from gap analysis

**Never modifies:**
- Any code files
- Any test files
- Anything outside `plans/roadmap/` (except bug tracker via `/add-bug`)
