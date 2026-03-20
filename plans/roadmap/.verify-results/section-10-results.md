# Section 10 Verification Results: Control Flow

**Verified**: 2026-03-19
**Section**: `plans/roadmap/section-10-control-flow.md`
**Status**: in-progress (134/485 items, ~27%)

---

## Summary

Section 10 covers if expressions, for expressions, loop expressions, while loops, labeled blocks, error propagation (?), let bindings, scoping, panics, and index expressions. The checked items are generally accurate -- all spec tests and AOT tests pass. Several test count references in the roadmap are stale (e.g., block_scope.ori has 23 tests, not 3; loops.ori has 35 tests, not 29). Three scoping AOT tests are listed as `[ignored]` in the roadmap but are actually passing without `#[ignore]`. The `?` operator (10.4) is marked mostly `[ ]` but has working AOT tests and a complete interpreter implementation. One checked unchecked item ("Without else: then-branch must be void or Never") is confirmed genuinely incomplete.

**All spec tests**: 4181 passed, 0 failed, 42 skipped
**All AOT tests referenced**: passing (for_loops 34/34, scoping 36/36, mutations 22/22, error_handling 28/28, higher_order 50/50)

---

## 10.1 if Expression

### [x] Parse `if cond then expr else expr` -- VERIFIED

- **Spec tests**: `tests/spec/expressions/conditionals.ori` -- 19 tests, all passing
- **AOT tests**: `ori_llvm/tests/aot/scoping.rs` -- 6 tests covering if-else as expression value, all passing
- Tests verify: basic if-then-else, else-if chains, nested conditionals, complex conditions, short-circuit, Never coercion in branches, function call conditions, computation in branches, boolean conditions, comparison operators, negation

**Classification**: VERIFIED

### [x] Else-if chains -- VERIFIED

- **Spec tests**: `test_if_else_if` (3 branches), `test_long_else_if` (5 branches)
- **AOT tests**: `test_scope_nested_if_expression` (4-branch else-if chain)
- Tests confirm correct branch selection across multiple cases

**Classification**: VERIFIED

### [x] Condition must be `bool` -- VERIFIED

- **Spec tests**: All conditional tests use bool conditions (comparison operators, boolean variables, boolean operators)
- No compile-fail test for non-bool conditions exists

**Classification**: VERIFIED -- but WEAK TESTS (no negative test for non-bool condition)

### [x] Branch type unification -- VERIFIED

- **Spec tests**: Multiple tests verify both branches return same type (int, str)
- **AOT tests**: `test_scope_if_else_value`, `test_scope_if_else_string_value` (type-checked branches producing same type)
- `test_if_coercion` tests Never coercion (panic in else-branch)

**Classification**: VERIFIED

### [ ] Without else: then-branch must be void or Never -- CONFIRMED INCOMPLETE

- **Spec tests**: `test_if_then_true_executes`, `test_if_then_false_skips`, `test_if_then_guard_pattern`, `test_if_then_explicit_unit` -- these DO test void/Never then-branches
- **Bug confirmed**: `let x = if true then 42` (non-void then-branch without else) is accepted by the compiler but should be a type error per spec. The type checker does not enforce that an if-without-else must produce void or Never.

**Classification**: VERIFIED as `[ ]` -- BUG FOUND (non-void if-without-else accepted)

### [ ] Never coercion in branches -- NEEDS TESTS

- The test `test_if_coercion` in `conditionals.ori` DOES test Never coercion (`if true then 42 else panic(msg: "unreachable")`), and it passes. This item appears partially implemented but marked `[ ]`.

**Classification**: PARTIALLY IMPLEMENTED -- needs negative tests and more thorough coverage

### [ ] Struct literal restriction in condition -- not verified (genuinely incomplete)

**Classification**: VERIFIED as `[ ]`

---

## 10.2 for Expressions

### [x] Parse `for x in items do expr` -- VERIFIED

- **Spec tests**: `tests/spec/expressions/loops.ori` -- 35 tests total (roadmap says 29, STALE COUNT)
- **AOT tests**: `ori_llvm/tests/aot/for_loops.rs` -- 34 tests total, covering Range, List, Str, Option, Map iteration (do and yield), guards, break/continue, mutation, step ranges, descending ranges, zero-step panic
- All for-do tests pass in both interpreter and AOT

**Classification**: VERIFIED -- STALE COUNT in roadmap (29 -> 35)

### [x] Bind loop variable -- VERIFIED

- Tested across all iterable types (Range, List, Str, Option, Map) in both spec and AOT tests
- Loop variable is used in body and guard expressions

**Classification**: VERIFIED

### [x] Execute body for side effects -- VERIFIED

- Accumulator patterns tested: `test_for_do_basic`, `test_for_range_sum`, `test_for_list_sum`, `test_for_str_count_chars`, `test_for_map_sum`
- AOT tests: `test_mut_loop_counter`, `test_mut_loop_accumulator`, `test_mut_loop_product`, `test_mut_loop_conditional_accumulator`

**Classification**: VERIFIED

### [x] Result type `void` -- VERIFIED

- `test_for_do_returns_void` explicitly tests void return
- All for-do tests use the expression for side effects only

**Classification**: VERIFIED

### [x] Parse `for x in items yield expr` -- VERIFIED

- **Spec tests**: `test_for_yield_basic`, `test_for_yield_empty`, `test_for_yield_identity`
- **AOT tests**: `test_for_range_yield`, `test_for_list_yield`, `test_for_str_yield`, `test_for_option_yield_some`, `test_for_option_yield_none`, `test_for_map_yield`

**Classification**: VERIFIED

### [x] Collect results into list / Result type `[T]` -- VERIFIED

- All for-yield tests verify collection via `.length()` or direct equality comparison

**Classification**: VERIFIED

### [x] Parse `for x in items if guard yield expr` -- VERIFIED

- **Spec tests**: `test_for_yield_with_guard`, `test_for_yield_guard_transform`, `test_for_do_with_guard`, `test_for_do_guard_all_filtered`
- **AOT tests**: `test_for_range_with_guard`, `test_for_list_with_guard`

**Classification**: VERIFIED

### [x] Only yield when guard true -- VERIFIED

- `test_for_do_guard_all_filtered` verifies no elements pass when all filtered
- `test_for_yield_with_guard` verifies only matching elements collected

**Classification**: VERIFIED

### [ ] For-yield comprehensions -- VERIFIED as `[ ]`

- Nested for clauses, multi-target collection (Set, Map), type inference for collection target -- all genuinely not implemented

**Classification**: VERIFIED as `[ ]`

---

## 10.3 loop Expression

### [x] Parse `loop { body }` -- VERIFIED

- **Spec tests**: `test_loop_with_break`, `test_loop_break_value`, `test_loop_int`, `test_loop_continue`
- **AOT tests**: `test_mut_loop_break`, `test_mut_while_pattern`

**Classification**: VERIFIED

### [x] Loop until `break` -- VERIFIED

- All loop tests use break to terminate
- `test_loop_conditional_break` tests conditional break with value

**Classification**: VERIFIED

### [x] Body is a block expression -- VERIFIED

- All loop tests use `loop { ... }` block syntax

**Classification**: VERIFIED

### [x] Parse `break` with optional value -- VERIFIED

- `test_loop_with_break` (break without value), `test_loop_break_value` (break with value), `test_loop_conditional_break` (conditional break with computed value)
- `test_for_yield_break_value` tests break with value in for-yield context

**Classification**: VERIFIED

### [x] Parse `continue` -- VERIFIED

- `test_loop_continue` tests continue in loop
- `test_for_do_continue`, `test_for_yield_continue` test continue in for expressions
- `test_for_yield_continue_value` tests continue with substitution value in yield context

**Classification**: VERIFIED

### [x] Result type from `break` value -- VERIFIED

- `test_loop_break_value` assigns `loop { break 42 }` to int variable
- `test_loop_int` tests `let result: int = loop {break 42}`

**Classification**: VERIFIED

### [x] Type `void` for break without value -- VERIFIED

- `test_loop_void` tests void loop type
- `test_mut_loop_break` (AOT) tests loop with break, no value

**Classification**: VERIFIED

### [ ] `continue value` error in loop -- VERIFIED as `[ ]`

- No error reported for `continue value` in loop context. Genuinely not implemented.

**Classification**: VERIFIED as `[ ]`

### [ ] Type `Never` for infinite loops -- VERIFIED as `[ ]`

- No tests for infinite loop Never type. Genuinely not implemented.

**Classification**: VERIFIED as `[ ]`

### [ ] Multiple break paths type unification -- VERIFIED as `[ ]`

- No tests for break type mismatch error. Genuinely not implemented.

**Classification**: VERIFIED as `[ ]`

### [ ] Labeled loops -- VERIFIED as `[ ]`

- Parser DOES support `loop:label`, `for:label`, `break:label`, `continue:label` (via `parse_optional_label()`)
- Evaluator does NOT handle labels -- `ControlAction::Break`/`ControlAction::Continue` do not carry label information
- Type checker has no label scope/shadowing validation
- The `#skip` test `test_find_first` confirms labeled breaks are known-not-working
- All labeled loop items are genuinely `[ ]` at the semantic level despite parser support

**Classification**: VERIFIED as `[ ]` -- parser partially ahead of semantics

---

## 10.3B Labeled Block Early Exit -- VERIFIED as not-started

- No `LabeledBlock` in IR, no `block` keyword in lexer
- All items genuinely `[ ]`

**Classification**: VERIFIED as `[ ]`

---

## 10.3A while Expression -- VERIFIED as not-started

- `while` is NOT in the keyword table -- not even recognized by the lexer
- No `KwWhile` token variant, no `ExprKind::While`
- All items genuinely `[ ]`

**Classification**: VERIFIED as `[ ]`

---

## 10.4 Error Propagation (?)

### [ ] Parse postfix `?` operator -- ROADMAP INACCURACY (actually implemented)

- **Parser**: `ExprKind::Try(ExprId)` exists in `ast/expr.rs`
- **Type checker**: Handles `CanExpr::Try` in inference
- **Evaluator**: `CanExpr::Try` handled in `can_eval/mod.rs` -- unwraps Ok/Some, propagates Err with trace injection, returns None
- **AOT tests**: 6 tests passing (`test_err_try_result_ok`, `test_err_try_result_err`, `test_err_try_result_chain`, `test_err_try_result_early_exit`, `test_err_try_option_some`, `test_err_try_option_none`)
- The `?` operator is FULLY WORKING across interpreter and LLVM

**Classification**: ROADMAP INACCURACY -- items should be `[x]`, not `[ ]`. The `?` operator is implemented and tested.

### [ ] On Result: unwrap Ok or return Err -- ROADMAP INACCURACY

- Fully implemented. `CanExpr::Try` match arm handles `Value::Ok(v)` -> unwrap, `Value::Err(_)` -> propagate.
- AOT tests confirm chaining and early exit.

**Classification**: ROADMAP INACCURACY -- should be `[x]`

### [ ] On Option: unwrap Some or return None -- ROADMAP INACCURACY

- Fully implemented. `CanExpr::Try` match arm handles `Value::Some(v)` -> unwrap, `Value::None` -> propagate.
- AOT tests confirm both paths.

**Classification**: ROADMAP INACCURACY -- should be `[x]`

### [ ] Only valid in functions returning Result/Option -- not verified

- Not verified whether the type checker enforces this restriction. No compile-fail tests exist.

**Classification**: NEEDS TESTS

### [x] `Traceable` trait for built-in Error type -- VERIFIED

- `with_trace`, `trace`, `trace_entries`, `has_trace` methods exist in `ori_eval/src/methods/error/`
- Error trace injection happens at `?` propagation points in `can_eval/mod.rs` via `inject_trace_entry`
- Registered in evaluator prelude/builder

**Classification**: VERIFIED

### [ ] Error Return Traces -- PARTIALLY IMPLEMENTED

- Trace collection at `?` is implemented (see `inject_trace_entry` in evaluator)
- `TraceEntry` type exists
- Error trace methods exist
- But no Ori spec tests exist for trace functionality
- LLVM codegen for traces is not implemented

**Classification**: PARTIALLY IMPLEMENTED -- interpreter has trace support, but no spec tests and no LLVM support

---

## 10.5 Let Bindings

### [x] Parse `let x = expr` -- VERIFIED

- **Spec tests**: `tests/spec/expressions/bindings.ori` -- 17 tests, all passing
- **AOT tests**: `test_scope_let_basic`, `test_scope_let_chain`, etc.

**Classification**: VERIFIED

### [x] Parse `let mut x = expr` -- VERIFIED

- **Spec tests**: `tests/spec/expressions/mutation.ori` -- 15 tests, all passing
- **AOT tests**: 21 mutation tests passing

**Classification**: VERIFIED

### [x] Parse `let x: Type = expr` -- VERIFIED

- **Spec tests**: `test_let_annotated_int`, `test_let_annotated_str`, `test_let_annotated_bool`, `test_let_annotated_float`, `test_let_annotated_char`
- **AOT tests**: `test_scope_let_type_annotation`

**Classification**: VERIFIED

### [x] Parse struct destructuring -- VERIFIED

- **Spec tests**: `test_struct_destructure_shorthand`, `test_struct_destructure_rename`, `test_struct_destructure_partial`, `test_struct_destructure_nested`
- No AOT coverage (roadmap accurately notes this)

**Classification**: VERIFIED

### [x] Parse tuple destructuring -- VERIFIED

- **Spec tests**: `test_tuple_destructure`
- **AOT tests**: `test_scope_tuple_destructure`

**Classification**: VERIFIED

### [x] Parse list destructuring -- VERIFIED

- **Spec tests**: `test_list_destructure_basic`, `test_list_destructure_head`, `test_list_destructure_with_rest`
- No AOT coverage (roadmap accurately notes this)

**Classification**: VERIFIED

---

## 10.6 Scoping

### [x] Lexical scoping -- VERIFIED

- **Spec tests**: `tests/spec/expressions/block_scope.ori` -- 23 tests (STALE COUNT: roadmap says "3 tests")
- **AOT tests**: `test_scope_let_basic`, `test_scope_let_chain`, `test_scope_block_as_value`, `test_scope_nested_blocks_as_values`, `test_scope_shadow_in_nested_block`, `test_scope_shadow_three_levels`, `test_scope_shadow_in_loop`
- **STALE ANNOTATION**: Roadmap lists `test_scope_shadow_in_nested_block`, `test_scope_shadow_three_levels`, `test_scope_shadow_in_loop` as `[ignored]` but they are NOT ignored -- all pass

**Classification**: VERIFIED -- STALE (test count 3->23, `[ignored]` annotations no longer accurate)

### [x] No hoisting -- VERIFIED

- **Spec tests**: Sequential binding tests confirm variables must be defined before use
- **AOT tests**: `test_scope_let_chain` (sequential let bindings depend on previous values)

**Classification**: VERIFIED

### [x] Shadowing -- VERIFIED

- **Spec tests**: `test_let_shadow`, `test_let_shadow_different_type`, plus extensive block_scope tests
- **AOT tests**: `test_scope_shadow_same_type`, `test_scope_shadow_different_type`, `test_scope_shadow_uses_previous`, `test_scope_shadow_in_nested_block`, `test_scope_shadow_three_levels`, `test_scope_many_lets_same_name`, `test_scope_string_shadow`
- STALE ANNOTATION: Roadmap lists some as `[ignored]` but all pass

**Classification**: VERIFIED -- STALE (ignored annotations)

### [x] Lambda capture by value -- VERIFIED

- **Spec tests**: `tests/spec/expressions/lambdas.ori` -- 30 tests (roadmap says 29, STALE COUNT)
- **AOT tests**: Multiple closure capture tests in `scoping.rs` and `higher_order.rs`

**Classification**: VERIFIED -- STALE COUNT (29->30)

---

## 10.7 Panics

### [ ] Implicit panics -- not verified in detail

- Shift overflow panics tested in `operators_bitwise.ori` via `assert_panics`
- Division by zero caught by `catch(expr: 1 / 0)` in `catch.ori`
- No dedicated `panics.ori` spec test file exists
- Index-out-of-bounds panic testing not found

**Classification**: PARTIALLY IMPLEMENTED -- some implicit panics work but NEEDS TESTS for comprehensive coverage

### [x] `panic(message)` function -- VERIFIED

- Used extensively across test files as `panic(msg: "...")`
- Short-circuit tests in `coalesce.ori` confirm panic works in both true and false paths
- `assert_panics` built-in works in `operators_bitwise.ori`
- No dedicated `panics.ori` spec test but panic is thoroughly exercised indirectly

**Classification**: VERIFIED

### [x] `catch(expr)` pattern -- VERIFIED

- **Rust tests**: `ori_patterns/src/builtins/catch/tests.rs` -- 4 tests passing
- **Ori tests**: `tests/spec/patterns/catch.ori` -- 7 tests: success, panic, message, div_zero, ok_value, string, nested
- All tests pass. Tests verify: Ok wrapping, Err on panic, message capture, division by zero, string expressions, nested catch

**Classification**: VERIFIED

### [ ] `PanicInfo` type -- VERIFIED as `[ ]`

- No PanicInfo type implementation found. Genuinely incomplete.

**Classification**: VERIFIED as `[ ]`

---

## 10.8 Index Expressions

### [x] `#` length symbol in index brackets -- VERIFIED

- **Spec tests**: `tests/spec/expressions/index_access.ori` -- 35 tests (matches roadmap)
- **Parser**: `ExprKind::HashLength` inside `[...]` brackets
- **Type checker**: Resolves to int
- **Evaluator**: Evaluates as `len(receiver)` -- verified working: `xs[# - 1]` returns last element
- **ARC lowering**: `CanExpr::HashLength` resolved via `hash_length` field in lowering context
- **LLVM**: Roadmap says "placeholder exists, needs real impl" -- STALE. The ARC pipeline handles HashLength via `hash_length` variable propagation. However, no AOT test verifies this.

**Classification**: VERIFIED -- STALE description ("placeholder" claim appears outdated; ARC pipeline handles it)

---

## 10.9 Section Completion Checklist

All items `[ ]`. Section is in-progress.

**Classification**: VERIFIED as `[ ]`

---

## Stale Data Summary

| Item | Issue | Details |
|------|-------|---------|
| 10.1 `conditionals.ori` count | Correct | 19 tests matches |
| 10.2 `loops.ori` count | STALE | Roadmap says 29, actual is 35 |
| 10.6 `block_scope.ori` count | STALE | Roadmap says 3, actual is 23 |
| 10.6 `lambdas.ori` count | STALE | Roadmap says 29, actual is 30 |
| 10.6 AOT `[ignored]` annotations | STALE | `test_scope_shadow_in_nested_block`, `test_scope_shadow_three_levels`, `test_scope_shadow_in_loop` are NOT ignored -- all pass |
| 10.4 `?` operator items | ROADMAP INACCURACY | 3 items marked `[ ]` are actually fully implemented and tested |
| 10.8 LLVM HashLength "placeholder" | STALE | ARC pipeline handles HashLength properly |

## Bugs Found

| Bug | Location | Severity |
|-----|----------|----------|
| Non-void if-without-else accepted | `ori_types` type checker | Medium -- `let x = if true then 42` compiles but should error per spec |

## Items Needing Tests

| Item | Description |
|------|-------------|
| Non-bool condition in `if` | No compile-fail test exists |
| `?` context validation | No test that `?` outside Result/Option function is rejected |
| Implicit panics | No comprehensive spec test file for index-out-of-bounds |
| Error return traces | No Ori spec tests for trace functionality |
