# Test Reorganization Index

> **Maintenance Notice:** Update this index when adding/modifying sections.

Quick-reference keyword index for finding plan sections. Search for a term to locate the relevant section.

---

## How to Use

1. **Search this file** (Ctrl+F / Cmd+F) for keywords related to what you're looking for
2. **Find the section ID** in the keyword cluster
3. **Open the section file**: `plans/test-reorg/section-{ID}-*.md`

---

## Keyword Clusters by Section

### Section 01: Infrastructure
**File:** `section-01-infrastructure.md` | **Status:** ✅ Completed

```
infrastructure, setup, directory structure
compiler/oric/tests/phases/, phase tests, test organization
common utilities, test helpers, shared code
parse helper, typecheck helper, eval helper, codegen helper
```

---

### Section 02: Extreme Violations (1000+ lines)
**File:** `section-02-extreme-violations.md` | **Status:** ✅ Completed

```
extreme violations, 1000+ lines, debug.rs, linker
ori_llvm/src/aot/debug.rs, debug info, DWARF
ori_llvm/src/aot/linker/mod.rs, linker driver
debug_config, debug_builder, debug_types, debug_context
linker_core, linker_gcc, linker_msvc, linker_wasm
```

---

### Section 03: High Violations (500-800 lines)
**File:** `section-03-high-violations.md` | **Status:** ✅ Completed

```
high violations, 500-800 lines
scalar_int, ori_patterns, pattern matching, value representation
passes.rs, optimization, optimization passes
errors.rs, pattern errors
object.rs, object emit, object file
lexer, tokenization, ori_lexer
```

---

### Section 04: Medium Violations (200-500 lines)
**File:** `section-04-medium-violations.md` | **Status:** ✅ Completed

```
medium violations, 200-500 lines
ori_types, type context, types.rs, type_interner.rs
oric, build command, commands/build.rs
ori_ir, visitor, AST traversal
ori_diagnostic, queue, error handling
mangle, mangling, symbol names
target, targets, platform-specific
wasm, WebAssembly config
ori_lexer, lexer tests
ori_patterns, scalar_int, pattern errors
```

---

### Section 05: Cleanup
**File:** `section-05-cleanup.md` | **Status:** ✅ Completed

```
cleanup, finalization, polish
mod tests, empty test modules, remove
CI, continuous integration, GitHub Actions
documentation, CLAUDE.md, guidelines
test naming convention, test organization docs
```

---

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | Infrastructure | `section-01-infrastructure.md` | ✅ Completed |
| 02 | Extreme Violations | `section-02-extreme-violations.md` | ✅ Completed |
| 03 | High Violations | `section-03-high-violations.md` | ✅ Completed |
| 04 | Medium Violations | `section-04-medium-violations.md` | ✅ Completed |
| 05 | Cleanup | `section-05-cleanup.md` | ✅ Completed |

---

## Phase → Directory Mapping

| Phase | Crates | Target Directory |
|-------|--------|------------------|
| Parse | ori_lexer, ori_parse | `compiler/oric/tests/phases/parse/` |
| Typeck | ori_typeck, ori_types | `compiler/oric/tests/phases/typeck/` |
| Eval | ori_eval, ori_patterns | `compiler/oric/tests/phases/eval/` |
| Codegen | ori_llvm, ori_rt | `compiler/oric/tests/phases/codegen/` |
| Common | ori_diagnostic, ori_ir, oric | `compiler/oric/tests/phases/common/` |
