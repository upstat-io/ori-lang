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

- [ ] For each builtin type with a `TypeDef` in `ori_registry` (Str, List, Map, Set, Iterator, Int, Float, Bool, Char, Byte, Duration, Size, Option, Result, Error, Ordering, Range, Tuple, Channel):  - [ ] Add test in `compiler/ori_eval/src/methods/tests.rs`: `registry methods for {type} are all handled by eval dispatch`  - [ ] Add test in `compiler/ori_llvm/src/codegen/arc_emitter/builtins/tests.rs`: `registry methods for {type} are all handled by LLVM dispatch OR have backend_required: false`- [ ] **Template pattern** (follow `option_builtin_handlers_match_registry` in `compiler/ori_llvm/tests/aot/builtins.rs` or similar):
  ```rust
  #[test]
  fn registry_methods_for_str_covered_by_eval() {
      let registry_methods: Vec<_> = ori_registry::find_type(TypeTag::Str)
          .unwrap().methods.iter().map(|m| m.name).collect();
      for name in &registry_methods {
          assert!(eval_can_dispatch(TypeTag::Str, name),
              "eval missing handler for Str.{name}");
      }
  }
  ```
- [ ] Verify: adding a new method to `ori_registry` without both backends handling it causes a test failure
- [ ] **WARNING:** This sub-section requires understanding how eval dispatch routing works (`dispatch_check.rs`, `collection/mod.rs`, `CollectionMethod` enum) AND how LLVM builtin dispatch works (`declare_builtins!` macro in `builtins/*.rs`). Read both dispatch systems before writing tests.
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
- [ ] For methods eval-only: write a concrete test that calls the method in an AOT-compiled program and verify it produces the correct result. "Verify via runtime calls" means actually running the method through `ori build` + execute, not just assuming it works.- [ ] For genuine gaps (methods that don't work at all in LLVM): file via `/add-bug` to bug tracker immediately
- [ ] Update `ori_registry` `backend_required` flags to accurately reflect which methods MUST have LLVM builtins vs which can fall through to runtime. Each flag value must be justified by the test from the previous step.

---

## 03.R Third Party Review Findings

- None.

---

## 03.T Test Strategy

This section is primarily about creating enforcement tests. The test strategy itself IS the deliverable.

- [ ] For each builtin type with a registry TypeDef, create paired enforcement tests:
  - **Eval coverage test:** Every registry method for type T has a handler in eval dispatch
  - **LLVM coverage test:** Every registry method for type T has either a LLVM builtin handler OR `backend_required: false`
  - Test matrix dimensions: {all registry types} x {eval, llvm} = ~19 types x 2 backends = ~38 tests
- [ ] Create a "synthetic drift" test: temporarily add a fake method to a type's registry def and verify the enforcement test catches the missing handler
- [ ] For the 80+ eval-only methods audit: create AOT spec tests that exercise each eval-only method through `ori build` + execute. Test matrix: {eval-only methods} x {basic invocation, edge case input}
- [ ] Verify `timeout 150 cargo test -p ori_eval` passes
- [ ] Verify `timeout 150 cargo test -p ori_llvm` passes
- [ ] Verify `timeout 150 ./test-all.sh` passes

---

## 03.N Completion Checklist

- [ ] Registry coverage enforcement tests for all builtin types in both backends
- [ ] Adding a new registry method without backend handling causes test failure
- [ ] 80+ eval-only methods formally audited and tracked
- [ ] AOT spec tests for eval-only methods that should work via runtime calls
- [ ] `timeout 150 cargo test -p ori_eval` passes
- [ ] `timeout 150 cargo test -p ori_llvm` passes
- [ ] `timeout 150 ./test-all.sh` passes
- [ ] `/tpr-review` covering Section 03
- [ ] `/impl-hygiene-review`
- [ ] `/improve-tooling` retrospective completed — MANDATORY at section close, after both reviews are clean. Reflect on the section's debugging journey (which `diagnostics/` scripts you ran, which command sequences you repeated, where you added ad-hoc `dbg!`/`tracing` calls, where output was hard to interpret) and identify any tool/log/diagnostic improvement that would have made this section materially easier OR that would help the next section touching this area. Implement every accepted improvement NOW (zero deferral) and commit each via SEPARATE `/commit-push`. The retrospective is mandatory even when nothing felt painful — that is exactly when blind spots accumulate. See `.claude/skills/improve-tooling/SKILL.md` "Retrospective Mode" for the full protocol.
