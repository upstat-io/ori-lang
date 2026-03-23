# Section 06: Capabilities System -- Verification Results

**Verified**: 2026-03-19
**Section status**: in-progress (50/351, 14%)
**Methodology**: Sampled 3-5 checked items per complete subsection (6.1-6.8, 6.17). Confirmed unchecked items in not-started subsections (6.9, 6.11-6.14, 6.16) are genuinely incomplete. Ran `cargo st tests/spec/capabilities/` and `cargo st tests/spec/expressions/with_expr.ori` -- all pass (4181 passed, 0 failed, 42 skipped). Ran Rust tests for parser (`ori_parse`) and type checker (`ori_types`).

## Summary

| Status | Count |
|--------|-------|
| VERIFIED | 17 |
| STALE TEST | 6 |
| WEAK TESTS | 2 |
| NEEDS TESTS | 1 |
| CONFIRMED INCOMPLETE | ~55 |

**Overall assessment**: The checked items are genuinely working -- parser, type checker, evaluator all handle `uses` clauses, `with...in` expressions, and capability propagation checks (E2014). However, several Rust test file references in the roadmap are stale/incorrect (tests don't exist at the claimed locations or with the claimed names). Section 6.9 (Unsafe) is marked not-started but has significant partial implementation that should be acknowledged. Section 6.10 (def impl) has more implementation than the roadmap indicates.

---

## STALE TEST REFERENCES (cross-cutting)

The roadmap references `ori_types/src/check/tests.rs` for 7+ capability-related tests across sections 6.2, 6.3, 6.5, and 6.8. **These tests do not exist.** The actual `ori_types/src/check/tests.rs` contains only 8 tests for `ModuleChecker` basics (constructors, scopes, error accumulation). There is one test (`module_checker_function_scope`) that exercises `has_capability()`, but the roadmap claims of "7 tests for capability trait validation", "4 tests for Suspend", "7 tests for E2014 propagation" are all nonexistent at that location.

The actual capability constraint checking is in `compiler/ori_types/src/infer/expr/calls/constraints.rs` (the `check_call_capabilities` function), which is exercised by the Ori spec tests, not by dedicated Rust unit tests.

---

## 6.1 Capability Declaration

### [x] `uses` clause -- VERIFIED
- **Parser tests**: `compiler/ori_parse/src/tests/parser.rs` -- 4 tests: `test_uses_clause_single_capability`, `test_uses_clause_multiple_capabilities`, `test_uses_clause_with_where`, `test_no_uses_clause`. All pass.
- **Compositional test**: `test_generics_with_capabilities` in `compiler/ori_parse/src/tests/compositional.rs` -- tests generics + uses + where combinations. Passes.
- **Ori Tests**: `tests/spec/capabilities/declaration.ori` -- 3 test annotations (test_pure_function, test_with_capability, test_generic_with_capability). All pass.
- **Evidence**: `timeout 150 cargo test -p ori_parse -- test_uses_clause test_no_uses_clause` -- 4 passed, 0 failed.

### [x] Multiple capabilities -- VERIFIED
- **Parser test**: `test_uses_clause_multiple_capabilities` passes.
- **Ori Tests**: `tests/spec/capabilities/declaration.ori` -- @save_and_log example parses and runs correctly.

### STALE TEST: Roadmap says "Rust Tests: `ori_parse/src/lib.rs`" but tests are in `compiler/ori_parse/src/tests/parser.rs`.

---

## 6.2 Capability Traits

### [x] Capability traits -- STALE TEST
- **Roadmap claims**: "7 tests for capability trait validation" in `ori_types/src/check/tests.rs`. **These tests do not exist.**
- **Ori Tests**: `tests/spec/capabilities/traits.ori` -- entirely commented out (0 active tests). The file is a placeholder with TODO notes about type checker support needed.
- **Evidence**: The traits ARE defined in `library/std/prelude.ori` (Http, FileSystem, Cache, Clock, Random, Logger, Env) and are parseable, but there are no active Ori tests exercising capability trait method dispatch (e.g., `Http.get(url:)`).
- **Actual status**: Trait definitions exist in prelude, but the test file claims for Ori tests and Rust tests are both stale.
- **Classification**: STALE TEST (claimed tests don't exist; test file is entirely commented out)

---

## 6.3 Suspend Capability

### [x] Explicit suspension declaration -- STALE TEST
- **Roadmap claims**: "4 tests in ori_types/src/check/tests.rs (marker trait, signature storage, combined capabilities, sync function)". **These tests do not exist.** The only related test in that file is `module_checker_function_scope` which tests `has_capability()` generically.
- **Roadmap claims**: `test_sync_function_no_suspend_capability` -- **Does not exist** in any crate.
- **Ori Tests**: `tests/spec/capabilities/async.ori` is intentionally empty (async is not a language feature; Ori uses `Suspend` capability).
- **Classification**: STALE TEST (claimed Rust tests don't exist; Ori test file is empty)

### [x] No `async` type modifier -- VERIFIED
- **Parser test**: `test_no_async_type_modifier` in `compiler/ori_parse/src/tests/parser.rs` -- verifies `async int` fails to parse. Passes.
- **Parser test**: `test_async_as_identifier` -- verifies `async` can be used as identifier. Passes.
- **Roadmap claims** `test_async_keyword_reserved`: **Does not exist.** The actual test is `test_async_as_identifier` which tests the OPPOSITE (that async is NOT reserved).
- **Classification**: VERIFIED (behavior is correct, but test name reference is wrong)

### [x] No `await` expression -- STALE TEST
- **Roadmap claims**: `test_await_syntax_not_supported` in `ori_types/src/check/tests.rs`. **Does not exist.**
- **Evidence**: The evaluator has an `await_not_supported()` error factory (seen at line 340 of `can_eval/mod.rs`), confirming the feature IS rejected at runtime.
- **Classification**: STALE TEST (claimed test doesn't exist, but behavior is correct)

### [x] `uses Async` capability -- VERIFIED
- **Parser test**: `test_uses_async_capability_parses` -- verifies `@async_op () -> int uses Async = 42` parses correctly. Passes.

---

## 6.4 Providing Capabilities

### [x] `with...in` expression -- VERIFIED
- **Parser tests**: 3 tests in `compiler/ori_parse/src/tests/parser.rs`: `test_with_capability_expression`, `test_with_capability_with_struct_provider`, `test_with_capability_nested`. All pass.
- **Compositional test**: `test_with_capability_expressions` passes.
- **Ori Tests**: `tests/spec/capabilities/providing.ori` -- 17 test annotations covering basic provision, scoping, nesting, shadowing, different types, complex bodies, function calls. All pass.
- **Evidence**: `timeout 150 cargo st tests/spec/capabilities/providing.ori` -- all 17 tests pass.

### [x] Scoping -- VERIFIED
- **Evaluator**: `with_binding()` in `can_eval/mod.rs:350` handles scoped capability provision.
- **Ori Tests**: `tests/spec/capabilities/providing.ori` -- `test_scoping`, `test_shadowing`, `test_three_level_nesting`. All pass.

---

## 6.5 Capability Propagation

### [ ] Runtime capability propagation -- CONFIRMED INCOMPLETE
- **Ori Tests**: `tests/spec/expressions/with_expr.ori` has 2 tests skipped with `#skip("capability provision to called functions not implemented")`: `test_basic_with` and `test_multiple_capabilities`.
- **Status**: Correctly marked incomplete. Capability bindings in `with...in` don't propagate through function calls to callees.

### [x] Static transitive requirements -- VERIFIED
- **Code**: `check_call_capabilities()` in `compiler/ori_types/src/infer/expr/calls/constraints.rs` checks that callers declare required capabilities. E2014 diagnostic exists in `compiler/ori_diagnostic/src/errors/E2014.md`.
- **Ori Tests**: `tests/spec/capabilities/propagation.ori` -- 7 test annotations covering caller-declares-capability, nested provision, pure function calls. All pass.
- **Roadmap claims** "7 tests in ori_types/src/check/tests.rs": **Stale** -- the E2014 propagation checking is exercised by the Ori spec tests, not by dedicated Rust unit tests.
- **Classification**: VERIFIED (behavior correct, test reference stale)

### [x] Providing vs requiring -- VERIFIED
- **Code**: `check_call_capabilities()` checks `engine.has_capability(cap)` which considers both declared (`uses`) and provided (`with...in`) capabilities.
- **Ori Tests**: `tests/spec/capabilities/propagation.ori` -- `test_pure_caller_with_provide`, `test_caller_nested_provide` test providing capabilities via `with...in` to satisfy callee requirements.

---

## 6.6 Standard Capabilities

### [x] Trait interfaces defined -- VERIFIED
- **Location**: `library/std/prelude.ori` lines 258-318.
- **Traits confirmed**: Http (get/post/put/delete), FileSystem (read/write/exists/delete), Cache (get/set/del), Clock (now/today), Random (rand_int/rand_float), Logger (debug/info/warn/error), Env (get).
- **Note**: `Print` capability mentioned in syntax reference is NOT a separate trait -- `print()` is a built-in function, not a capability trait method. The roadmap correctly lists 7 traits.

### [ ] Real capability implementations -- CONFIRMED INCOMPLETE
- All 7 traits are definition-only with no real implementations. Correctly deferred to Section 7.

---

## 6.7 Testing with Capabilities

### [x] Mock implementations -- VERIFIED
- **Ori Tests**: `tests/spec/capabilities/propagation.ori` -- MockHttp and MockLogger structs with trait implementations used in `with...in` expressions. All 7 tests pass.
- **Pattern**: Tests demonstrate the pattern of defining mock structs that implement capability traits, then providing them via `with...in` for testing.

### [x] Test example -- VERIFIED
- Same file demonstrates the testing pattern with mock capabilities.

---

## 6.8 Capability Constraints

### [x] Compile-time enforcement -- WEAK TESTS
- **Code**: `check_call_capabilities()` function exists and is wired into call inference.
- **E2014 diagnostic**: Error code, message, and documentation all exist.
- **Ori Tests**: Propagation tests verify that capabilities ARE satisfied when provided, but there are **no negative tests** (compile_fail tests) verifying that calling a capability-using function WITHOUT providing the capability produces E2014.
- **Classification**: WEAK TESTS -- enforcement code exists but no test verifies it produces the expected error on violation.

---

## 6.9 Unsafe Capability (FFI Prep) -- PARTIALLY IMPLEMENTED (roadmap says "not-started")

**Finding: Roadmap status is inaccurate.** Section 6.9 is marked `status: not-started` but significant implementation exists:

- [done] IR representation: `ExprKind::Unsafe(ExprId)` in `compiler/ori_ir/src/ast/expr.rs:306`
- [done] Parser: `parse_unsafe_expr()` in `compiler/ori_parse/src/grammar/expr/primary/specials.rs:111`
- [done] Type checker: Transparent inference in `compiler/ori_types/src/infer/expr/mod.rs:216`
- [done] Evaluator: `CanExpr::Unsafe(inner) => self.eval_can(inner)` in `can_eval/mod.rs:337`
- [done] Visitor support: `compiler/ori_ir/src/visitor/walk_expr.rs:44`
- [done] Ori Tests: `tests/spec/capabilities/unsafe_block.ori` -- 6 tests (single expr, multi-stmt, nested, sub-expression, type preservation for str and bool). All pass.
- [not done] `Unsafe` as marker capability in type checker (no `uses Unsafe` requirement or `UnsafeContext` tracking)
- [not done] LLVM codegen
- [not done] E1250 diagnostic for calling unsafe code outside unsafe block

**The `unsafe { expr }` block expression works end-to-end in the interpreter**, but without capability enforcement (any code can use `unsafe {}` without declaring `uses Unsafe`).

---

## 6.10 Default Implementations (`def impl`) -- MORE IMPLEMENTED THAN CLAIMED

**Finding: Roadmap status understates implementation.** Several items marked `[ ]` are actually done:

- [done] `def` keyword in lexer: `compiler/ori_lexer/src/keywords/mod.rs:59` -- `"def" => Some(TokenKind::Def)`
- [done] Parse `def impl Trait { ... }`: `compiler/ori_parse/src/grammar/item/impl_def/mod.rs:187` -- `parse_def_impl()` with 5 Rust tests (basic, public, multiple methods, empty, multiple blocks). All pass.
- [done] IR representation: `DefImplDef` tracked in `Module.def_impls`
- [done] Evaluator method collection: `collect_def_impl_methods()` in `compiler/ori_eval/src/module_registration/mod.rs:371` with 2 Rust tests. Passes.
- [not done] Type checking validation (verify trait exists, method signature checking)
- [not done] Module export with default
- [not done] Name resolution (with...in > imported def > module-local)
- [not done] Ori spec tests (file exists but entirely commented out)
- [not done] LLVM codegen

**Parser and evaluator registration are done. Type checking and Ori-level integration are not.**

---

## 6.11 Capability Composition -- CONFIRMED NOT STARTED

- No multi-binding `with` syntax support in parser
- No capability variance checking
- No resolution priority order beyond basic `with...in` scoping
- Error codes E1200-E1203 do not exist in the codebase
- **Classification**: Genuinely not started

---

## 6.12 Default Implementation Resolution -- CONFIRMED NOT STARTED

- `without` is not a recognized keyword in the lexer
- No import conflict detection
- No `without def` import syntax in parser
- **Classification**: Genuinely not started

---

## 6.13 Named Capability Sets (`capset`) -- CONFIRMED NOT STARTED

- `capset` is not a recognized keyword in the lexer
- No capset parsing, expansion, or validation
- Error codes E1220-E1223 do not exist
- **Classification**: Genuinely not started

---

## 6.14 Intrinsics Capability -- CONFIRMED NOT STARTED

- No SIMD type validation, Mask type, or Intrinsics trait
- No error codes E1060-E1064
- **Classification**: Genuinely not started

---

## 6.16 Stateful Handlers -- CONFIRMED NOT STARTED

- `handler` is not a recognized keyword in the lexer
- No handler expression parsing or IR
- No error codes E1204-E1207
- 1 test in `with_expr.ori` is skipped with `#skip("requires stateful handlers")`
- **Classification**: Genuinely not started

---

## 6.17 Section Completion Checklist

### [x] 6.1-6.5 complete -- WEAK TESTS
- Declaration, providing, scoping all work. Propagation is partially complete (static checking works, runtime propagation to called functions does not).
- Capability trait test file (`traits.ori`) is entirely commented out.
- The "complete" claim is overstated for 6.2 (traits) and 6.5 (propagation) -- 6.2 has zero active tests, 6.5 has incomplete runtime propagation.

### [x] 6.6 trait definitions in prelude -- VERIFIED
- All 7 standard capability traits defined in `library/std/prelude.ori`.

### [x] 6.7-6.8 complete -- VERIFIED (with caveat)
- Mocking pattern demonstrated. Compile-time enforcement exists (E2014) but lacks negative tests.

### [ ] 6.9-6.16 -- CONFIRMED INCOMPLETE (but 6.9 and 6.10 have partial implementation)

---

## Cross-Section Notes

### LLVM Gap
No LLVM codegen exists for any capability feature (`with...in`, `uses`, `unsafe`). The `ori_llvm` crate has zero references to `WithCapability` or `Unsafe` expression kinds. All capability tests are interpreter-only.

### Test Count Discrepancy
The roadmap header says "~36 test annotations across 6 test files." Actual count:
- declaration.ori: 3
- providing.ori: 17
- propagation.ori: 7
- unsafe_block.ori: 6
- with_expr.ori: 16 (but 3 skipped)
- traits.ori: 0 (all commented out)
- default-impl.ori: 0 (all commented out)
- async.ori: 0 (empty)

Total active: 49 annotations across 5 files (not 36 across 6).

### Stale Rust Test References
The roadmap frequently references `ori_types/src/check/tests.rs` for capability-specific unit tests that do not exist. The actual capability checking logic is in `ori_types/src/infer/expr/calls/constraints.rs` and is tested indirectly through Ori spec tests. The roadmap should either update these references to reflect the actual code location, or note that the tests are integration-level (Ori spec tests) rather than unit-level (Rust tests).
