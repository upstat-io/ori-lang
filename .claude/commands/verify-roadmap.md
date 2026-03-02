# Verify Roadmap Command

Systematically verify roadmap items using parallel subagents. This command does NOT implement features — it only verifies status, audits test quality, and annotates items that need better test coverage.

## Usage

```
/verify-roadmap [section]
```

- No args: Start from Section 1, Item 1
- `section-4`, `4`: Start from Section 4
- `continue`: Resume from last verified item (if tracking exists)

---

## Core Principle

**Verification only, no implementation.** For each item:

1. **Can verify → verified** — Tests pass, feature works → mark `[x]`
2. **Cannot verify → annotate + pending** — Insufficient tests → add test tasks, leave `[ ]`
3. **Move on** — Never fix code, never write features, just verify and annotate

---

## Workflow

### Architecture: Parallel Agents with Supervisor

Verification uses **parallel subagents** to process sections concurrently, with the main context acting as supervisor.

```
Main Context (Supervisor)
├── Batch 1: Launch agents for sections 0, 1, 2 (in background)
│   ├── Agent: section-00-parser.md → writes results to temp file
│   ├── Agent: section-01-type-system.md → writes results to temp file
│   └── Agent: section-02-type-inference.md → writes results to temp file
├── Monitor: Check agent outputs, verify they're auditing tests properly
├── Collect: Read results, apply to section files
├── Batch 2: Launch next batch...
└── Final: Update frontmatter, commit checkpoint
```

**Batch size**: 3-4 sections per batch (avoids overwhelming system resources with test runs).

**Why batches, not all-at-once**: Agents run tests via `cargo`, which involves compilation. Concurrent `cargo test` invocations for different packages can conflict. Batching keeps parallelism manageable.

### Step 1: Plan Batches

Read all section files. Group into batches of 3-4 sections, ordered by section number. If the user specified a single section, skip batching — just run one agent.

### Step 2: Launch Agent Batch

For each batch, launch parallel `general-purpose` subagents using the Task tool with `run_in_background: true`. Each agent receives:

1. The section file path
2. The spec directory path (`docs/ori_lang/v2026/spec/`)
3. Instructions to follow the verification protocol below
4. A results output path: `plans/roadmap/.verify-results/section-XX-results.md`

Each agent processes its section items sequentially (items within a section stay sequential to avoid test conflicts).

### Step 3: Supervisor Monitoring

While agents run, the main context:

1. **Periodically checks agent output** using Read on the output files
2. **Verifies agents are actually reading tests** — look for evidence of file reads, not just "tests pass"
3. **Flags agents that appear to skip test auditing** — if an agent marks items verified without showing it read the test code, intervene
4. **Collects completed results** as agents finish

### Step 4: Apply Results

After a batch completes, the main context:

1. Reads each agent's results file
2. Applies the status updates and annotations to the actual section files
3. Updates frontmatter statuses
4. Reports the batch summary

### Step 5: Next Batch or Commit

If more batches remain, go to Step 2. Otherwise, commit checkpoint.

---

## Agent Verification Protocol

Each subagent follows this protocol for every item in its assigned section:

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

**Every test that passes must be explicitly read and audited.** A passing test is NOT sufficient for verification — the test itself must be correct. For each test file found:

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

4. **Classify the test quality**:

   | Quality | Meaning | Action |
   |---------|---------|--------|
   | **Sound** | Tests are correct, assertions match spec, reasonable coverage | Mark `[x]` |
   | **Weak** | Tests pass but coverage is insufficient or assertions are shallow | Leave `[ ]`, annotate with specific gaps |
   | **Wrong** | Tests have incorrect assertions or test wrong behavior | Leave `[ ]`, annotate as WRONG TEST |
   | **Stale** | Tests reference outdated syntax/features | Leave `[ ]`, annotate as STALE TEST |

#### 2d. Update Item Status

**If Verified (tests pass AND are sound):**
```markdown
- [x] **Implement**: Feature X [done] (verified 2026-02-08)
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

#### 2e. Report Progress

After each item, briefly report (include test audit result):
```
✓ 1.1.1 Primitive int type — VERIFIED (3 tests in tests/spec/types/primitives.ori — sound)
✗ 1.1.2 Duration arithmetic — WEAK TESTS (tests pass but only test addition, missing overflow/negative)
✗ 1.1.3 Size comparison — WRONG TEST (asserts Size > Size returns int, spec says bool)
✗ 1.1.4 Duration literals — NEEDS TESTS (no tests found)
```

### Frontmatter Updates

After applying results to a section, the supervisor updates frontmatter:
- All items `[x]` → `status: complete`
- Mixed → `status: in-progress`
- All items `[ ]` → `status: not-started`

### Batch Commit Checkpoints

After each batch completes, the supervisor offers to commit:
```
Batch 1 verification complete (Sections 0, 1, 2).
- Section 0: 95/115 verified, 20 need attention
- Section 1: 100/124 verified, 24 need attention
- Section 2: 30/38 verified, 8 need attention

Commit checkpoint? (Allows resuming later with /verify-roadmap continue)
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

### What Counts as "Weak Tests"

1. **Shallow assertions** — `assert!(result.is_ok())` without checking the value
2. **Single path only** — Only happy path tested, no edge cases or errors
3. **Missing feature coverage** — Feature has 5 behaviors, tests cover 2

### What Counts as "Wrong Tests"

1. **Incorrect expected values** — Assertion doesn't match what the spec requires
2. **Testing wrong behavior** — Test name says "addition" but tests multiplication
3. **Copy-paste errors** — Test is a duplicate of another with no meaningful difference
4. **False positive** — Test passes for the wrong reason (e.g., error swallowed)

### What Counts as "Cannot Verify"

1. **No tests exist** — Feature claimed complete but no test coverage
2. **Tests don't cover claim** — Tests exist but don't test the specific feature

### Annotation Requirements

**Be specific.** Every annotation must say exactly what's wrong and what's needed.

Good:
```markdown
- WEAK TESTS:
  - [ ] Add test: Duration + Duration returns Duration (only int + int tested)
  - [ ] Add test: Duration overflow panics
  - [ ] Strengthen: tests/spec/types/duration.ori line 12 — assert actual value not just Ok
```

Bad:
```markdown
- NEEDS TESTS: Add more tests
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
```markdown
- [ ] **Implement**: Feature X
  - BUG FOUND: [brief description]
  - Should be fixed before marking complete
```

Do NOT fix it. Just document and move on.

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

### Agent Per-Item Output (in results file)

Each agent writes its results in this format per item:
```
─── Verifying 1.1.1: int type ───
Tests found: tests/spec/types/primitives.ori (12 tests)
Tests run: ✓ all pass
Audit: READ tests/spec/types/primitives.ori
  - line 5: `assert 1 + 2 == 3` — correct per spec
  - line 8: `assert -1 == -(1)` — correct, tests unary negation
  - line 12: `assert int_max + 1` — tests overflow behavior
  Coverage: happy path ✓, negation ✓, overflow ✓
Status: VERIFIED (sound)
```

**Critical**: Agents MUST show evidence of reading test files. A result like this is REJECTED by the supervisor:
```
─── Verifying 1.1.1: int type ───
Tests found: tests/spec/types/primitives.ori
Tests run: ✓ pass
Status: VERIFIED
```
(No audit evidence — supervisor will flag this agent and re-verify the item.)

### Supervisor Batch Summary
```
═══ Batch 1 Complete (Sections 0, 1, 2) ═══

Section 0 — Parser:
  Verified:      95/115
  Weak tests:     8
  Needs tests:   12
  Regressions:    0

Section 1 — Type System:
  Verified:     100/124
  Weak tests:     3
  Wrong tests:    1
  Needs tests:    6
  Regressions:    2

  Items needing attention:
  - 1.1A.5: float precision — WEAK TESTS (only tests 1.0 + 2.0, no edge cases)
  - 1.1A.8: Duration subtract — WRONG TEST (expects int, spec says Duration)
  - 1.1B.4: break/continue Never type — NEEDS TESTS
  - 1.1A.12: Duration LLVM arithmetic — REGRESSION

Section 2 — Type Inference:
  Verified:      30/38
  Needs tests:    8
```

---

## Files Modified

Only modifies:
- `plans/roadmap/section-*.md` — Status and annotations
- `plans/roadmap/.verify-results/` — Temporary agent results (can be deleted after verification)

Never modifies:
- Any code files
- Any test files
- Anything outside `plans/roadmap/`
