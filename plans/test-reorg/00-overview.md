# Test Reorganization Plan

> **Goal**: Reorganize Ori compiler tests into a dual-organization structure—feature tests and phase tests—while enforcing the 200-line inline test limit.

## Problem Statement

The compiler has grown to include 148 inline test modules totaling ~20,670 lines of test code. Of these:
- **25 modules (17%)** exceed the 200-line guideline
- **~10,882 lines (53%)** are in violation modules
- The worst offenders are 1,000+ lines each

This impacts:
1. **Readability**: Source files become dominated by tests
2. **Compilation**: Tests recompile when source changes (even if tests unchanged)
3. **Navigation**: Hard to find tests by feature vs. by compiler phase

## Solution: Dual Test Organization

```
tests/
├── spec/           # Feature tests (KEEP AS-IS)
│   └── ...         # User-facing behavior organized by language feature
│
├── phases/         # NEW: Compiler phase tests
│   ├── parse/      # Lexer + parser tests
│   ├── typeck/     # Type system tests
│   ├── eval/       # Interpreter tests
│   └── codegen/    # LLVM backend tests
│
└── compile-fail/   # KEEP AS-IS
```

---

## Current State Analysis

### Violations by Crate

| Crate | Violations | Worst Offender |
|-------|------------|----------------|
| ori_llvm | 10 | debug.rs (1,099), linker/mod.rs (1,071) |
| oric | 3 | commands/build.rs (399) |
| ori_patterns | 2 | value/scalar_int.rs (786) |
| ori_eval | 2 | — |
| ori_parse | 2 | — |
| ori_types | 2 | lib.rs (451) |
| ori_ir | 1 | builtin_methods.rs (327) |
| ori_diagnostic | 1 | queue.rs (261) |
| ori_lexer | 1 | lib.rs (461) |
| ori_rt | 1 | lib.rs (209) |

### Total Metrics

| Metric | Value |
|--------|-------|
| Inline test modules | 148 |
| Total inline test lines | ~20,670 |
| Modules > 200 lines | 25 (17%) |
| Lines in violations | ~10,882 (53%) |

---

## Section Overview

### Section 1: Infrastructure
Set up the `tests/phases/` directory structure and create shared test utilities.

### Section 2: Extreme Violations
Extract the two 1,000+ line modules:
- `ori_llvm/src/aot/debug.rs` (1,099 lines, 66 tests)
- `ori_llvm/src/aot/linker/mod.rs` (1,071 lines, 73 tests)

### Section 3: High Violations
Extract 500-800 line modules:
- `ori_patterns/src/value/scalar_int.rs` (786 lines)
- `ori_llvm/src/aot/passes.rs` (636 lines)
- `ori_patterns/src/errors.rs` (512 lines)
- `ori_llvm/src/aot/object.rs` (466 lines)
- `ori_lexer/src/lib.rs` (461 lines)

### Section 4: Medium Violations
Extract 200-500 line modules (remaining 18 modules).

### Section 5: Cleanup
Remove empty `mod tests` blocks, update CI, update documentation.

---

## Dependencies

```
Section 1 (Infrastructure)
    ↓
Section 2 (Extreme: 1000+ lines)
    ↓
Section 3 (High: 500-800 lines)
    ↓
Section 4 (Medium: 200-500 lines)
    ↓
Section 5 (Cleanup)
```

All sections are sequential—each depends on the previous completing.

---

## Success Criteria

1. **All inline test modules < 200 lines**
2. **Phase tests clearly organized by compiler stage**
3. **Spec tests unchanged** (feature-organized, dual-backend)
4. **CI runs both phase and spec tests**
5. **Test helpers reduce boilerplate**
6. **Clear documentation on where to add new tests**

---

## Timeline Estimate

| Stage | Days | Deliverable |
|-------|------|-------------|
| Infrastructure | 1 | Directory structure, helpers |
| Extreme violations | 2 | 2 modules extracted |
| High violations | 3 | 5 modules extracted |
| Medium violations | 4 | 18 modules extracted |
| Cleanup | 1 | Documentation, CI |
| **Total** | **11** | Full compliance |

---

## Rollback Plan

If issues arise:
1. Phase tests are additive—old inline tests still work
2. Move tests back by copying from `tests/phases/` to inline
3. Delete `tests/phases/` directory

---

## Questions to Resolve

1. Should `tests/phases/` be a workspace member or use path dependencies?
2. How to handle tests that span multiple phases?
3. Should phase tests also run through both backends?
4. Naming: `codegen/` vs `llvm/` for the codegen phase?

---

## Quick Reference

| Document | Purpose |
|----------|---------|
| `index.md` | Keyword search index |
| `00-overview.md` | This file—high-level goals |
| `section-01-infrastructure.md` | Directory and helper setup |
| `section-02-extreme-violations.md` | 1000+ line extractions |
| `section-03-high-violations.md` | 500-800 line extractions |
| `section-04-medium-violations.md` | 200-500 line extractions |
| `section-05-cleanup.md` | CI, docs, final polish |
