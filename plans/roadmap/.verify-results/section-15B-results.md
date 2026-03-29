# Section 15B Verification Results: Function Syntax

**Verified**: 2026-03-28
**Section status**: not-started (per frontmatter)
**Actual status**: partially-implemented -- several features have deep implementations

## Files Loaded

- `/home/eric/projects/ori_lang/CLAUDE.md` (full)
- All 20 files in `.claude/rules/` (full)
- `/home/eric/projects/ori_lang/plans/roadmap/section-15B-function-syntax.md` (full)
- Spec: `docs/ori_lang/v2026/spec/grammar.ebnf` (call_arg, named_arg, clause_params sections)
- Spec: `docs/ori_lang/v2026/spec/14-expressions.md`
- Spec: `docs/ori_lang/v2026/spec/10-declarations.md`

## Evidence Summary

### Key Findings

1. **Named arguments (no dot prefix) are ALREADY IMPLEMENTED.** The parser accepts `name: value` syntax (not `.name: value`). All tests use the new syntax. Named arg reordering works in the evaluator.

2. **Default parameter values are IMPLEMENTED in parser + type checker + evaluator.** The `Param` struct has a `default: Option<ExprId>` field. The type checker tracks `required_params`. The evaluator fills defaults via `bind_parameters_with_defaults`. **BUT the LLVM backend does NOT handle default param calls** -- tested and confirmed LLVM verification failure.

3. **Function clauses (multiple definitions) are IMPLEMENTED in parser but NOT in execution.** The parser recognizes literal patterns (`0: int`), list patterns, and guard clauses (`if n < 0`). However, multiple function declarations with the same name fail at parse time (`expected ';' after item declaration`). The grouping of same-name declarations into a single function is not implemented.

4. **Lambda positional exception is ALREADY IMPLEMENTED.** `items.map(x -> x * 2)` works. Single-param function calls with inline lambdas accept positional args. Tested and confirmed passing.

5. **Argument punning is ALREADY IMPLEMENTED.** `f(x:)` desugars to `f(x: x)` in the parser. Tests exist in `tests/spec/declarations/argument_punning.ori` and pass.

---

## 15B.1 Remove Dot Prefix from Named Arguments

### - [ ] Parser accepts `IDENTIFIER ':'` instead of `'.' IDENTIFIER ':'`

**Verdict**: [done] -- ALREADY IMPLEMENTED

The parser (`ori_parse/src/grammar/expr/postfix.rs:380-431`) uses `is_named_arg_start()` to detect named args as `IDENTIFIER ':'`. No dot prefix exists in the current implementation. All test files use `name: value` syntax.

Evidence: `tests/spec/declarations/named_arguments.ori` (25 tests, all pass).

### - [ ] Enforce named arguments for built-in functions

**Verdict**: [partial] -- PARTIALLY IMPLEMENTED

`function_exp` builtins (`print`, `panic`, `recurse`, `parallel`, etc.) require named args via the parser. But `function_val` builtins (`len`, `is_empty`, `assert`, `assert_eq`, etc.) accept positional args.

Evidence:
- `print("hello")` -> `error[E1013]: print requires named properties`
- `len([1,2,3])` -> runs successfully with positional
- `assert_eq(1, 1)` -> runs successfully with positional

**Sub-items**:
- [ ] Rust Tests -- [todo] No tests enforce named args for all builtins.
- [ ] Ori Tests: `tests/spec/expressions/builtin_named_args.ori` -- [todo] Does not exist.
- [ ] Ori Tests: `tests/compile-fail/builtin_positional_args.ori` -- [todo] Does not exist.

### - [ ] Allow positional only for function variable calls

**Verdict**: [done] -- ALREADY IMPLEMENTED

Function variables accept positional args. Tested:
```
let f = (x: int) -> int = x * 2; f(5)  // works
```

**Sub-items**:
- [ ] Rust Tests -- [todo] No dedicated tests.
- [ ] Ori Tests: `tests/spec/expressions/function_var_positional.ori` -- [todo] Does not exist.

### - [ ] Clear error message when positional used incorrectly

**Verdict**: [partial] -- PARTIALLY IMPLEMENTED

`E1013` exists for `function_exp` builtins. No equivalent for direct function calls where named args should be enforced.

**Sub-items**:
- [ ] Rust Tests -- [todo]
- [ ] Ori Tests: `tests/compile-fail/positional_arg_error.ori` -- [todo] Does not exist.

### Built-in Function Updates (print, len, is_empty, assert, assert_eq, etc.)

**Verdict**: [partial] -- PARTIALLY IMPLEMENTED

- `print` requires `msg:` -- [done]
- `panic` requires `msg:` -- [done] (it's a `function_exp`)
- `len` does NOT require `collection:` -- [todo]
- `is_empty` does NOT require `collection:` -- [todo]
- `assert` does NOT require `condition:` -- [todo]
- `assert_eq` does NOT require `actual:`, `expected:` -- [todo]
- `assert_ne` does NOT require `actual:`, `unexpected:` -- [todo]
- `assert_some/none` -- [todo]
- `assert_ok/err` -- [todo]
- `assert_panics` -- [todo]
- `assert_panics_with` -- [todo]
- `compare`, `min`, `max` -- [todo]
- `repeat` -- [todo]

The enforcement of named args for these builtins would require changing them from `FunctionVal` (which allows positional) to something that enforces named args.

### Formatter

**Verdict**: [partial] -- Width-based stacking exists in `ori_fmt/src/packing/` and `width/`. No specific test for named-argument stacking rules.

### Migration Tool

**Verdict**: [todo] -- NOT IMPLEMENTED

No `ori migrate remove-dot-prefix` command exists. No migration tooling.

### Documentation & Tests

- [ ] Update all existing tests to use named arguments for built-ins -- [partial] Many tests use positional for `len`, `assert_eq`, etc.
- [ ] Update spec examples -- [todo]
- [ ] Update CLAUDE.md examples -- [partial] CLAUDE.md shows named arg syntax.

---

## 15B.2 Default Parameter Values

### Parser

**All parser items**: [done] -- ALREADY IMPLEMENTED

- `Param` struct (`ori_ir/src/ast/items/function.rs:84-98`) has `default: Option<ExprId>` field.
- Parser (`ori_parse/src/grammar/item/function/mod.rs:502-508`) parses `= expression` after type.
- Grammar (`grammar.ebnf:250`): `clause_param = match_pattern [ ":" type ] [ "=" expression ]`

Evidence: `tests/spec/declarations/named_arguments.ori` tests default params extensively (all pass).

**Sub-items**:
- [ ] Rust Tests -- [partial] No dedicated Rust-level default param parsing tests found.
- [ ] Ori Tests: `tests/spec/declarations/default_params.ori` -- [todo] Not created, but `named_arguments.ori` covers defaults.
- [ ] LLVM Support -- [todo] LLVM verification fails on calls with omitted defaults.
- [ ] LLVM Rust Tests -- [todo]
- [ ] AOT Tests -- [todo]

### Type Checker

**All type checker items**: [done] in interpreter -- IMPLEMENTED

- `required_params` computed in `ori_types/src/check/signatures/mod.rs:245` and `check/bodies/mod.rs:404`.
- Call validation uses `required_params` in `ori_types/src/infer/expr/calls/call_inference.rs:41-54`.
- `Signature` struct has `required_params: usize` field (`ori_types/src/output/mod.rs:412`).

Evidence: `named_arguments.ori` tests verify: all-defaults omitted, partial defaults, expr defaults.

**Sub-items**:
- [ ] Verify default expression has parameter's type -- [done] Type checker handles this.
- [ ] Verify default doesn't reference other parameters -- [todo] No evidence of this check.
- [ ] Track which parameters have defaults for call validation -- [done] `required_params` tracked.
- [ ] Capability checking for default expressions -- [todo] No evidence of this check.

### Call Site Validation

- [ ] Required parameters must be provided -- [done] `call_inference.rs:46` checks `arg_ids.len() < required_params`.
- [ ] Allow omitting parameters with defaults -- [done] `arg_ids.len()` between `required_params` and `params.len()` accepted.
- [ ] Clear error message when required param missing -- [partial] Arity mismatch error exists, but may not be specifically helpful for default params.

### Code Generation (LLVM)

**Verdict**: [todo] -- NOT IMPLEMENTED

Confirmed by testing: building a program with default parameter calls produces LLVM verification error:
```
error[E5001]: LLVM module verification failed
  = note: LLVM says: "Incorrect number of arguments passed to called function!"
```

The ARC lowering / LLVM emitter does not fill in default expressions for omitted arguments.

**Sub-items**:
- [ ] Insert default expressions for omitted arguments -- [todo]
- [ ] Evaluate defaults at call time (not definition time) -- [done] Evaluator does this in `bind_parameters_with_defaults`.
- [ ] Correct evaluation order -- [todo] LLVM side not implemented.

### Trait Method Defaults

- [ ] Allow defaults in trait method signatures -- [todo] Not tested.
- [ ] Allow implementations to override/remove defaults -- [todo]
- [ ] Trait object calls use trait's declared default -- [todo]

---

## 15B.3 Multiple Function Clauses

### Parser

**Verdict**: [partial] -- Parser recognizes clause syntax but DOES NOT group same-name declarations.

The parser (`ori_parse/src/grammar/item/function/mod.rs:372-528`) supports:
- Literal patterns in params: `(0: int)` -- [done] (lines 406-417)
- List patterns in params: `([]: [T])` -- [done] (lines 420-428)
- Guard clauses: `if condition` -- [done] Grammar includes `guard_clause`
- Default values in clause params: `(x: int = 42)` -- [done] (lines 502-508)

However, multiple declarations with the same name (`@factorial (0: int) = 1` then `@factorial (n) = ...`) fail at parse time with `expected ';' after item declaration`. The parser does not group same-name function declarations into a multi-clause function.

Evidence: Tested `@factorial (0: int) -> int = 1\n@factorial (n: int) -> int = n * factorial(n: n - 1)` -- produces parse error.

**Sub-items**:
- [ ] Allow `match_pattern` in parameter position -- [done] Parser supports it.
- [ ] Parse `if` guard clause -- [done] Grammar includes guard_clause.
- [ ] Group multiple declarations with same name into single function -- [todo] NOT IMPLEMENTED. This is the critical missing piece.

### Semantic Analysis

All items: [todo] -- NOT IMPLEMENTED

- [ ] Validate all clauses have same parameter count -- [todo]
- [ ] Validate all clauses have same return type -- [todo]
- [ ] Validate all clauses have same capabilities -- [todo]
- [ ] First clause rules (visibility, generics, types) -- [todo]
- [ ] Type inference for subsequent clause parameters -- [todo]
- [ ] Error if visibility/generics repeated on subsequent clauses -- [todo]

### Exhaustiveness & Reachability

All items: [todo] -- NOT IMPLEMENTED

- [ ] Exhaustiveness checking across all clauses -- [todo]
- [ ] Unreachable clause detection and warnings -- [todo]

### Code Generation

All items: [todo] -- NOT IMPLEMENTED

- [ ] Desugar clauses to single function with `match` -- [todo]
- [ ] Function clause `if` guards -- [todo]

### Integration

All items: [todo] -- NOT IMPLEMENTED

- [ ] Named argument reordering before pattern matching -- [todo]
- [ ] Default parameter filling before pattern matching -- [todo]
- [ ] Tests target function name (cover all clauses) -- [todo]

### Note on test file

`tests/spec/declarations/clause_params.ori` exists but is ENTIRELY COMMENTED OUT. Every test is behind `//` comment markers. The file contains extensive test coverage (350+ lines) for default params, clauses, guards, list patterns, generics, and more -- but none of it is active. Status markers in comments indicate:
- Basic params: needs implementation
- Default params: Parser OK, some work in type checker
- Pattern matching: Parser OK, TypeChecker NEEDS IMPL
- Guard clauses: Parser OK, Evaluator NEEDS IMPL
- List patterns: Parser OK, TypeChecker NEEDS IMPL

---

## 15B.4 Positional Lambdas for Single-Parameter Functions

### Type Checker

**Verdict**: [done] -- ALREADY IMPLEMENTED

The type checker accepts positional lambda arguments for single-parameter method calls.

Evidence:
```ori
let doubled = items.map(x -> x * 2);  // works
items.filter(x -> x > 0);             // works
```

Tested and confirmed passing.

The parser detects lambda expressions in positional position and the type checker allows them. This is the default behavior of the call resolution since lambdas in method calls have always been allowed positionally (the `items.map(x -> expr)` syntax is idiomatic Ori).

**Sub-items**:
- [ ] Check for lambda-literal positional argument exception -- [done] Works implicitly.
- [ ] Verify callee has exactly 1 explicit parameter -- [partial] Works for methods. Not clear if arbitrary multi-param functions correctly reject positional lambdas.
- [ ] Verify argument expression is a `LambdaExpr` AST node -- [partial] Not explicitly checked -- any positional arg works.
- [ ] Reject positional for function references/variables -- [todo] Not tested. `list.map(double)` may work even though per spec it should require named arg.

### Error Messages

- [ ] Clear error when using positional non-lambda for single-param function -- [todo] No specific error exists.

### Edge Cases

- [ ] Nested lambdas work correctly -- [partial] Likely works but no dedicated test.
- [ ] Chained method calls with lambdas -- [partial] Likely works (common pattern) but no dedicated test.
- [ ] Lambda returning lambda -- [todo] No test.

### Documentation

- [ ] Update spec `09-expressions.md` -- [todo] (Note: this is `14-expressions.md` in the actual spec)
- [ ] Update `CLAUDE.md` -- [partial] CLAUDE.md already documents lambda positional syntax.

---

## 15B.5 Argument Punning (Call Arguments)

### Parser

**Verdict**: [done] -- ALREADY IMPLEMENTED

Argument punning (`f(x:)` -> `f(x: x)`) is implemented in `ori_parse/src/grammar/expr/postfix.rs:402-411`.

Evidence:
- Parser code at line 402-408: When `name:` is followed by `,` or `)`, creates synthetic `Expr::Ident` with the argument name.
- `tests/spec/declarations/argument_punning.ori` -- 6 tests covering single-param, multi-param, mixed, string, method, and partial punning. ALL PASS (4181 passed, 0 failed, 42 skipped).

**Sub-items**:
- [ ] In call argument parsing, create synthetic `Expr::Ident` -- [done] Lines 402-408.
- [ ] Mixed punned and explicit arguments -- [done] Tested in `test_mixed_punning` and `test_partial_punning`.
- [ ] `f(x)` positional unchanged (no regression) -- [done] Positional still works.

### Error Messages

- [ ] `f(x:)` when `x` not in scope produces "cannot find value" -- [partial] Would produce standard scope error. No dedicated test.
- [ ] `f(x:)` when function has no param `x` produces "unknown parameter" -- [partial] Would produce standard error. No dedicated test.

### Formatter

- [ ] Detect `name == value_ident` in call args and emit `name:` form -- [todo] NOT IMPLEMENTED. Formatter does not canonicalize `f(x: x)` to `f(x:)`.
- [ ] Preserve `f(x: other)` when names differ -- [todo] (formatter logic not implemented)

### Documentation

- [ ] Update spec `09-expressions.md` -- [partial] `grammar.ebnf:446` documents: `named_arg = identifier ":" [ expression ]` with punning comment.
- [ ] Update `grammar.ebnf` -- [done] Line 446 already shows optional expression.
- [ ] Update `.claude/rules/ori-syntax.md` -- [done] Already documents `f(x:)` syntax.

---

## 15B.6 Section Completion Checklist

- [ ] All implementation items have checkboxes marked `[ ]` -- [done]
- [ ] All spec docs updated -- [partial]
- [ ] CLAUDE.md updated with syntax changes -- [partial]
- [ ] Migration tools working -- [todo]
- [ ] All tests pass: `./test-all.sh` -- Not run for this verification.
- [ ] `/tpr-review` passed -- [todo]

---

## Summary

| Subsection | Plan Status | Actual Status | Items Done | Items Partial | Items Todo |
|---|---|---|---|---|---|
| 15B.1 Remove Dot Prefix | not-started | partially-done | 3 | 5 | 15 |
| 15B.2 Default Parameters | not-started | partially-done | 7 | 3 | 15 |
| 15B.3 Function Clauses | not-started | barely-started | 3 | 0 | 20+ |
| 15B.4 Lambda Positional | not-started | mostly-done | 3 | 4 | 5 |
| 15B.5 Argument Punning | not-started | mostly-done | 5 | 2 | 3 |
| 15B.6 Completion Checklist | not-started | not-started | 1 | 2 | 3 |

**Overall**: The section is marked `not-started` but has significant hidden implementations. The status should be updated to `partially-implemented`.

**What is DONE and working**:
1. Named argument syntax (no dot prefix) -- parser and evaluator
2. Default parameter values -- parser, type checker, and evaluator (NOT LLVM)
3. Lambda positional exception -- works for methods
4. Argument punning -- parser and evaluator, with tests

**Critical gaps**:
1. **LLVM backend for default params** -- calls with omitted defaults produce LLVM verification errors. This is the highest-priority gap.
2. **Function clause grouping** -- the parser recognizes clause syntax but does not group same-name declarations. The evaluator cannot execute multi-clause functions. This is the largest unimplemented feature.
3. **Named arg enforcement for all builtins** -- only `function_exp` builtins enforce named args. `function_val` builtins (`len`, `assert_eq`, etc.) still accept positional.
4. **Formatter punning canonicalization** -- `f(x: x)` is not auto-shortened to `f(x:)`.

**PLAN QUALITY NOTES**:
1. Like 15A, many items have LLVM sub-items for features that work at the type checker level (call validation, param tracking). While LLVM support IS needed for default params, many of the LLVM sub-items are duplicative -- LLVM needs to handle the general case of default parameters, not separate LLVM items per type-checker validation step.
2. The plan lists 78 items but many are sub-sub-items (LLVM Rust Tests, AOT Tests) that inflate the count. A more accurate count of real implementation work would be ~30 items.
3. The `clause_params.ori` test file exists with 350+ lines of comprehensive tests, all commented out. This is valuable work waiting to be activated.
4. The plan correctly identifies that the `as` proposal removes `function_val` for type conversions (15A.2 note), but doesn't propagate this impact to 15B.1 where it affects which builtins need named-arg enforcement.
