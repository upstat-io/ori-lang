---
bug: "BUG-04-041"
title: "AOT codegen error + poison value produces crashing binary instead of clean compilation failure"
severity: "medium"
status: complete
goal: "AOT compilation aborts with a clear error when codegen errors are recorded, instead of producing a crashing binary"
success_criteria:
  - "`ori build` exits non-zero with descriptive error when codegen records errors"
  - "No crashing binary produced when codegen uses poison values"
  - "`ori run --compile` also reports error instead of producing crashing binary"
subsystem: "compiler/oric/src/commands/codegen_pipeline.rs"
found: "2026-04-06"
source: "tpr-review"
third_party_review:
  status: none
  updated: null
---

# Fix: BUG-04-041 — AOT codegen error + poison value produces crashing binary

**Status:** Not Started
**Severity:** Medium
**Goal:** When codegen records soft errors (via `record_codegen_error_with_msg`), the AOT pipeline should abort compilation with a clear error message instead of producing a binary containing poison LLVM values that crash at runtime.

**Success Criteria:**
- [ ] `ori build /tmp/test.ori` exits non-zero with error message listing codegen issues
- [ ] `ori run --compile /tmp/test.ori` also reports error instead of crashing binary
- [ ] JIT path behavior unchanged (already works correctly)
- [ ] No regressions in test-all.sh

**Context:** The JIT path (`evaluator/compile.rs:383`) checks `codegen_errors > 0` and returns `Err` with descriptive message. The AOT path (`codegen_pipeline.rs`) never checks this counter. When an unsupported operation uses `record_codegen_error_with_msg` + poison value pattern, the AOT binary is generated with garbage values that SIGSEGV at runtime. Discovered via BUG-04-039 fix where `join` on Duration iterators produces a codegen error but AOT compilation succeeds.

---

## 1. Root Cause Analysis

- **Symptom**: `ori build` succeeds but produced binary crashes (exit 139/SIGSEGV)
- **Proximate cause**: `run_codegen_pipeline()` never checks `IrBuilder.codegen_error_count()`
- **Root cause**: The JIT path was the first consumer and added the check. When AOT was built, the check wasn't duplicated. The `IrBuilder` is created inside a block scope and dropped before the error can be surfaced.
- **Blast radius**: Any unsupported operation that uses `record_codegen_error_with_msg` + poison value. Currently: `join` on non-Printable types (byte, Duration, Size, Ordering).
- **Affected files**:
  - `compiler/oric/src/commands/codegen_pipeline.rs` — add codegen error check before block exits

---

## 2. TDD — Test Matrix

### Exact failing case
- [ ] AOT test: `[1s, 2s].iter().join(separator: ", ")` via `ori build` then run should NOT produce a crashing binary

### Semantic pin
- [ ] Rust test: build program with unsupported codegen operation, verify compilation returns Err (not Ok)

### Negative pin
- [ ] Rust test: build program with NO codegen errors, verify compilation succeeds normally

---

## 3. Implementation

- [ ] In `run_codegen_pipeline()`, extract `builder.codegen_error_count()` and `builder.codegen_error_descriptions()` at the end of the compilation block (before line 457), store them in variables that persist after the block.
- [ ] After the block, check the error count. If > 0, return `Err` with descriptive message (matching the JIT path's format).
- [ ] This check should run BEFORE the LLVM IR verification (line 486), since codegen errors may also cause verification failures that produce less clear messages.

---

## 4. Completion Checklist

- [ ] All new tests pass unchanged after fix
- [ ] Debug AND release builds pass
- [ ] `timeout 150 ./test-all.sh` green
- [ ] `timeout 150 ./clippy-all.sh` green
- [ ] `/commit-push`
- [ ] Bug entry updated: `- [x]`
- [ ] Fix section status → `complete`
- [ ] `/tpr-review` passed (medium: expected)
- [ ] `/impl-hygiene-review` passed (medium: recommended)
- [ ] `/improve-tooling` retrospective completed — MANDATORY at section close, after both reviews are clean. Reflect on the section's debugging journey (which `diagnostics/` scripts you ran, which command sequences you repeated, where you added ad-hoc `dbg!`/`tracing` calls, where output was hard to interpret) and identify any tool/log/diagnostic improvement that would have made this section materially easier OR that would help the next section touching this area. Implement every accepted improvement NOW (zero deferral) and commit each via SEPARATE `/commit-push`. The retrospective is mandatory even when nothing felt painful — that is exactly when blind spots accumulate. See `.claude/skills/improve-tooling/SKILL.md` "Retrospective Mode" for the full protocol.

**Exit Criteria:** `ori build /tmp/duration_join.ori` (containing `[1s, 2s].iter().join(separator: ", ")`) exits with non-zero status and prints an error message containing "codegen" and the count of errors. No binary is produced. `test-all.sh` is green with 0 regressions.
