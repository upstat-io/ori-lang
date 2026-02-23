---
section: "09"
title: "FBIP Enforcement"
status: complete
goal: "Promote #fbip from informational analysis to enforced compile-time annotation"
inspired_by:
  - "Koka CheckFBIP (Core/CheckFBIP.hs)"
sections:
  - id: "09.1"
    title: "Define #fbip annotation"
    status: complete
  - id: "09.2"
    title: "Wire enforcement into pipeline"
    status: complete
  - id: "09.3"
    title: "Diagnostics"
    status: complete
  - id: "09.4"
    title: "Tests"
    status: complete
---

# Section 09: FBIP Enforcement

**Status:** Complete
**Goal:** Functions annotated `#fbip` are verified to be "functional but in-place" — all constructor reuse opportunities are realized.

**Context:** Ori already has `analyze_fbip` in `ori_arc/src/fbip/mod.rs` which detects missed reuse opportunities. Previously this was purely informational. The Koka compiler's `CheckFBIP.hs` enforces FBIP as a compile error — if a function is annotated as FBIP but has missed reuses, compilation fails. This gives developers a way to guarantee zero-allocation performance for critical code paths.

---

## 09.1 Define `#fbip` Annotation

**Files:** `compiler/ori_parse/src/grammar/attr/mod.rs`, `compiler/ori_ir/src/ast/items/function.rs`, `compiler/ori_types/src/output/mod.rs`, `compiler/ori_types/src/check/signatures/mod.rs`

- [x] Add `Fbip` variant to `AttrKind` enum
- [x] Add `is_fbip: bool` to `ParsedAttrs` — bare flag, no arguments
- [x] Parse both `#fbip` and `#[fbip]` syntax (follows existing `#skip`/`#derive` pattern)
- [x] Add `is_fbip: bool` to `Function` AST node
- [x] Add `is_fbip: bool` to `FunctionSig` (crosses Salsa boundary)
- [x] Propagate through `infer_function_signature_with_arena()`

Syntax:
```ori
#fbip
@transform (tree: Tree) -> Tree = match tree {
    Leaf(x) => Leaf(x + 1),
    Node(l, r) => Node(transform(l), transform(r)),
};
```

The annotation is a promise by the developer: "this function should allocate zero new heap memory beyond what reset/reuse provides."

---

## 09.2 Wire Enforcement into Pipeline

**Files:** `compiler/ori_arc/src/ir/mod.rs`, `compiler/ori_arc/src/lower/mod.rs`, `compiler/ori_arc/src/fbip/mod.rs`, `compiler/ori_arc/src/lib.rs`, `compiler/ori_llvm/src/codegen/function_compiler/mod.rs`

- [x] Add `is_fbip: bool` to `ArcFunction` struct
- [x] Thread `is_fbip` through `ArcIrBuilder::finish()` and `lower_function_can()`
- [x] Implement `check_fbip_enforcement()` — reuses existing `analyze_fbip()`, wraps result into `ArcProblem::FbipViolation`
- [x] Integrate at end of `run_arc_pipeline()` (after RC elimination): if `func.is_fbip`, run enforcement
- [x] Change `run_arc_pipeline()` return type to `Vec<ArcProblem>` to propagate violations
- [x] Update `run_arc_pipeline_all()` to collect problems from each function
- [x] Thread `is_fbip` through all callers:
  - `ori_llvm` function compiler (functions, tests, impl methods)
  - `ori_llvm` evaluator (JIT path)
  - `oric` compile_common (AOT path)
  - All `ori_arc` test files

---

## 09.3 Diagnostics

**Files:** `compiler/ori_diagnostic/src/error_code/mod.rs`, `compiler/oric/src/problem/codegen/mod.rs`

- [x] Add error code `E4004` — "FBIP enforcement violation"
- [x] Add `FbipViolation` variant to `ArcProblem` (with `func_name: String`, `missed_count`, `achieved_count`, `span`)
- [x] Add `ArcFbipViolation` variant to `CodegenProblem`
- [x] Implement `into_diagnostic()` with:
  - Error message: `"#fbip function '{name}' has {n} missed reuse opportunity(ies)"`
  - Label on function span: `"this function is annotated #fbip"`
  - Note: count of achieved vs missed reuses
  - Suggestion: `"remove #fbip or restructure to enable constructor reuse"`

---

## 09.4 Tests

- [x] Parser tests (`ori_parse/src/grammar/attr/tests.rs`):
  - `test_parse_fbip_attribute` — `#fbip` parses correctly
  - `test_parse_fbip_attribute_with_brackets` — `#[fbip]` also works
  - `test_parse_no_fbip_attribute` — function without `#fbip` has `is_fbip: false`
- [x] FBIP enforcement tests (`ori_arc/src/fbip/tests.rs`):
  - `enforcement_compliant_no_violation` — Reset/Reuse pair → no violation
  - `enforcement_violation_missed_reuse` — unpaired RcDec + Construct → FbipViolation returned
  - `enforcement_scalar_only_no_violation` — scalar-only function → no violation
- [x] Codegen diagnostic test (`oric/src/problem/codegen/tests.rs`):
  - `test_arc_fbip_violation_from` — E4004 code, correct message formatting
- [x] Error code bookkeeping updated (COUNT 118→119, MAX_UNDOCUMENTED 54→55)
- [x] Non-annotated functions unaffected (enforcement gated on `func.is_fbip`)
- [x] `cargo t` — 4,518+ Rust tests pass, 0 failures
- [x] `cargo bl` — LLVM build compiles
- [x] `./test-all.sh` — no new failures

---

## 09.5 Completion Checklist

- [x] `#fbip` annotation parsed and propagated (parser → AST → FunctionSig → ArcFunction)
- [x] `check_fbip_enforcement()` reuses existing `analyze_fbip()` analysis
- [x] Pipeline enforces FBIP on annotated functions (end of `run_arc_pipeline`)
- [x] Diagnostic messages with source spans and actionable suggestions (E4004)
- [x] Positive and negative unit tests (7 FBIP tests + 3 parser tests + 1 diagnostic test)
- [x] Non-annotated functions unaffected
- [x] `./test-all.sh` passes

**Exit Criteria:** `#fbip @f (...) = ...` compiles only if `f` achieves full constructor reuse. Violations produce E4004 diagnostics explaining what reuse was missed and why.
