---
section: 10
title: Control Flow
status: in-progress
reviewed: false
tier: 3
goal: Complete control flow constructs
spec:
  - spec/14-expressions.md
  - spec/11-blocks-and-scope.md
  - spec/16-control-flow.md
  - spec/17-errors-and-panics.md
sections:
  - id: "10.1"
    title: if Expression
    status: in-progress
  - id: "10.2"
    title: for Expressions
    status: in-progress
  - id: "10.3"
    title: loop Expression
    status: in-progress
  - id: "10.3B"
    title: Labeled Block Early Exit
    status: not-started
  - id: "10.3A"
    title: while Expression
    status: not-started
  - id: "10.4"
    title: Error Propagation (?)
    status: in-progress
  - id: "10.5"
    title: Let Bindings
    status: in-progress
  - id: "10.6"
    title: Scoping
    status: in-progress
  - id: "10.7"
    title: Panics
    status: in-progress
  - id: "10.8"
    title: Index Expressions
    status: in-progress
  - id: "10.9"
    title: Section Completion Checklist
    status: not-started
---

# Section 10: Control Flow

**Goal**: Complete control flow constructs

> **SPEC**: `spec/14-expressions.md`, `spec/11-blocks-and-scope.md`, `spec/16-control-flow.md`, `spec/17-errors-and-panics.md`
> **PROPOSALS**:
> - `proposals/approved/if-expression-proposal.md` — Conditional expression semantics
> - `proposals/approved/error-return-traces-proposal.md` — Automatic error trace collection
> - `proposals/approved/loop-expression-proposal.md` — Loop expression semantics
> - `proposals/approved/while-loop-proposal.md` — While loop expression (sugar over loop)
> - `proposals/approved/labeled-block-early-exit-proposal.md` — Labeled block early exit (break:label from blocks)

---

## 10.1 if Expression

**Proposal**: `proposals/approved/if-expression-proposal.md`

- [x] **Implement**: Parse `if cond then expr else expr` — spec/14-expressions.md § Conditional [done] (2026-02-10)
  - [x] **Rust Tests**: Parser and evaluator — if expression
  - [x] **Ori Tests**: `tests/spec/expressions/conditionals.ori` — 19 tests
  - [ ] **LLVM Support**: LLVM codegen for if expression
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — if expression codegen
  - [x] **AOT Tests**: `ori_llvm/tests/aot/scoping.rs` — `test_scope_if_else_value`, `test_scope_if_else_computed`, `test_scope_nested_if_expression`, `test_scope_if_block_branches`, `test_scope_if_else_string_value`, `test_scope_let_each_branch` (if-then-else as expression in value position)

- [x] **Implement**: Else-if chains (grammar convenience) — spec/14-expressions.md § Conditional [done] (2026-02-10)
  - [x] **Rust Tests**: Parser — chained if parsing
  - [x] **Ori Tests**: `tests/spec/expressions/conditionals.ori`
  - [ ] **LLVM Support**: LLVM codegen for chained conditions
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — chained conditions codegen
  - [x] **AOT Tests**: `ori_llvm/tests/aot/scoping.rs` — `test_scope_nested_if_expression` (else-if chain with 4 branches: `x > 20`, `x > 10`, `x > 0`, else)

- [x] **Implement**: Condition must be `bool` — spec/14-expressions.md § Conditional [done] (2026-02-10)
  - [x] **Rust Tests**: Type checker — condition type checking
  - [x] **Ori Tests**: `tests/spec/expressions/conditionals.ori`
  - [ ] **LLVM Support**: N/A (compile-time check)
  - [ ] **LLVM Rust Tests**: N/A

- [x] **Implement**: Branch type unification — spec/14-expressions.md § Conditional [done] (2026-02-10)
  - [x] **Rust Tests**: Type checker — branch type unification
  - [x] **Ori Tests**: `tests/spec/expressions/conditionals.ori`
  - [ ] **LLVM Support**: LLVM codegen for branch type unification
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — branch type unification codegen
  - [x] **AOT Tests**: `ori_llvm/tests/aot/scoping.rs` — `test_scope_if_else_value`, `test_scope_if_else_computed`, `test_scope_if_block_branches`, `test_scope_if_else_string_value`, `test_scope_let_each_branch` (if-else branches producing same type: int, string, block values)

- [ ] **Implement**: Without else: then-branch must be `void` or `Never` — spec/14-expressions.md § Conditional
  - [ ] **Rust Tests**: `ori_types/src/infer/expr/mod.rs` — void branch validation
  - [ ] **Ori Tests**: `tests/spec/expressions/conditionals.ori`
  - [ ] **LLVM Support**: N/A (compile-time check)
  - [ ] **LLVM Rust Tests**: N/A

- [ ] **Implement**: Never coercion in branches — spec/14-expressions.md § Conditional
  - [ ] **Rust Tests**: `ori_types/src/infer/expr/mod.rs` — Never coercion
  - [ ] **Ori Tests**: `tests/spec/expressions/conditionals.ori`
  - [ ] **LLVM Support**: LLVM codegen for diverging branches
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — Never coercion codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Struct literal restriction in condition — spec/14-expressions.md § Conditional
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr.rs` — struct literal exclusion
  - [ ] **Ori Tests**: `tests/compile-fail/if_struct_literal.ori`
  - [ ] **LLVM Support**: N/A (parse-time check)
  - [ ] **LLVM Rust Tests**: N/A

---

## 10.2 for Expressions

> **NOTE**: This is the `for x in items do/yield expr` **expression** syntax for iteration.
> The `for(over:, match:, default:)` **pattern** with named arguments is a separate construct in Section 8.

**Imperative form (do):**

- [x] **Implement**: Parse `for x in items do expr` — spec/14-expressions.md § For Expressions [done] (2026-02-10)
  - [x] **Rust Tests**: Parser — for-do parsing
  - [x] **Ori Tests**: `tests/spec/expressions/loops.ori` — 29 tests
  - [ ] **LLVM Support**: LLVM codegen for for-do expression
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — for-do codegen
  - [x] **AOT Tests**: `ori_llvm/tests/aot/for_loops.rs` — `test_for_range_sum`, `test_for_range_inclusive`, `test_for_range_empty`, `test_for_list_sum`, `test_for_list_empty`, `test_for_str_count_chars`, `test_for_str_empty`, `test_for_option_some`, `test_for_option_none`, `test_for_map_sum`, `test_for_map_entries` (for-do over Range, List, Str, Option, Map including empty iterables)

- [x] **Implement**: Bind loop variable — spec/14-expressions.md § For Expressions [done] (2026-02-10)
  - [x] **Rust Tests**: Evaluator — loop variable binding
  - [x] **Ori Tests**: `tests/spec/expressions/loops.ori`
  - [ ] **LLVM Support**: LLVM codegen for loop variable binding
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/binding_tests.rs` — loop variable binding codegen
  - [x] **AOT Tests**: `ori_llvm/tests/aot/for_loops.rs` — `test_for_range_sum`, `test_for_range_with_guard`, `test_for_list_sum`, `test_for_list_with_guard`, `test_for_str_count_chars`, `test_for_str_char_values`, `test_for_option_some`, `test_for_map_sum` (loop variable bound and used in body/guard across Range, List, Str, Option, Map)

- [x] **Implement**: Execute body for side effects — spec/14-expressions.md § For Expressions [done] (2026-02-10)
  - [x] **Rust Tests**: Evaluator — body execution
  - [x] **Ori Tests**: `tests/spec/expressions/loops.ori`
  - [ ] **LLVM Support**: LLVM codegen for loop body execution
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — loop body execution codegen
  - [x] **AOT Tests**: `ori_llvm/tests/aot/for_loops.rs` — `test_for_range_sum`, `test_for_list_sum`, `test_for_str_count_chars`, `test_for_map_sum`, `test_for_map_entries` (loop body executes for side effects across all iterable types), `ori_llvm/tests/aot/mutations.rs` — `test_mut_loop_counter`, `test_mut_loop_accumulator`, `test_mut_loop_product`, `test_mut_loop_conditional_accumulator` (loop body with accumulation patterns)

- [x] **Implement**: Result type `void` — spec/14-expressions.md § For Expressions [done] (2026-02-10)
  - [x] **Rust Tests**: Type checker — for-do type
  - [x] **Ori Tests**: `tests/spec/expressions/loops.ori`
  - [ ] **LLVM Support**: LLVM codegen for for-do void type
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — for-do void type codegen
  - [x] **AOT Tests**: `ori_llvm/tests/aot/for_loops.rs` — `test_for_range_sum`, `test_for_list_sum`, `test_for_str_count_chars`, `test_for_option_some`, `test_for_option_none`, `test_for_map_sum` (for-do expressions used for side effects, result discarded)

**Collection building (yield):**

- [x] **Implement**: Parse `for x in items yield expr` — spec/14-expressions.md § For Expressions [done] (2026-02-10)
  - [x] **Rust Tests**: Parser — for-yield parsing
  - [x] **Ori Tests**: `tests/spec/expressions/loops.ori`
  - [ ] **LLVM Support**: LLVM codegen for for-yield expression
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — for-yield codegen
  - [x] **AOT Tests**: `ori_llvm/tests/aot/for_loops.rs` — `test_for_range_yield`, `test_for_list_yield`, `test_for_str_yield`, `test_for_option_yield_some`, `test_for_option_yield_none`, `test_for_map_yield`, `test_for_list_with_guard` (for-yield over Range, List, Str, Option, Map)

- [x] **Implement**: Collect results into list — spec/14-expressions.md § For Expressions [done] (2026-02-10)
  - [x] **Rust Tests**: Evaluator — yield collection
  - [x] **Ori Tests**: `tests/spec/expressions/loops.ori`
  - [ ] **LLVM Support**: LLVM codegen for yield collection
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — yield collection codegen
  - [x] **AOT Tests**: `ori_llvm/tests/aot/for_loops.rs` — `test_for_range_yield`, `test_for_list_yield`, `test_for_str_yield`, `test_for_option_yield_some`, `test_for_option_yield_none`, `test_for_map_yield` (yield collects results into list across all iterable types)

- [x] **Implement**: Result type `[T]` — spec/14-expressions.md § For Expressions [done] (2026-02-10)
  - [x] **Rust Tests**: Type checker — for-yield type
  - [x] **Ori Tests**: `tests/spec/expressions/loops.ori`
  - [ ] **LLVM Support**: LLVM codegen for for-yield list type
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — for-yield list type codegen
  - [x] **AOT Tests**: `ori_llvm/tests/aot/for_loops.rs` — `test_for_range_yield`, `test_for_list_yield`, `test_for_str_yield`, `test_for_option_yield_some`, `test_for_option_yield_none`, `test_for_map_yield` (yield produces list, verified via `.length()`)

**With guards:**

- [x] **Implement**: Parse `for x in items if guard yield expr` — spec/14-expressions.md § For Expressions [done] (2026-02-10)
  - [x] **Rust Tests**: Parser — for-guard parsing
  - [x] **Ori Tests**: `tests/spec/expressions/loops.ori` — for_do_with_guard, for_yield_with_guard tests
  - [ ] **LLVM Support**: LLVM codegen for for-guard expression
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — for-guard codegen
  - [x] **AOT Tests**: `ori_llvm/tests/aot/for_loops.rs` — `test_for_range_with_guard`, `test_for_list_with_guard` (for-if-do and for-if-yield guard parsing and execution)

- [x] **Implement**: Only yield when guard true — spec/14-expressions.md § For Expressions [done] (2026-02-10)
  - [x] **Rust Tests**: Evaluator — guard filtering
  - [x] **Ori Tests**: `tests/spec/expressions/loops.ori` — guard_all_filtered, guard_transform tests
  - [ ] **LLVM Support**: LLVM codegen for guard filtering
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — guard filtering codegen
  - [x] **AOT Tests**: `ori_llvm/tests/aot/for_loops.rs` — `test_for_range_with_guard` (range guard filtering), `test_for_list_with_guard` (list guard filtering with yield)

**For-yield comprehensions** (proposals/approved/for-yield-comprehensions-proposal.md):

- [ ] **Implement**: Type inference for collection target — proposals/approved/for-yield-comprehensions-proposal.md § Type Inference
  - [ ] Infer from context (`let list: [int] = for ...`)
  - [ ] Default to list when no context
  - [ ] **Rust Tests**: `ori_types/src/infer/expr/mod.rs` — for-yield type inference
  - [ ] **Ori Tests**: `tests/spec/expressions/comprehensions.ori`
  - [ ] **LLVM Support**: LLVM codegen for type-directed collection
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — comprehension type inference
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Collect into any `Collect<T>` type — proposals/approved/for-yield-comprehensions-proposal.md § Collect Target
  - [ ] Support `Set<T>` collection
  - [ ] Support `{K: V}` collection via `Collect<(K, V)>`
  - [ ] Duplicate map keys overwrite earlier values
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/loops.rs` — multi-target collection
  - [ ] **Ori Tests**: `tests/spec/expressions/comprehensions.ori`
  - [ ] **LLVM Support**: LLVM codegen for multi-target collection
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — multi-target collection codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Nested `for` clauses — proposals/approved/for-yield-comprehensions-proposal.md § Nested Comprehensions
  - [ ] Parse `for x in xs for y in ys yield expr`
  - [ ] Desugar to `flat_map`
  - [ ] Support filters on each clause
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr.rs` — nested for parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/comprehensions.ori`
  - [ ] **LLVM Support**: LLVM codegen for nested comprehensions
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — nested comprehensions codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Break/continue in yield context — proposals/approved/for-yield-comprehensions-proposal.md § Break and Continue
  - [ ] `continue` skips current element
  - [ ] `continue value` substitutes value for yield expression
  - [ ] `break` stops iteration, collects results so far
  - [ ] `break value` stops and adds final value
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/loops.rs` — yield break/continue
  - [ ] **Ori Tests**: `tests/spec/expressions/comprehensions.ori`
  - [ ] **LLVM Support**: LLVM codegen for yield break/continue
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — yield break/continue codegen
  - [ ] **AOT Tests**: No AOT coverage yet

---

## 10.3 loop Expression

**Proposal**: `proposals/approved/loop-expression-proposal.md`

- [x] **Implement**: Parse `loop { body }` — spec/14-expressions.md § Loop Expressions [done] (2026-02-10)
  - [x] **Rust Tests**: Parser — loop parsing
  - [x] **Ori Tests**: `tests/spec/expressions/loops.ori` — loop_with_break, loop_break_value, loop_int tests
  - [ ] **LLVM Support**: LLVM codegen for loop expression
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — loop expression codegen
  - [x] **AOT Tests**: `ori_llvm/tests/aot/mutations.rs` — `test_mut_loop_break`, `test_mut_while_pattern` (loop expression with break and mutation)

- [x] **Implement**: Loop until `break` — spec/16-control-flow.md § Break [done] (2026-02-10)
  - [x] **Rust Tests**: Evaluator — break handling
  - [x] **Ori Tests**: `tests/spec/expressions/loops.ori` — loop_with_break test
  - [ ] **LLVM Support**: LLVM codegen for break handling
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — break handling codegen
  - [x] **AOT Tests**: `ori_llvm/tests/aot/mutations.rs` — `test_mut_loop_break` (loop until break), `test_mut_while_pattern` (Collatz loop with conditional break)

- [x] **Implement**: Body is a block expression `loop { ... }` — proposals/approved/loop-expression-proposal.md § Body [done] (2026-02-10)
  - [x] **Rust Tests**: Parser — loop body parsing
  - [x] **Ori Tests**: `tests/spec/expressions/loops.ori` — all loop tests use `loop { ... }`
  - [ ] **LLVM Support**: LLVM codegen for loop body
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — loop body codegen
  - [x] **AOT Tests**: `ori_llvm/tests/aot/mutations.rs` — `test_mut_loop_break`, `test_mut_while_pattern` (loop body with mutation, conditionals, and break)

- [x] **Implement**: Parse `break` with optional value — spec/16-control-flow.md § Break [done] (2026-02-10)
  - [x] **Rust Tests**: Parser — break parsing
  - [x] **Ori Tests**: `tests/spec/expressions/loops.ori` — loop_break_value, loop_conditional_break tests
  - [ ] **LLVM Support**: LLVM codegen for break with value
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — break with value codegen
  - [x] **AOT Tests**: `ori_llvm/tests/aot/mutations.rs` — `test_mut_loop_break` (break without value), `test_mut_while_pattern` (conditional break in loop)

- [x] **Implement**: Parse `continue` — spec/16-control-flow.md § Continue [done] (2026-02-10)
  - [x] **Rust Tests**: Parser — continue parsing
  - [x] **Ori Tests**: `tests/spec/expressions/loops.ori` — loop_continue test
  - [ ] **LLVM Support**: LLVM codegen for continue
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — continue codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `continue value` error in loop — proposals/approved/loop-expression-proposal.md § Continue With Value
  - [ ] Error E0861 when continue has value in loop context
  - [ ] Helpful suggestion to use break or remove value
  - [ ] **Rust Tests**: `ori_types/src/check/loops.rs` — continue value validation
  - [ ] **Ori Tests**: `tests/compile-fail/loop_continue_value.ori`
  - [ ] **LLVM Support**: N/A (compile-time check)
  - [ ] **LLVM Rust Tests**: N/A

- [x] **Implement**: Result type from `break` value — proposals/approved/loop-expression-proposal.md § Loop Type [done] (2026-02-10)
  - [x] **Rust Tests**: Type checker — break type inference
  - [x] **Ori Tests**: `tests/spec/expressions/loops.ori` — loop_break_value, loop_int tests
  - [ ] **LLVM Support**: LLVM codegen for break type inference
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — break type inference codegen
  - [ ] **AOT Tests**: No AOT coverage yet (loop break-with-value returning typed result not directly tested)

- [x] **Implement**: Type `void` for break without value — proposals/approved/loop-expression-proposal.md § Break Without Value [done] (2026-02-10)
  - [x] **Rust Tests**: Type checker — void loop type
  - [x] **Ori Tests**: `tests/spec/expressions/loops.ori` — loop_with_break (void function)
  - [ ] **LLVM Support**: LLVM codegen for void loop
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — void loop codegen
  - [x] **AOT Tests**: `ori_llvm/tests/aot/mutations.rs` — `test_mut_loop_break` (loop with break, no value — void result type)

- [ ] **Implement**: Type `Never` for infinite loops — proposals/approved/loop-expression-proposal.md § Infinite Loop Type
  - [ ] Loop with no break has type Never
  - [ ] Coerces to any type in value context
  - [ ] **Rust Tests**: `ori_types/src/infer/expr/mod.rs` — Never loop type
  - [ ] **Ori Tests**: `tests/spec/expressions/loops.ori`
  - [ ] **LLVM Support**: LLVM codegen for Never loop
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — Never loop codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Multiple break paths type unification — proposals/approved/loop-expression-proposal.md § Multiple Break Paths
  - [ ] All breaks must have compatible types
  - [ ] Error E0860 for type mismatch
  - [ ] **Rust Tests**: `ori_types/src/infer/expr/mod.rs` — break type unification
  - [ ] **Ori Tests**: `tests/compile-fail/loop_break_type_mismatch.ori`
  - [ ] **LLVM Support**: LLVM codegen for break unification
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — break unification codegen
  - [ ] **AOT Tests**: No AOT coverage yet

**Labeled loops:**

- [ ] **Implement**: Parse `loop:name { body }` — spec/16-control-flow.md § Labeled Loops
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr.rs` — labeled loop parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/loops.ori`
  - [ ] **LLVM Support**: LLVM codegen for labeled loop
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — labeled loop codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Parse `for:name x in items` — spec/16-control-flow.md § Labeled Loops
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr.rs` — labeled for parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/loops.ori`
  - [ ] **LLVM Support**: LLVM codegen for labeled for
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — labeled for codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Parse `break:name` and `continue:name` — spec/16-control-flow.md § Label Reference
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr.rs` — label reference parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/loops.ori`
  - [ ] **LLVM Support**: LLVM codegen for label references
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — label references codegen
  - [ ] **AOT Tests**: No AOT coverage yet

**Labeled loop semantics** (proposals/approved/labeled-loops-proposal.md):

- [ ] **Implement**: Label scope rules — proposals/approved/labeled-loops-proposal.md § Label Scope
  - [ ] Labels visible only within their loop body
  - [ ] No language-imposed nesting depth limit
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/loops.rs` — label scope validation
  - [ ] **Ori Tests**: `tests/spec/expressions/labeled_loops.ori`
  - [ ] **LLVM Support**: LLVM codegen for label scope
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — label scope codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: No label shadowing — proposals/approved/labeled-loops-proposal.md § No Shadowing
  - [ ] Error if label already in scope
  - [ ] Error E0871 with helpful suggestion
  - [ ] **Rust Tests**: `ori_types/src/check/labels.rs` — shadowing detection
  - [ ] **Ori Tests**: `tests/compile-fail/labeled_loop_shadow.ori`
  - [ ] **LLVM Support**: N/A (compile-time check)
  - [ ] **LLVM Rust Tests**: N/A

- [ ] **Implement**: Type consistency for `break:label value` — proposals/approved/labeled-loops-proposal.md § Type Consistency
  - [ ] All break paths for a labeled loop must produce same type
  - [ ] Error E0872 for type mismatch
  - [ ] **Rust Tests**: `ori_types/src/infer/expr/mod.rs` — break type unification
  - [ ] **Ori Tests**: `tests/compile-fail/labeled_break_type.ori`
  - [ ] **LLVM Support**: LLVM codegen for typed break
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — typed break codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `continue:label value` in for-yield — proposals/approved/labeled-loops-proposal.md § Continue With Value in For-Yield
  - [ ] Value type must match target loop's yield element type
  - [ ] Inner loop's partial collection discarded
  - [ ] Value contributed to outer loop's collection
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/loops.rs` — continue value in yield
  - [ ] **Ori Tests**: `tests/spec/expressions/labeled_loops.ori`
  - [ ] **LLVM Support**: LLVM codegen for continue value
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — continue value codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `continue:label value` error in for-do — proposals/approved/labeled-loops-proposal.md § Continue With Value in For-Do
  - [ ] Error E0873 when continue has value in for-do context
  - [ ] Helpful suggestion to use for-yield or remove value
  - [ ] **Rust Tests**: `ori_types/src/check/loops.rs` — continue value validation
  - [ ] **Ori Tests**: `tests/compile-fail/labeled_continue_in_do.ori`
  - [ ] **LLVM Support**: N/A (compile-time check)
  - [ ] **LLVM Rust Tests**: N/A

- [ ] **Implement**: Undefined label error — proposals/approved/labeled-loops-proposal.md § Error Messages
  - [ ] Error E0870 for undefined label
  - [ ] Suggest similar labels if available
  - [ ] **Rust Tests**: `ori_types/src/check/labels.rs` — undefined label detection
  - [ ] **Ori Tests**: `tests/compile-fail/undefined_label.ori`
  - [ ] **LLVM Support**: N/A (compile-time check)
  - [ ] **LLVM Rust Tests**: N/A

---

## 10.3B Labeled Block Early Exit

**Proposal**: `proposals/approved/labeled-block-early-exit-proposal.md`

`block:name { ... break:name value ... }` — early exit from named blocks without `return`. Extends the label system to plain blocks.

- [ ] **Implement**: Add `block` as context-sensitive keyword — spec/07-lexical-elements.md § 7.3.3
  - [ ] **Lexer**: Recognize `block` as context-sensitive keyword in `ori_lexer/src/keywords/mod.rs`
  - [ ] **Rust Tests**: Lexer keyword recognition test
  - [ ] **Ori Tests**: `tests/spec/lexical/keywords.ori` — verify `block` is context-sensitive (valid as identifier, keyword before `:`)

- [ ] **Implement**: Parse `block:name { body }` — grammar.ebnf § labeled_block
  - [ ] Parser produces `ExprKind::LabeledBlock { label, body }` (or desugars to labeled loop)
  - [ ] `block` recognized only before `:`; `block` as identifier still valid
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr.rs` — labeled block parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/labeled_blocks.ori`

- [ ] **Implement**: `break:name value` exits labeled block — spec/16-control-flow.md § 16.3
  - [ ] All break paths and final expression must have compatible types
  - [ ] Block type = unified type of all exit paths
  - [ ] **Rust Tests**: `ori_types/src/infer/expr/mod.rs` — labeled block type inference
  - [ ] **Ori Tests**: `tests/spec/expressions/labeled_blocks.ori`
  - [ ] **LLVM Support**: LLVM codegen for labeled block break
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — labeled block codegen

- [ ] **Implement**: Unlabeled `break` is loop-only — spec/16-control-flow.md § 16.2.1
  - [ ] Bare `break` inside a labeled block targets the innermost enclosing loop, not the block
  - [ ] **Rust Tests**: `ori_types/src/check/labels.rs` — break target validation
  - [ ] **Ori Tests**: `tests/spec/expressions/labeled_blocks.ori`

- [ ] **Implement**: `continue:block_label` is a compile-time error
  - [ ] Blocks do not iterate — `continue` targeting a block is invalid
  - [ ] **Rust Tests**: `ori_types/src/check/labels.rs` — continue-block error
  - [ ] **Ori Tests**: `tests/compile-fail/labeled_block_continue.ori`

- [ ] **Implement**: Block labels share namespace with loop labels (no shadowing)
  - [ ] A block label cannot shadow a loop label and vice versa
  - [ ] Uses existing E0871 error
  - [ ] **Rust Tests**: `ori_types/src/check/labels.rs` — cross-construct shadowing
  - [ ] **Ori Tests**: `tests/compile-fail/labeled_block_shadow.ori`

- [ ] **Implement**: Nesting — labeled blocks can contain labeled blocks and labeled loops
  - [ ] `break:outer` from inner block/loop works correctly
  - [ ] `continue:loop_label` passes through labeled blocks (transparent)
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/mod.rs` — nested labeled block evaluation
  - [ ] **Ori Tests**: `tests/spec/expressions/labeled_blocks.ori`
  - [ ] **LLVM Support**: LLVM codegen for nested labeled blocks
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — nested labeled block codegen

---

## 10.3A while Expression

**Proposal**: `proposals/approved/while-loop-proposal.md`

Syntactic sugar: `while condition do body` desugars to `loop { if !condition then break; body }`. Always `void` type.

### Sync Points: `ExprKind::While` (multi-crate sync required)

If `while` is represented as its own `ExprKind::While` variant (rather than desugared in the parser):
1. **`ori_ir`** — Add `ExprKind::While { condition, body, label }` variant
2. **`ori_types`** — Type checking: condition must be `bool`, result type is `void`, handle E0860/E0861
3. **`ori_eval`** — Evaluation: desugar to loop+break or direct while execution
4. **`ori_llvm`** — Codegen: `br` instruction with condition check, loop body block, exit block

If desugared in the parser to `loop { if !condition then break; body }`, only steps 1-2 (parser + IR) are needed.

### Implementation

- [ ] **Implement**: Add `while` as reserved keyword — spec/07-lexical-elements.md § 7.3.1
  - [ ] **Lexer**: Add `while` to keyword lookup in `ori_lexer/src/keywords/mod.rs` (5-char bucket)
  - [ ] **Token**: Add `KwWhile` variant to `ori_ir/src/token/tag.rs`
  - [ ] **Rust Tests**: Lexer keyword recognition test
  - [ ] **Ori Tests**: `tests/spec/lexical/keywords.ori` — verify `while` is reserved

- [ ] **Implement**: Parse `while [label] expression do expression` — spec/14-expressions.md § 14.12
  - [ ] Parser produces `ExprKind::While { condition, body, label }`
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr.rs` — while parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/while_loop.ori`

- [ ] **Implement**: Desugar to `loop { if !condition then break; body }` — spec/16-control-flow.md § 16.2.4
  - [ ] Desugaring in parser or early lowering
  - [ ] **Rust Tests**: Verify desugared form
  - [ ] **Ori Tests**: `tests/spec/expressions/while_loop.ori`

- [ ] **Implement**: Type checking — `while...do` has type `void`
  - [ ] `break value` in while is error E0860
  - [ ] `continue value` in while is error E0861
  - [ ] **Rust Tests**: `ori_types` — while type inference
  - [ ] **Ori Tests**: `tests/compile-fail/while_break_value.ori`, `tests/compile-fail/while_continue_value.ori`

- [ ] **Implement**: Labels on while — `while:name condition do body`
  - [ ] `break:name` and `continue:name` work correctly
  - [ ] **Rust Tests**: Parser — labeled while parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/while_loop.ori`

- [ ] **Implement**: LLVM codegen for while expression
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/aot/while_loops.rs`

---

## 10.4 Error Propagation (?)

- [x] **Implement**: Parse postfix `?` operator — spec/16-control-flow.md § Error Propagation [done] (verified 2026-03-28)
  - [ ] **Rust Tests**: `ori_parse/src/grammar/postfix.rs` — ? operator parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/postfix.ori`
  - [ ] **LLVM Support**: LLVM codegen for ? operator
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/error_propagation_tests.rs` — ? operator codegen
  - [x] **AOT Tests**: `ori_llvm/tests/aot/error_handling.rs` — `test_err_try_result_ok`, `test_err_try_result_err`, `test_err_try_option_some`, `test_err_try_option_none` (postfix `?` operator on both Result and Option)

- [x] **Implement**: On `Result<T, E>`: unwrap `Ok` or return `Err` — spec/16-control-flow.md § On Result [done] (verified 2026-03-28)
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/postfix.rs` — Result propagation
  - [ ] **Ori Tests**: `tests/spec/expressions/postfix.ori`
  - [ ] **LLVM Support**: LLVM codegen for Result propagation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/error_propagation_tests.rs` — Result propagation codegen
  - [x] **AOT Tests**: `ori_llvm/tests/aot/error_handling.rs` — `test_err_try_result_ok`, `test_err_try_result_err`, `test_err_try_result_chain`, `test_err_try_result_early_exit`, `test_err_deep_try_chain` (`?` on Result unwrapping Ok, propagating Err, chaining, and early exit)

- [x] **Implement**: On `Option<T>`: unwrap `Some` or return `None` — spec/16-control-flow.md § On Option [done] (verified 2026-03-28)
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/postfix.rs` — Option propagation
  - [ ] **Ori Tests**: `tests/spec/expressions/postfix.ori`
  - [ ] **LLVM Support**: LLVM codegen for Option propagation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/error_propagation_tests.rs` — Option propagation codegen
  - [x] **AOT Tests**: `ori_llvm/tests/aot/error_handling.rs` — `test_err_try_option_some`, `test_err_try_option_none` (`?` on Option with Some and None paths)

- [ ] **Implement**: Only valid in functions returning `Result`/`Option` — spec/16-control-flow.md § Error Propagation
  - [ ] **Rust Tests**: `ori_types/src/check/propagation.rs` — context validation
  - [ ] **Ori Tests**: `tests/compile-fail/invalid_propagation.ori`
  - [ ] **LLVM Support**: LLVM codegen for propagation context validation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/error_propagation_tests.rs` — context validation codegen
  - [ ] **AOT Tests**: No AOT coverage yet

**Error Return Traces** (proposals/approved/error-return-traces-proposal.md):

- [ ] **Implement**: Automatic trace collection at `?` propagation points
  - [ ] `?` operator records source location (file, line, column, function name)
  - [ ] Trace entries stored internally in Error type
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/postfix.rs` — trace collection
  - [ ] **Ori Tests**: `tests/spec/errors/trace_collection.ori`
  - [ ] **LLVM Support**: LLVM codegen for trace collection
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/error_propagation_tests.rs` — trace collection codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `TraceEntry` type — proposals/approved/error-return-traces-proposal.md § Error Type Enhancement
  - [ ] Fields: `function: str`, `file: str`, `line: int`, `column: int`
  - [ ] **Rust Tests**: `ori_ir/src/types/error.rs` — TraceEntry type
  - [ ] **Ori Tests**: `tests/spec/errors/trace_entry.ori`
  - [ ] **LLVM Support**: LLVM codegen for TraceEntry type
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/error_propagation_tests.rs` — TraceEntry type codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Error trace methods — proposals/approved/error-return-traces-proposal.md § Accessing Traces
  - [ ] `Error.trace() -> str` — formatted trace string
  - [ ] `Error.trace_entries() -> [TraceEntry]` — programmatic access
  - [ ] `Error.has_trace() -> bool` — check if trace available
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Error trace methods
  - [ ] **Ori Tests**: `tests/spec/errors/trace_methods.ori`
  - [ ] **LLVM Support**: LLVM codegen for Error trace methods
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/error_propagation_tests.rs` — Error trace methods codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `Printable` for Error includes trace — proposals/approved/error-return-traces-proposal.md § Printing Errors
  - [ ] `str(error)` includes trace in output
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Error printing
  - [ ] **Ori Tests**: `tests/spec/errors/trace_printing.ori`
  - [ ] **LLVM Support**: LLVM codegen for Error printing with trace
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/error_propagation_tests.rs` — Error printing codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `Result.context()` method — proposals/approved/error-return-traces-proposal.md § Result Methods
  - [ ] `.context(msg: str)` adds context while preserving trace
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Result.context
  - [ ] **Ori Tests**: `tests/spec/errors/context_method.ori`
  - [ ] **LLVM Support**: LLVM codegen for Result.context method
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/error_propagation_tests.rs` — Result.context codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: `Traceable` trait for built-in Error type — implemented in §3.13 (2026-02-17)
  - [x] `@with_trace(self, entry: TraceEntry) -> Self` — push trace entry
  - [x] `@trace(self) -> str` — formatted trace string
  - [x] `@trace_entries(self) -> [TraceEntry]` — list of TraceEntry structs
  - [x] `@has_trace(self) -> bool` — check if trace exists
  - Note: Custom error types implementing Traceable is deferred (requires user-defined trait impls)
  - [ ] **LLVM Support**: LLVM codegen for Traceable trait
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/error_propagation_tests.rs` — Traceable trait codegen
  - [ ] **AOT Tests**: No AOT coverage yet

---

## 10.5 Let Bindings

- [x] **Implement**: Parse `let x = expr` — spec/14-expressions.md § Let Bindings [done] (2026-02-10)
  - [x] **Rust Tests**: Parser and evaluator — let binding
  - [x] **Ori Tests**: `tests/spec/expressions/bindings.ori` — 17 tests (let_inferred, let_string, etc.)
  - [ ] **LLVM Support**: LLVM codegen for let binding
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/binding_tests.rs` — let binding codegen
  - [x] **AOT Tests**: `ori_llvm/tests/aot/scoping.rs` — `test_scope_let_basic`, `test_scope_let_type_annotation`, `test_scope_let_chain`, `test_scope_block_as_value`, `test_scope_block_single_expression`, `test_scope_block_with_side_effects` (basic let bindings, chaining, and block-expression values)

- [x] **Implement**: Parse `let mut x = expr` — spec/14-expressions.md § Mutable Bindings [done] (2026-02-10)
  - [x] **Rust Tests**: Parser and evaluator — mutable binding
  - [x] **Ori Tests**: `tests/spec/expressions/mutation.ori` — 15 tests (mutable_basic, mutable_loop, etc.)
  - [ ] **LLVM Support**: LLVM codegen for mutable binding
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/binding_tests.rs` — mutable binding codegen
  - [x] **AOT Tests**: `ori_llvm/tests/aot/mutations.rs` — `test_mut_simple_reassign`, `test_mut_reassign_multiple`, `test_mut_reassign_self_reference`, `test_mut_loop_counter`, `test_mut_loop_accumulator`, `test_mut_if_reassign`, `test_mut_if_else_reassign`, `test_mut_reassign_string`, `test_mut_reassign_bool`, `test_mut_swap_values`, `test_mut_fibonacci` (21 tests covering mutable bindings in all contexts)

- [x] **Implement**: Parse `let x: Type = expr` — spec/14-expressions.md § Let Bindings [done] (2026-02-10)
  - [x] **Rust Tests**: Parser and type checker — typed binding
  - [x] **Ori Tests**: `tests/spec/expressions/bindings.ori` — let_annotated_int, let_annotated_str, etc.
  - [ ] **LLVM Support**: LLVM codegen for typed binding
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/binding_tests.rs` — typed binding codegen
  - [x] **AOT Tests**: `ori_llvm/tests/aot/scoping.rs` — `test_scope_let_type_annotation` (int, float, bool type-annotated let bindings)

- [x] **Implement**: Parse struct destructuring `let { x, y } = val` — spec/14-expressions.md § Destructuring [done] (2026-02-10)
  - [x] **Rust Tests**: Parser — struct destructure parsing
  - [x] **Ori Tests**: `tests/spec/expressions/bindings.ori` — struct_destructure_shorthand, struct_destructure_rename
  - [ ] **LLVM Support**: LLVM codegen for struct destructuring
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/binding_tests.rs` — struct destructuring codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: Parse tuple destructuring `let (a, b) = val` — spec/14-expressions.md § Destructuring [done] (2026-02-10)
  - [x] **Rust Tests**: Parser — tuple destructure parsing
  - [x] **Ori Tests**: `tests/spec/expressions/bindings.ori` — tuple_destructure test
  - [ ] **LLVM Support**: LLVM codegen for tuple destructuring
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/binding_tests.rs` — tuple destructuring codegen
  - [x] **AOT Tests**: `ori_llvm/tests/aot/scoping.rs` — `test_scope_tuple_destructure` (tuple destructuring in let binding)

- [x] **Implement**: Parse list destructuring `let [head, ..tail] = val` — spec/14-expressions.md § Destructuring [done] (2026-02-10)
  - [x] **Rust Tests**: Parser — list destructure parsing
  - [x] **Ori Tests**: `tests/spec/expressions/bindings.ori` — list_destructure_basic, list_destructure_head, list_destructure_with_rest
  - [ ] **LLVM Support**: LLVM codegen for list destructuring
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/binding_tests.rs` — list destructuring codegen
  - [ ] **AOT Tests**: No AOT coverage yet

---

## 10.6 Scoping

- [x] **Implement**: Lexical scoping — spec/11-blocks-and-scope.md § Lexical Scoping [done] (2026-02-10)
  - [x] **Rust Tests**: Evaluator — lexical scope tests
  - [x] **Ori Tests**: `tests/spec/expressions/block_scope.ori` — 3 tests (let_bindings_in_run, nested_run_shadowing, run_returns_last_expression)
  - [ ] **LLVM Support**: LLVM codegen for lexical scoping
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/scope_tests.rs` — lexical scoping codegen
  - [x] **AOT Tests**: `ori_llvm/tests/aot/scoping.rs` — `test_scope_let_basic`, `test_scope_let_chain`, `test_scope_block_as_value`, `test_scope_nested_blocks_as_values`, `test_scope_shadow_in_nested_block` [ignored], `test_scope_shadow_three_levels` [ignored], `test_scope_shadow_in_loop` [ignored] (lexical scoping with blocks, nesting, and control flow)

- [x] **Implement**: No hoisting — spec/11-blocks-and-scope.md § No Hoisting [done] (2026-02-10)
  - [x] **Rust Tests**: Evaluator — no hoisting tests
  - [x] **Ori Tests**: `tests/spec/expressions/block_scope.ori` — sequential binding verified
  - [ ] **LLVM Support**: LLVM codegen for no hoisting
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/scope_tests.rs` — no hoisting codegen
  - [x] **AOT Tests**: `ori_llvm/tests/aot/scoping.rs` — `test_scope_let_chain` (sequential let bindings depend on previous values, verifying no hoisting)

- [x] **Implement**: Shadowing — spec/11-blocks-and-scope.md § Shadowing [done] (2026-02-10)
  - [x] **Rust Tests**: Evaluator — shadowing tests
  - [x] **Ori Tests**: `tests/spec/expressions/bindings.ori` — let_shadow, let_shadow_different_type; `mutation.ori` — shadow_mutability
  - [ ] **LLVM Support**: LLVM codegen for shadowing
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/scope_tests.rs` — shadowing codegen
  - [x] **AOT Tests**: `ori_llvm/tests/aot/scoping.rs` — `test_scope_shadow_same_type`, `test_scope_shadow_different_type`, `test_scope_shadow_uses_previous`, `test_scope_shadow_in_nested_block` [ignored], `test_scope_shadow_three_levels` [ignored], `test_scope_many_lets_same_name`, `test_scope_string_shadow` (same-type, cross-type, self-referential, and nested-block shadowing)

- [x] **Implement**: Lambda capture by value — spec/11-blocks-and-scope.md § Lambda Capture [done] (2026-02-10)
  - [x] **Rust Tests**: Evaluator — capture tests
  - [x] **Ori Tests**: `tests/spec/expressions/lambdas.ori` — 29 tests (closure_capture, closure_capture_multiple, closure_nested)
  - [ ] **LLVM Support**: LLVM codegen for lambda capture by value
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/scope_tests.rs` — lambda capture codegen
  - [x] **AOT Tests**: `ori_llvm/tests/aot/scoping.rs` — `test_scope_closure_captures_outer`, `test_scope_shadow_before_closure` (closure capture by value), `ori_llvm/tests/aot/higher_order.rs` — `test_hof_closure_capture_computation`, `test_hof_closure_capture_in_loop`, `test_hof_closure_bool_capture`, `test_hof_two_closures_same_capture`, `test_hof_nested_closure_deep`, `test_hof_make_adder`, `test_hof_make_multiplier`, `test_hof_make_predicate` (closure captures and returns)

---

## 10.7 Panics

- [ ] **Implement**: Implicit panics (index out of bounds, division by zero) — spec/17-errors-and-panics.md § Implicit Panics
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/binary.rs` — implicit panic tests
  - [ ] **Ori Tests**: `tests/spec/expressions/panics.ori`
  - [ ] **LLVM Support**: LLVM codegen for implicit panics
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/panic_tests.rs` — implicit panics codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: `panic(message)` function — spec/17-errors-and-panics.md § Explicit Panic [done] (2026-02-10)
  - [x] **Rust Tests**: Evaluator — panic function
  - [x] **Ori Tests**: `tests/spec/expressions/coalesce.ori` — panic in short-circuit tests; `operators_bitwise.ori` — assert_panics tests
  - [ ] **LLVM Support**: LLVM codegen for panic function
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/panic_tests.rs` — panic function codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: `catch(expr)` pattern — spec/17-errors-and-panics.md § Catching Panics [done] (2026-02-19)
  - [x] **Rust Tests**: `ori_patterns/src/builtins/catch/tests.rs` — catch_success, catch_error, name, required_props; `ori_types/src/infer/expr/tests.rs` — catch type inference
  - [x] **Ori Tests**: `tests/spec/patterns/catch.ori` — 7 tests: success, panic, message, div_zero, ok_value, string, nested
  - [ ] **LLVM Support**: LLVM codegen for catch pattern (simplified placeholder exists)
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/panic_tests.rs` — catch pattern codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `PanicInfo` type — spec/17-errors-and-panics.md § PanicInfo
  - [ ] **Rust Tests**: `ori_ir/src/types/panic.rs` — PanicInfo type tests
  - [ ] **Ori Tests**: `tests/spec/patterns/catch.ori`
  - [ ] **LLVM Support**: LLVM codegen for PanicInfo type
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/panic_tests.rs` — PanicInfo type codegen
  - [ ] **AOT Tests**: No AOT coverage yet

---

## 10.8 Index Expressions — [partial] Evaluator Complete

- [x] **Implement**: `#` length symbol in index brackets (`list[# - 1]`) — spec/14-expressions.md § Index Access [done] (2026-02-10)
  - [x] **Parser**: Parse `#` as `ExprKind::HashLength` inside `[...]` — `ori_parse/src/grammar/expr/postfix.rs`
  - [x] **Type Checker**: Resolve `HashLength` to receiver's length type (`int`) — `ori_types/src/infer/`
  - [x] **Evaluator**: Evaluate `HashLength` as `len(receiver)` in index context — `ori_eval/src/interpreter/mod.rs`
  - [x] **Ori Tests**: `tests/spec/expressions/index_access.ori` — hash_last, hash_second_last, hash_first, hash_middle, hash_arithmetic (35 total tests)
  - [ ] **LLVM Support**: LLVM codegen for hash length in index (placeholder exists, needs real impl)
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/collection_tests.rs` — hash length codegen
  - [ ] **AOT Tests**: No AOT coverage yet

**Implementation Notes:**
- Added `IN_INDEX` context flag to `ParseContext`
- Parser recognizes `#` (TokenKind::Hash) as `ExprKind::HashLength` only inside index brackets
- Type checker and evaluator already had full support for `HashLength`
- 901 tests pass (up from 900)
- LLVM has placeholder (returns 0), needs proper implementation later

---

## 10.9 Section Completion Checklist

- [ ] All items above have all three checkboxes marked `[ ]`
- [ ] Spec updated: `spec/14-expressions.md`, `spec/16-control-flow.md` reflect implementation
- [ ] CLAUDE.md updated if syntax/behavior changed
- [ ] 80+% test coverage
- [ ] Run full test suite: `./test-all.sh`
- [ ] `/tpr-review` passed — independent Codex review found no critical or major issues (or all findings triaged)

**Exit Criteria**: All control flow constructs work including labeled loops, scoping, and panic handling
