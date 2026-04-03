---
section: "03"
title: "Cross-Backend Dispatch Unification"
status: not-started
reviewed: false
goal: "Add registry-driven enforcement tests ensuring eval and LLVM backends cover the same method sets — detect drift at compile/test time"
depends_on: ["02"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "03.1"
    title: "Registry Coverage Enforcement Tests"
    status: not-started
  - id: "03.2"
    title: "Eval Exhaustiveness Guards Expansion"
    status: not-started
  - id: "03.3"
    title: "Method Coverage Gap Audit"
    status: not-started
  - id: "03.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "03.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 03: Cross-Backend Dispatch Unification

**Status:** Not Started
**Goal:** Both backends (ori_eval and ori_llvm) independently maintain method dispatch tables that must agree. Add enforcement tests that verify coverage against `ori_registry`, so adding a method to the registry and only one backend is caught immediately.

**Context:** The cross-backend DRY analysis found 17 LEAK findings where eval and LLVM maintain parallel dispatch tables for str (30 vs 14), map (15 vs 9), set, iterator, and trait methods. The LLVM side already has some registry sync tests (`option_builtin_handlers_match_registry`, `result_builtin_handlers_match_registry`), but the eval side has no equivalent. 80+ methods exist in eval but not LLVM — these may be intentional (LLVM defers to runtime calls), but there's no formal tracking.

**Depends on:** Section 02 (eval dispatch is cleaner after DRY extraction).

---

## 03.1 Registry Coverage Enforcement Tests

**File(s):** `compiler/ori_eval/src/methods/tests.rs` (new or existing), `compiler/ori_llvm/src/codegen/arc_emitter/builtins/tests.rs` (existing)

Add tests that iterate `ori_registry::methods_for(TypeTag::Str)` etc. and verify both backends can handle each method.

- [ ] For each builtin type (Str, List, Map, Set, Iterator, Int, Float, Bool, Char, Byte, Duration, Size, Option, Result):
  - [ ] Add test: `registry methods for {type} are all handled by eval dispatch`
  - [ ] Add test: `registry methods for {type} are all handled by LLVM dispatch OR have backend_required: false` <!-- reviewed: accuracy fix — field is `backend_required: false`, not `backend_not_required` -->
- [ ] Verify: adding a new method to `ori_registry` without both backends handling it causes a test failure
- [ ] Pattern: follow `option_builtin_handlers_match_registry` in LLVM tests as the template

---

## 03.2 Eval Exhaustiveness Guards Expansion

**File(s):** `compiler/ori_eval/src/methods/mod.rs`

The existing `_enforce_exhaustiveness()` function catches new TypeTags at compile time. Extend this pattern to method coverage.

- [ ] Add a test that iterates all TypeTag variants and verifies `dispatch_method()` has a handler for each (not just the compile-time dead function)
- [ ] Verify: the test is more comprehensive than the dead function approach — it tests runtime routing, not just compilation

---

## 03.3 Method Coverage Gap Audit

**File(s):** Documentation + bug tracker

The analysis found 80+ eval-only methods. These need to be formally tracked as either (a) intentionally eval-only (runtime handles them in LLVM) or (b) genuine gaps.

- [ ] For each type, compare eval method count vs LLVM method count vs registry method count
- [ ] For methods eval-only: verify they work in LLVM via runtime calls (not codegen builtins)
- [ ] For genuine gaps (methods that don't work at all in LLVM): file via `/add-bug` to bug tracker Section 04
- [ ] Update `ori_registry` `backend_required` flags to accurately reflect which methods MUST have LLVM builtins vs which can fall through to runtime

---

## 03.R Third Party Review Findings

- None.

---

## 03.N Completion Checklist

- [ ] Registry coverage enforcement tests for all builtin types in both backends
- [ ] Adding a new registry method without backend handling causes test failure
- [ ] 80+ eval-only methods formally audited and tracked
- [ ] `timeout 150 cargo test -p ori_eval` passes
- [ ] `timeout 150 cargo test -p ori_llvm` passes
- [ ] `timeout 150 ./test-all.sh` passes
- [ ] `/tpr-review` covering Section 03
- [ ] `/impl-hygiene-review last commit`
