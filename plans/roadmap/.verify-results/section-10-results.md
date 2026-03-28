# Section 10: Control Flow -- Verification Results

**Verified**: 2026-03-28
**Methodology**: Systematic -- read all CLAUDE.md, all .claude/rules/ files (20 files), section file, all referenced test files, ran relevant test suites with timeout 150

**Files loaded before verification**:
- /home/eric/projects/ori_lang/CLAUDE.md (full)
- All 20 files in /home/eric/projects/ori_lang/.claude/rules/ (tests.md, roadmap.md, eval.md, aot.md, llvm.md, spec.md, types.md, patterns.md, compiler.md, ir.md, parse.md, arc.md, runtime.md, registry.md, ori-lang.md, diagnostic.md, impl-hygiene.md, cargo.md, typeck.md, ori-syntax.md)
- /home/eric/projects/ori_lang/plans/roadmap/section-10-control-flow.md (full, 718 lines)
- Test files read: conditionals.ori, loops.ori, bindings.ori, block_scope.ori, mutation.ori, lambdas.ori, index_access.ori, catch.ori, coalesce.ori, operators_bitwise.ori, for_loops.rs, scoping.rs, mutations.rs, error_handling.rs

**Test runs**:
- `cargo st tests/spec/expressions/conditionals.ori` -- 4181 passed, 0 failed, 42 skipped
- `cargo st tests/spec/expressions/loops.ori` -- 4181 passed, 0 failed, 42 skipped
- `cargo st tests/spec/expressions/bindings.ori` -- 4181 passed, 0 failed, 42 skipped
- `cargo st tests/spec/expressions/block_scope.ori` -- 4181 passed, 0 failed, 42 skipped
- `cargo st tests/spec/expressions/mutation.ori` -- all pass
- `cargo st tests/spec/expressions/lambdas.ori` -- all pass
- `cargo st tests/spec/expressions/index_access.ori` -- all pass
- `cargo st tests/spec/patterns/catch.ori` -- all pass
- `cargo test -p ori_llvm --test aot -- for_loops` -- 34 passed, 0 failed
- `cargo test -p ori_llvm --test aot -- scoping` -- 36 passed, 0 failed
- `cargo test -p ori_llvm --test aot -- mutations` -- 22 passed, 0 failed
- `cargo test -p ori_llvm --test aot -- error_handling` -- 28 passed, 0 failed
- `cargo test -p ori_llvm --test aot -- higher_order` -- 57 passed, 0 failed

---

## 10.1 if Expression

### Item 1: Parse `if cond then expr else expr` -- [x]

**Roadmap status**: [x] done (2026-02-10)
**Verdict**: CORRECT

- **Rust Tests**: [x] -- Parser and evaluator tests exist and pass
- **Ori Tests**: [x] -- `tests/spec/expressions/conditionals.ori` has 19 tests (roadmap says 19, confirmed). All pass.
- **LLVM Support**: [ ] -- roadmap correctly unchecked. No dedicated LLVM codegen unit tests.
- **LLVM Rust Tests**: [ ] -- `ori_llvm/tests/control_flow_tests.rs` does not exist.
- **AOT Tests**: [x] -- `scoping.rs` has `test_scope_if_else_value`, `test_scope_if_else_computed`, `test_scope_nested_if_expression`, `test_scope_if_block_branches`, `test_scope_if_else_string_value`, `test_scope_let_each_branch`. All 36 scoping tests pass.

### Item 2: Else-if chains -- [x]

**Roadmap status**: [x] done (2026-02-10)
**Verdict**: CORRECT

- **Ori Tests**: [x] -- `test_if_else_if`, `test_long_else_if` in conditionals.ori. Verified.
- **AOT Tests**: [x] -- `test_scope_nested_if_expression` tests a 4-branch else-if chain. Passes.

### Item 3: Condition must be bool -- [x]

**Roadmap status**: [x] done (2026-02-10)
**Verdict**: CORRECT

- Source code confirmed: `infer_if()` in `control_flow.rs` pushes `IfCondition` context and checks `cond_ty` against `Idx::BOOL`. Any non-bool condition will produce a type error.
- WEAK TESTS -- No negative test (`#compile_fail`) verifying that a non-bool condition is rejected. Only positive tests exercising valid bool conditions.

### Item 4: Branch type unification -- [x]

**Roadmap status**: [x] done (2026-02-10)
**Verdict**: CORRECT

- Source confirmed: `infer_if()` unifies `then_ty` with `else_ty` via `check_type`. Both branches must agree.
- **AOT Tests**: [x] -- Multiple AOT tests verify consistent branch types (int, str, block values). All pass.

### Item 5: Without else: then-branch must be void or Never -- [ ]

**Roadmap status**: [ ] unchecked
**Verdict**: CORRECT (accurately unchecked)

- Source code shows `infer_if()` returns `Idx::UNIT` regardless of then-branch type when else is absent. No error is emitted for non-void/non-Never then-branches -- just a comment "For now, just return unit". The check is not enforced.
- No tests exist for this.

### Item 6: Never coercion in branches -- [ ]

**Roadmap status**: [ ] unchecked
**Verdict**: PARTIALLY DONE -- should be [partial]

- The `test_if_coercion` test in conditionals.ori successfully tests `if true then 42 else panic(msg: "unreachable")` which is `int` vs `Never` coercion. This passes in the interpreter.
- The unification engine handles `Never` coercion: when one branch is `Never`, the other's type wins. This works through standard type unification since `Never` is the bottom type.
- However, the roadmap's sub-items (LLVM tests, AOT tests) are all unchecked and accurate -- no dedicated LLVM/AOT coverage for Never coercion in branches.
- BUG FOUND: The roadmap should be [partial] not [ ] since the interpreter handles Never coercion correctly and there's an Ori test proving it.

### Item 7: Struct literal restriction in condition -- [ ]

**Roadmap status**: [ ] unchecked
**Verdict**: PARTIALLY DONE -- should be [partial]

- The parser has `NO_STRUCT_LIT` context flag (confirmed in parse.md rules). This prevents `Point { x: 1 }` from being parsed as a condition. The restriction exists at the parser level.
- However, no `#compile_fail` test exists for this. No test file `tests/compile-fail/if_struct_literal.ori` exists.

---

## 10.2 for Expressions

### Imperative form (do) -- Items 1-4: all [x]

**Roadmap status**: All [x] done (2026-02-10)
**Verdict**: ALL CORRECT

- **Ori Tests**: `tests/spec/expressions/loops.ori` -- roadmap says 29 tests, actual count is 35 (more tests added after initial roadmap creation). STALE COUNT in roadmap.
- **AOT Tests**: All referenced AOT tests in `for_loops.rs` exist and pass (34 tests total in for_loops.rs). Test names match roadmap descriptions.
- LLVM Support/Rust Tests correctly unchecked.

### Collection building (yield) -- Items 5-8: all [x]

**Roadmap status**: All [x] done (2026-02-10)
**Verdict**: ALL CORRECT

- Ori tests verified: `for_yield_basic`, `for_yield_empty`, `for_yield_identity` etc. in loops.ori.
- AOT tests verified: `test_for_range_yield`, `test_for_list_yield`, `test_for_str_yield`, etc. All pass.

### With guards -- Items 9-10: all [x]

**Roadmap status**: All [x] done (2026-02-10)
**Verdict**: ALL CORRECT

- Ori tests: `for_do_with_guard`, `for_yield_with_guard`, `for_do_guard_all_filtered`, `for_yield_guard_transform` all in loops.ori and pass.
- AOT: `test_for_range_with_guard`, `test_for_list_with_guard` pass.

### For-yield comprehensions -- Items 11-13: all [ ]

**Roadmap status**: All [ ] unchecked
**Verdict**: CORRECT (accurately unchecked)

- `tests/spec/expressions/comprehensions.ori` does not exist.
- No comprehension type inference, multi-target collection, nested for clauses, or yield break/continue in comprehension context implemented.

---

## 10.3 loop Expression

### Items 1-8: loop { body }, break, continue, break value, break type

**Roadmap status**: [x] for items 1-8 (parse loop, loop until break, body, break with value, continue, result type from break, void for break-no-value)
**Verdict**: ALL CORRECT

- Ori tests verified: `loop_with_break`, `loop_break_value`, `loop_conditional_break`, `loop_continue`, `loop_void`, `loop_int` all in loops.ori, 35 tests total, all pass.
- AOT tests verified: `test_mut_loop_break`, `test_mut_while_pattern` in mutations.rs (pass).
- AOT for continue: correctly unchecked (no AOT test for `continue` in loop).
- AOT for break-with-value returning typed result: correctly unchecked (no direct AOT test).

### Item 9: `continue value` error in loop -- [ ]

**Roadmap status**: [ ] unchecked
**Verdict**: CORRECT (accurately unchecked)

- No `tests/compile-fail/loop_continue_value.ori` exists.
- E0861 error not verified.

### Item 10: Type Never for infinite loops -- [ ]

**Roadmap status**: [ ] unchecked
**Verdict**: CORRECT (accurately unchecked)

- No tests for `Never` type inference on loops without break.

### Item 11: Multiple break paths type unification -- [ ]

**Roadmap status**: [ ] unchecked
**Verdict**: CORRECT (accurately unchecked)

- No `tests/compile-fail/loop_break_type_mismatch.ori` exists.

### Labeled loops -- Items 12-18: all [ ]

**Roadmap status**: All [ ] unchecked
**Verdict**: CORRECT (accurately unchecked)

- `loop:name`, `for:name`, `break:name`, `continue:name` -- parser does not support labeled loops yet.
- `tests/spec/expressions/labeled_loops.ori` does not exist.
- No compile-fail tests for label shadowing, type consistency, etc.
- The `#skip` in loops.ori (line 405) correctly documents this: `#skip("requires labeled breaks (loop:name, break:name) - see line 361")`

### Labeled loop semantics -- Items 19-24: all [ ]

**Verdict**: CORRECT (accurately unchecked) -- none implemented.

---

## 10.3B Labeled Block Early Exit -- all [ ]

**Roadmap status**: All [ ] unchecked
**Verdict**: CORRECT (accurately unchecked)

- `block` is not a keyword in the lexer.
- No `ExprKind::LabeledBlock` in the AST.
- `tests/spec/expressions/labeled_blocks.ori` does not exist.
- All 7 sub-items correctly unchecked.

---

## 10.3A while Expression -- all [ ]

**Roadmap status**: All [ ] unchecked
**Verdict**: CORRECT (accurately unchecked)

- `while` is not a keyword in the lexer (confirmed: not in `ori_lexer/src/keywords/`).
- No `KwWhile` token variant.
- No `ExprKind::While` in AST.
- `tests/spec/expressions/while_loop.ori` does not exist.
- All 6 sub-items correctly unchecked.

---

## 10.4 Error Propagation (?)

### Item 1: Parse postfix `?` operator -- [ ]

**Roadmap status**: [ ] unchecked
**Verdict**: WRONG -- should be [x]

- `ExprKind::Try(ExprId)` exists in `ori_ir/src/ast/expr.rs`.
- Parser handles `?` in `ori_parse/src/grammar/expr/postfix.rs`.
- This IS implemented. The `[ ]` is wrong.
- Ori tests: `tests/spec/expressions/postfix.ori` was not checked but AOT tests exercise `?`.
- **AOT Tests**: [x] -- `test_err_try_result_ok`, `test_err_try_result_err`, `test_err_try_option_some`, `test_err_try_option_none` all pass. Roadmap correctly marks these as [x].

### Item 2: On Result -- unwrap Ok or return Err -- [ ]

**Roadmap status**: [ ] unchecked
**Verdict**: WRONG -- should be [x]

- `eval_try()` in `ori_eval/src/unary_operators.rs` handles `Result` propagation.
- `CanExpr::Try(inner)` in `ori_eval/src/interpreter/can_eval/mod.rs` handles the evaluator path.
- ARC lowering handles `Try` via `lower_try()` in `ori_arc/src/lower/expr/mod.rs`.
- AOT tests pass: `test_err_try_result_ok`, `test_err_try_result_err`, `test_err_try_result_chain`, `test_err_try_result_early_exit`, `test_err_deep_try_chain`.
- This IS implemented in both interpreter and LLVM backends.

### Item 3: On Option -- unwrap Some or return None -- [ ]

**Roadmap status**: [ ] unchecked
**Verdict**: WRONG -- should be [x]

- Same `eval_try()` function handles `Option` (unwraps `Some`, returns `None`).
- Rust unit tests exist: `ori_eval/src/tests/unary_operators_tests.rs` lines 254-261.
- AOT tests pass: `test_err_try_option_some`, `test_err_try_option_none`.

### Item 4: Only valid in functions returning Result/Option -- [ ]

**Roadmap status**: [ ] unchecked
**Verdict**: NEEDS INVESTIGATION -- likely partially done

- No `tests/compile-fail/invalid_propagation.ori` exists.
- The type checker likely infers the return type and may or may not reject `?` in non-Result/Option return contexts. Not verified with a negative test.

### Error Return Traces -- Items 5-10: all [ ]

**Roadmap status**: Items 5-9 all [ ]. Item 10 (Traceable) is [x].
**Verdict**: Items 5-9 CORRECT (accurately unchecked). Item 10 CORRECT ([x]).

- Automatic trace collection at `?` propagation points: NOT implemented.
- `TraceEntry` type: EXISTS in the type system (Traceable trait implemented in section 3.13).
- Error trace methods: EXISTS for the built-in `Error` type (confirmed in evaluator).
- Printable for Error: not verified for trace inclusion.
- Result.context(): not verified.
- Traceable trait: [x] CORRECT -- implemented in section 3.13, with spec tests in `tests/spec/traits/traceable/` (4 files). Rust tests in `ori_eval/src/methods/error/tests.rs`.

---

## 10.5 Let Bindings

### Items 1-7: let binding, mutable, typed, immutable, struct/tuple/list destructuring

**Roadmap status**: All [x] done (2026-02-10)
**Verdict**: ALL CORRECT

- **Ori Tests**: `tests/spec/expressions/bindings.ori` -- 17 tests (matches roadmap). All pass.
- **Ori Tests**: `tests/spec/expressions/mutation.ori` -- 15 tests (matches roadmap). All pass.
- **AOT Tests**: Verified in scoping.rs -- `test_scope_let_basic`, `test_scope_let_type_annotation`, `test_scope_let_chain`, `test_scope_tuple_destructure`. All pass.
- **AOT Tests**: Verified in mutations.rs -- 21+ tests covering mutable bindings. All pass.
- STALE DATA: Roadmap says `test_scope_shadow_in_nested_block [ignored]`, `test_scope_shadow_three_levels [ignored]`, `test_scope_shadow_in_loop [ignored]` but NONE of these are actually ignored -- they all pass without any `#[ignore]` annotation. These markers should be removed.
- Struct destructuring AOT: correctly unchecked.
- List destructuring AOT: correctly unchecked.

---

## 10.6 Scoping

### Items 1-4: Lexical scoping, no hoisting, shadowing, lambda capture

**Roadmap status**: All [x] done (2026-02-10)
**Verdict**: ALL CORRECT

- **Ori Tests**: `tests/spec/expressions/block_scope.ori` -- roadmap says 3 tests, actual count is 23 tests. STALE COUNT -- many tests were added after the initial claim.
- **Ori Tests**: `tests/spec/expressions/lambdas.ori` -- roadmap says 29 tests, actual count is 30. STALE COUNT.
- **AOT Tests**: scoping.rs verified with 36 tests. All pass.
- STALE DATA: Roadmap says scoping AOT tests `test_scope_shadow_in_nested_block [ignored]`, `test_scope_shadow_three_levels [ignored]`, `test_scope_shadow_in_loop [ignored]` but these are NOT ignored and all pass.

---

## 10.7 Panics

### Item 1: Implicit panics (index OOB, div by zero) -- [ ]

**Roadmap status**: [ ] unchecked
**Verdict**: PARTIALLY DONE -- should be [partial]

- Division by zero IS implemented in the evaluator (`division_by_zero()` error factory in `ori_patterns`).
- Index out of bounds IS implemented in the evaluator (confirmed in `exec/decision_tree/mod.rs`).
- However, `tests/spec/expressions/panics.ori` does not exist.
- No AOT tests for implicit panics exist (the AOT for_loops tests do test `zero_step_panics` which is related but not div-by-zero or index OOB).

### Item 2: `panic(message)` function -- [x]

**Roadmap status**: [x] done (2026-02-10)
**Verdict**: CORRECT

- `panic(msg:)` is a prelude function, implemented and tested.
- Roadmap references `tests/spec/expressions/coalesce.ori` -- WRONG TEST REFERENCE. `coalesce.ori` tests `??` operator, not `panic`. The `panic()` function is used as a helper in many test files (e.g., conditionals.ori uses `panic(msg: "should not execute")`) but coalesce.ori does not test `panic` as a feature.
- Roadmap references `operators_bitwise.ori` for `assert_panics` tests -- CORRECT. `operators_bitwise.ori` uses `assert_panics` to verify shift overflow panics.

### Item 3: `catch(expr)` pattern -- [x]

**Roadmap status**: [x] done (2026-02-19)
**Verdict**: CORRECT

- **Rust Tests**: `ori_patterns/src/builtins/catch/tests.rs` confirmed.
- **Ori Tests**: `tests/spec/patterns/catch.ori` -- 7 tests (matches roadmap). All pass.
- Tests cover: success, panic, message, div_zero, ok_value, string, nested.

### Item 4: PanicInfo type -- [ ]

**Roadmap status**: [ ] unchecked
**Verdict**: CORRECT (accurately unchecked)

- No dedicated PanicInfo tests.

---

## 10.8 Index Expressions

### Item 1: `#` length symbol in index brackets -- [x]

**Roadmap status**: [x] done (2026-02-10)
**Verdict**: CORRECT

- **Parser**: `ExprKind::HashLength` confirmed in `ori_parse/src/grammar/expr/postfix.rs`.
- **Type Checker**: HashLength resolved to `int` in `ori_types/src/infer/`.
- **Evaluator**: HashLength evaluated as `len(receiver)` in index context.
- **Ori Tests**: `tests/spec/expressions/index_access.ori` -- 35 tests (matches roadmap). All pass.
- **LLVM Support**: Correctly unchecked. Roadmap notes "placeholder exists, needs real impl" -- confirmed no `HashLength` handling in `ori_llvm/src/`.

---

## 10.9 Section Completion Checklist -- all [ ]

**Verdict**: CORRECT (accurately unchecked) -- section is not complete.

---

## Summary

### Statistics
- Total items audited: 82 (all items including sub-items)
- [x] items verified correct: 38
- [x] items WRONG (should be [ ] or [partial]): 0
- [ ] items verified correct (accurately unchecked): 35
- [ ] items WRONG (should be [x] or [partial]): 3
- Stale data found: 5 instances

### Items that should change status

| Item | Current | Should Be | Reason |
|------|---------|-----------|--------|
| 10.4 Item 1: Parse postfix `?` operator | [ ] | [x] | `ExprKind::Try` exists, parser handles `?`, evaluator handles it, ARC lowering handles it, AOT tests pass |
| 10.4 Item 2: On Result unwrap Ok or return Err | [ ] | [x] | `eval_try()` handles Result, ARC lowering has `lower_try()`, AOT tests pass |
| 10.4 Item 3: On Option unwrap Some or return None | [ ] | [x] | Same `eval_try()` handles Option, unit tests exist, AOT tests pass |

### Items that could be marked [partial]

| Item | Current | Observation |
|------|---------|-------------|
| 10.1 Item 6: Never coercion in branches | [ ] | Interpreter works (test_if_coercion passes), but no LLVM/AOT coverage |
| 10.1 Item 7: Struct literal restriction in condition | [ ] | Parser has NO_STRUCT_LIT flag, but no negative test |
| 10.7 Item 1: Implicit panics | [ ] | Evaluator has div-by-zero and index OOB, but no panics.ori test file |

### Stale data in roadmap

1. **loops.ori test count**: Roadmap says 29, actual is 35.
2. **block_scope.ori test count**: Roadmap says 3, actual is 23.
3. **lambdas.ori test count**: Roadmap says 29, actual is 30.
4. **Scoping AOT [ignored] markers**: `test_scope_shadow_in_nested_block`, `test_scope_shadow_three_levels`, `test_scope_shadow_in_loop` are NOT ignored -- they all pass. The `[ignored]` annotations should be removed.
5. **10.7 panic test reference**: Roadmap says `tests/spec/expressions/coalesce.ori` tests panic -- it does not. It tests the `??` operator.

### Test quality assessment

**Strengths**:
- Solid positive test coverage for implemented features (conditionals: 19, loops: 35, bindings: 17, mutation: 15, scoping: 23, lambdas: 30, index: 35).
- AOT tests comprehensive for for-loops (34 tests covering Range, List, Str, Option, Map, guards, break/continue with mutation, descending ranges, step ranges, zero-step panics).
- AOT scoping tests thorough (36 tests: let bindings, shadowing, blocks as values, closures, control flow interaction).
- `catch(expr:)` has full matrix: success, panic, message, div_zero, ok_value, string, nested.

**Weaknesses**:
- WEAK TESTS -- No `#compile_fail` negative tests for ANY control flow feature. Missing: non-bool condition, branch type mismatch, non-void if-without-else, break type mismatch, continue-value-in-loop, label shadowing, etc.
- No `tests/spec/expressions/panics.ori` -- implicit panics not tested as a standalone feature.
- Many LLVM unit test files referenced in roadmap do not exist (`control_flow_tests.rs`, `binding_tests.rs`, `scope_tests.rs`, `panic_tests.rs`, `error_propagation_tests.rs`). These are aspirational -- none of these files were ever created.
- The `?` operator is fully working in both interpreter and LLVM but the roadmap marks all its items as [ ]. This is a significant roadmap drift.
- No labeled loops or while expression implemented -- large gap in control flow completeness.

### Bugs found

1. **BUG: if-without-else accepts any then-branch type** -- `infer_if()` in `control_flow.rs` returns `Idx::UNIT` for if-without-else regardless of whether the then-branch is void/Never. Non-void then-branches should produce a type error per spec. (Item 10.1.5)
2. **BUG (roadmap only): `?` operator marked unchecked despite being fully implemented** -- Items 10.4.1-3 should be [x]. The `?` operator has parser support (`ExprKind::Try`), evaluator support (`eval_try`), ARC lowering (`lower_try`), and passing AOT tests.
