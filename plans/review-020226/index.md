# Code Review Remediation Index

> **Maintenance Notice:** Update this index when adding/modifying sections.

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 01: Clippy Errors & CI
**File:** `section-01-clippy.md` | **Status:** ✅ Completed | **Priority:** CRITICAL

```
clippy, lint, ci, build, compile error
if_not_else, cast_precision_loss, cast_possible_truncation
doc_markdown, self_only_used_in_recursion
unwrap_used, float_cmp, incremental.rs
```

---

### Section 02: Dependency Cleanup
**File:** `section-02-dependencies.md` | **Status:** ✅ Completed | **Priority:** CRITICAL

```
cargo machete, unused dependency, Cargo.toml
ori_stack, logos, ori_macros, rayon
parking_lot, rustc-hash, serde
transitive dependency, dead dep
```

---

### Section 03: Diagnostic System Migration
**File:** `section-03-ori-macros.md` | **Status:** ✅ Completed | **Priority:** CRITICAL

```
ori_macros, #[derive(Diagnostic)], diagnostic derive
Problem enum, SemanticProblem, TypeProblem, ParseProblem
IntoDiagnostic, into_diagnostic(), Render trait
error code, ErrorCode, E2001, E2003
primary_span, label, suggestion, note
```

---

### Section 04: Memory & Interning
**File:** `section-04-memory.md` | **Status:** ⚠️ Partially Completed | **Priority:** CRITICAL

```
String, Name, interning, interner
Salsa query, Clone, memory allocation
EvalOutput, Variant, type_name, variant_name
to_string(), lookup(), hot path
#[cold], error constructor
```

**Completed:**
- EvalOutput.Variant String→Name (Salsa-cached, high impact)
- #[cold] annotations on error constructors

**Deferred (lower impact):**
- Problem enums (not in main Salsa flow)
- TestResult (not Salsa-cached)
- Interpreter hot paths (needs Value refactor)

---

### Section 05: Panic Elimination
**File:** `section-05-panic.md` | **Status:** ⚠️ Mostly Complete | **Priority:** CRITICAL

```
panic!, unwrap, expect, user input
span.rs, interner.rs, arena.rs
Result, error handling, recoverable
capacity exceeded, invalid range
```

**Completed:**
- SpanError, InternError, TypeInternError with try_* APIs
- EvalError.span field and with_span() builder
- #[cold] panic helpers for truly exceptional conditions

**Remaining:**
- Audit evaluator for consistent span attachment

---

### Section 06: Performance Optimization
**File:** `section-06-performance.md` | **Status:** 🔄 In Progress | **Priority:** HIGH

```
O(n²), linear scan, nested loop
HashMap, FxHashMap, rustc-hash
hot path, allocation, clone
compiled_modules, ModuleNamespace
build.rs, core.rs, module_registration
```

**Completed:**
- build.rs O(n²) pattern → O(n+m) with FxHashMap index
- FxHashMap migration in ori_llvm (~27 files, all tests pass)

**Remaining:**
- ModuleNamespace Vec→HashMap migration
- Arc cloning optimization
- FxHashMap in ori_patterns

---

### Section 07: Large Function Extraction
**File:** `section-07-functions.md` | **Status:** Not Started | **Priority:** HIGH

```
function length, 50 lines, 100 lines, 200 lines
copy_expr, eval_inner, render, main
incremental.rs, interpreter, reporting
match statement, god function
```

---

### Section 08: Extractable Patterns
**File:** `section-08-patterns.md` | **Status:** Not Started | **Priority:** HIGH

```
match arms, repetitive, pattern extraction
Token display, display_name, 115 arms
binary operator, eval_*_binary, 15 functions
copy_* methods, AstCopier, visitor pattern
Value Debug, Display, derive macro
spacing rules, DSL, table-driven
```

---

### Section 09: Diagnostic Quality
**File:** `section-09-diagnostics.md` | **Status:** Not Started | **Priority:** HIGH

```
EvalError, span, source location
did you mean, suggestion, typo
undefined_variable, undefined_function
suggest_identifier, suggest_function
error message, user-friendly
```

---

### Section 10: Testing Improvements
**File:** `section-10-testing.md` | **Status:** Not Started | **Priority:** MEDIUM

```
typeck.rs, public function, test coverage
inline test, 200 lines, tests/ subdirectory
flaky test, SystemTime, deterministic
test naming, test_some, test_none
```

---

### Section 11: API Design
**File:** `section-11-api.md` | **Status:** Not Started | **Priority:** MEDIUM

```
boolean parameter, enum, flag
is_multiline, had_trailing, soft
TrailingCommaPolicy, LineMode, DiagnosticSeverity
context.rs, queue.rs
```

---

## Quick Reference

| ID | Title | File | Priority |
|----|-------|------|----------|
| 01 | Clippy Errors & CI | `section-01-clippy.md` | CRITICAL |
| 02 | Dependency Cleanup | `section-02-dependencies.md` | CRITICAL |
| 03 | Diagnostic System Migration | `section-03-ori-macros.md` | CRITICAL |
| 04 | Memory & Interning | `section-04-memory.md` | CRITICAL |
| 05 | Panic Elimination | `section-05-panic.md` | CRITICAL |
| 06 | Performance Optimization | `section-06-performance.md` | HIGH |
| 07 | Large Function Extraction | `section-07-functions.md` | HIGH |
| 08 | Extractable Patterns | `section-08-patterns.md` | HIGH |
| 09 | Diagnostic Quality | `section-09-diagnostics.md` | HIGH |
| 10 | Testing Improvements | `section-10-testing.md` | MEDIUM |
| 11 | API Design | `section-11-api.md` | MEDIUM |
