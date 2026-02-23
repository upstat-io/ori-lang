---
section: "15B"
title: Function Syntax
status: not-started
tier: 5
goal: Implement function-related syntax proposals
sections:
  - id: "15B.1"
    title: Remove Dot Prefix from Named Arguments
    status: not-started
  - id: "15B.2"
    title: Default Parameter Values
    status: not-started
  - id: "15B.3"
    title: Multiple Function Clauses
    status: not-started
  - id: "15B.4"
    title: Positional Lambdas for Single-Parameter Functions
    status: not-started
  - id: "15B.5"
    title: Argument Punning (Call Arguments)
    status: not-started
  - id: "15B.6"
    title: Section Completion Checklist
    status: not-started
---

# Section 15B: Function Syntax

**Goal**: Implement function-related syntax proposals

> **Source**: `docs/ori_lang/proposals/approved/`

---

## 15B.1 Remove Dot Prefix from Named Arguments

**Proposal**: `proposals/approved/remove-dot-prefix-proposal.md`

Change named argument syntax from `.name: value` to `name: value`. All functions require named arguments — no exceptions.

```ori
// Before
fetch_user(.id: 1)
print("Hello")  // positional was allowed for built-ins

// After
fetch_user(id: 1)
print(msg: "Hello")  // named required everywhere
```

### Key Design Decisions

- All functions require named arguments (built-ins, user-defined, methods)
- Only function variable calls allow positional: `let f = x -> x + 1; f(5)`
- Type conversions use `as` syntax (see 15D), not function calls
- No positional shorthand (`foo(x)` meaning `foo(x: x)` is NOT supported — but see 15B.5 for `foo(x:)` punning with trailing colon)

### Implementation

#### Parser (dot removal done, enforcement needed)

- [ ] **Done**: Parser accepts `IDENTIFIER ':'` instead of `'.' IDENTIFIER ':'`
  - Basic syntax change already implemented

- [ ] **Implement**: Enforce named arguments for built-in functions
  - [ ] **Rust Tests**: `ori_parse/src/grammar/call.rs` — builtin named arg enforcement
  - [ ] **Ori Tests**: `tests/spec/expressions/builtin_named_args.ori`
  - [ ] **Ori Tests**: `tests/compile-fail/builtin_positional_args.ori`

- [ ] **Implement**: Allow positional only for function variable calls
  - [ ] **Rust Tests**: `ori_parse/src/grammar/call.rs` — function var positional
  - [ ] **Ori Tests**: `tests/spec/expressions/function_var_positional.ori`

- [ ] **Implement**: Clear error message when positional used incorrectly
  - [ ] **Rust Tests**: `ori_diagnostic/src/problem.rs` — positional arg error
  - [ ] **Ori Tests**: `tests/compile-fail/positional_arg_error.ori`

#### Built-in Function Updates

- [ ] **Implement**: Update `print` to require `msg:` parameter
  - [ ] **Ori Tests**: `tests/spec/expressions/print_named.ori`

- [ ] **Implement**: Update `len` to require `collection:` parameter
  - [ ] **Ori Tests**: `tests/spec/expressions/len_named.ori`

- [ ] **Implement**: Update `is_empty` to require `collection:` parameter

- [ ] **Implement**: Update `assert` to require `condition:` parameter
  - [ ] **Ori Tests**: `tests/spec/expressions/assert_named.ori`

- [ ] **Implement**: Update `assert_eq` to require `actual:`, `expected:` parameters

- [ ] **Implement**: Update `assert_ne` to require `actual:`, `unexpected:` parameters

- [ ] **Implement**: Update `assert_some`, `assert_none` to require `option:` parameter

- [ ] **Implement**: Update `assert_ok`, `assert_err` to require `result:` parameter

- [ ] **Implement**: Update `assert_panics` to require `f:` parameter

- [ ] **Implement**: Update `assert_panics_with` to require `f:`, `msg:` parameters

- [ ] **Implement**: Update `panic` to require `msg:` parameter

- [ ] **Implement**: Update `compare`, `min`, `max` to require `left:`, `right:` parameters

- [ ] **Implement**: Update `repeat` to require `value:` parameter

#### Formatter

- [ ] **Implement**: Width-based stacking rule (inline if fits, stack if not)
  - [ ] **Rust Tests**: `ori_formatter/src/` — width-based stacking
  - [ ] **Ori Tests**: formatter integration tests

#### Migration Tool

- [ ] **Implement**: `ori migrate remove-dot-prefix` command
  - Finds `.identifier:` patterns and removes the dot
  - `--dry-run` flag for preview

#### Documentation & Tests

- [ ] **Implement**: Update all existing tests to use named arguments for built-ins
- [ ] **Implement**: Update spec examples to use named arguments everywhere
- [ ] **Implement**: Update CLAUDE.md examples (partially done)

---

## 15B.2 Default Parameter Values

**Proposal**: `proposals/approved/default-parameters-proposal.md`

Allow function parameters to specify default values, enabling callers to omit arguments.

```ori
@greet (name: str = "World") -> str = `Hello, {name}!`

greet()               // "Hello, World!"
greet(name: "Alice")  // "Hello, Alice!"
```

### Parser

- [ ] **Implement**: Extend `param` production to accept `= expression` after type
  - [ ] **Rust Tests**: `ori_parse/src/grammar/decl.rs` — default parameter parsing
  - [ ] **Ori Tests**: `tests/spec/declarations/default_params.ori`
  - [ ] **LLVM Support**: LLVM codegen for default parameter parsing
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/default_params_tests.rs` — default parameter parsing codegen

- [ ] **Implement**: Parse default expressions with correct precedence
  - [ ] **Rust Tests**: `ori_parse/src/grammar/decl.rs` — default expression precedence
  - [ ] **Ori Tests**: `tests/spec/declarations/default_params_precedence.ori`
  - [ ] **LLVM Support**: LLVM codegen for default expression precedence
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/default_params_tests.rs` — default expression precedence codegen

### Type Checker

- [ ] **Implement**: Verify default expression has parameter's type
  - [ ] **Rust Tests**: `oric/src/typeck/checker/params.rs` — default type checking
  - [ ] **Ori Tests**: `tests/compile-fail/default_param_type_mismatch.ori`
  - [ ] **LLVM Support**: LLVM codegen for default type checking
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/default_params_tests.rs` — default type checking codegen

- [ ] **Implement**: Verify default doesn't reference other parameters
  - [ ] **Rust Tests**: `oric/src/typeck/checker/params.rs` — default param reference checking
  - [ ] **Ori Tests**: `tests/compile-fail/default_param_references_other.ori`
  - [ ] **LLVM Support**: LLVM codegen for default param reference checking
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/default_params_tests.rs` — default param reference checking codegen

- [ ] **Implement**: Track which parameters have defaults for call validation
  - [ ] **Rust Tests**: `oric/src/typeck/checker/call.rs` — optional parameter tracking
  - [ ] **Ori Tests**: `tests/spec/expressions/call_with_defaults.ori`
  - [ ] **LLVM Support**: LLVM codegen for optional parameter tracking
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/default_params_tests.rs` — optional parameter tracking codegen

- [ ] **Implement**: Capability checking for default expressions
  - [ ] **Rust Tests**: `oric/src/typeck/checker/params.rs` — default capability checking
  - [ ] **Ori Tests**: `tests/spec/capabilities/default_param_capabilities.ori`
  - [ ] **LLVM Support**: LLVM codegen for default capability checking
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/default_params_tests.rs` — default capability checking codegen

### Call Site Validation

- [ ] **Implement**: Required parameters (no default) must be provided
  - [ ] **Rust Tests**: `oric/src/typeck/checker/call.rs` — required param validation
  - [ ] **Ori Tests**: `tests/compile-fail/missing_required_param.ori`
  - [ ] **LLVM Support**: LLVM codegen for required param validation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/default_params_tests.rs` — required param validation codegen

- [ ] **Implement**: Allow omitting parameters with defaults
  - [ ] **Rust Tests**: `oric/src/typeck/checker/call.rs` — optional param omission
  - [ ] **Ori Tests**: `tests/spec/expressions/omit_default_params.ori`
  - [ ] **LLVM Support**: LLVM codegen for optional param omission
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/default_params_tests.rs` — optional param omission codegen

- [ ] **Implement**: Clear error message when required param missing
  - [ ] **Rust Tests**: `ori_diagnostic/src/problem.rs` — missing param error
  - [ ] **Ori Tests**: `tests/compile-fail/missing_required_param_message.ori`
  - [ ] **LLVM Support**: LLVM codegen for missing param error
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/default_params_tests.rs` — missing param error codegen

### Code Generation

- [ ] **Implement**: Insert default expressions for omitted arguments
  - [ ] **Rust Tests**: `oric/src/codegen/call.rs` — default insertion
  - [ ] **Ori Tests**: `tests/spec/expressions/default_insertion.ori`
  - [ ] **LLVM Support**: LLVM codegen for default insertion
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/default_params_tests.rs` — default insertion codegen

- [ ] **Implement**: Evaluate defaults at call time (not definition time)
  - [ ] **Rust Tests**: `oric/src/codegen/call.rs` — call-time evaluation
  - [ ] **Ori Tests**: `tests/spec/expressions/default_call_time_eval.ori`
  - [ ] **LLVM Support**: LLVM codegen for call-time evaluation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/default_params_tests.rs` — call-time evaluation codegen

- [ ] **Implement**: Correct evaluation order (explicit args first, then defaults in param order)
  - [ ] **Rust Tests**: `oric/src/codegen/call.rs` — evaluation order
  - [ ] **Ori Tests**: `tests/spec/expressions/default_eval_order.ori`
  - [ ] **LLVM Support**: LLVM codegen for evaluation order
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/default_params_tests.rs` — evaluation order codegen

### Trait Method Defaults

- [ ] **Implement**: Allow defaults in trait method signatures
  - [ ] **Rust Tests**: `ori_parse/src/grammar/trait.rs` — trait method default parsing
  - [ ] **Ori Tests**: `tests/spec/traits/method_defaults.ori`
  - [ ] **LLVM Support**: LLVM codegen for trait method defaults
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/default_params_tests.rs` — trait method defaults codegen

- [ ] **Implement**: Allow implementations to override/remove defaults
  - [ ] **Rust Tests**: `oric/src/typeck/checker/impl.rs` — impl default override
  - [ ] **Ori Tests**: `tests/spec/traits/impl_override_defaults.ori`
  - [ ] **LLVM Support**: LLVM codegen for impl default override
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/default_params_tests.rs` — impl default override codegen

- [ ] **Implement**: Trait object calls use trait's declared default
  - [ ] **Rust Tests**: `oric/src/codegen/dyn_dispatch.rs` — dyn default dispatch
  - [ ] **Ori Tests**: `tests/spec/traits/dyn_trait_defaults.ori`
  - [ ] **LLVM Support**: LLVM codegen for dyn default dispatch
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/default_params_tests.rs` — dyn default dispatch codegen

---

## 15B.3 Multiple Function Clauses

**Proposal**: `proposals/approved/function-clauses-proposal.md`

Allow functions to be defined with multiple clauses that pattern match on arguments.

```ori
@factorial (0: int) -> int = 1
@factorial (n) -> int = n * factorial(n - 1)

@abs (n: int) -> int if n < 0 = -n
@abs (n) -> int = n
```

### Parser

- [ ] **Implement**: Allow `match_pattern` in parameter position (`clause_param`)
  - [ ] **Rust Tests**: `ori_parse/src/grammar/decl.rs` — clause parameter parsing
  - [ ] **Ori Tests**: `tests/spec/declarations/function_clauses.ori`
  - [ ] **LLVM Support**: LLVM codegen for clause parameter parsing
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/function_clauses_tests.rs` — clause parameter parsing codegen

- [ ] **Implement**: Parse `if` guard clause between `where_clause` and `=`
  - [ ] **Rust Tests**: `ori_parse/src/grammar/decl.rs` — guard clause parsing
  - [ ] **Ori Tests**: `tests/spec/declarations/function_clause_guards.ori`
  - [ ] **LLVM Support**: LLVM codegen for guard clause parsing
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/function_clauses_tests.rs` — guard clause parsing codegen

- [ ] **Implement**: Group multiple declarations with same name into single function
  - [ ] **Rust Tests**: `ori_parse/src/grammar/decl.rs` — clause grouping
  - [ ] **Ori Tests**: `tests/spec/declarations/function_clause_grouping.ori`
  - [ ] **LLVM Support**: LLVM codegen for clause grouping
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/function_clauses_tests.rs` — clause grouping codegen

### Semantic Analysis

- [ ] **Implement**: Validate all clauses have same parameter count
  - [ ] **Rust Tests**: `oric/src/typeck/checker/clauses.rs` — parameter count validation
  - [ ] **Ori Tests**: `tests/compile-fail/clause_param_count_mismatch.ori`
  - [ ] **LLVM Support**: LLVM codegen for parameter count validation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/function_clauses_tests.rs` — parameter count validation codegen

- [ ] **Implement**: Validate all clauses have same return type
  - [ ] **Rust Tests**: `oric/src/typeck/checker/clauses.rs` — return type validation
  - [ ] **Ori Tests**: `tests/compile-fail/clause_return_type_mismatch.ori`
  - [ ] **LLVM Support**: LLVM codegen for return type validation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/function_clauses_tests.rs` — return type validation codegen

- [ ] **Implement**: Validate all clauses have same capabilities (`uses`)
  - [ ] **Rust Tests**: `oric/src/typeck/checker/clauses.rs` — capability validation
  - [ ] **Ori Tests**: `tests/compile-fail/clause_capability_mismatch.ori`
  - [ ] **LLVM Support**: LLVM codegen for capability validation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/function_clauses_tests.rs` — capability validation codegen

- [ ] **Implement**: First clause rules (visibility, generics, types)
  - [ ] **Rust Tests**: `oric/src/typeck/checker/clauses.rs` — first clause signature
  - [ ] **Ori Tests**: `tests/spec/declarations/first_clause_rules.ori`
  - [ ] **LLVM Support**: LLVM codegen for first clause rules
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/function_clauses_tests.rs` — first clause rules codegen

- [ ] **Implement**: Type inference for subsequent clause parameters
  - [ ] **Rust Tests**: `oric/src/typeck/checker/clauses.rs` — clause type inference
  - [ ] **Ori Tests**: `tests/spec/declarations/clause_type_inference.ori`
  - [ ] **LLVM Support**: LLVM codegen for clause type inference
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/function_clauses_tests.rs` — clause type inference codegen

- [ ] **Implement**: Error if visibility/generics repeated on subsequent clauses
  - [ ] **Rust Tests**: `oric/src/typeck/checker/clauses.rs` — duplicate modifier errors
  - [ ] **Ori Tests**: `tests/compile-fail/clause_duplicate_modifiers.ori`
  - [ ] **LLVM Support**: LLVM codegen for duplicate modifier errors
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/function_clauses_tests.rs` — duplicate modifier errors codegen

### Exhaustiveness & Reachability

- [ ] **Implement**: Exhaustiveness checking across all clauses
  - [ ] **Rust Tests**: `oric/src/typeck/exhaustiveness.rs` — clause exhaustiveness
  - [ ] **Ori Tests**: `tests/compile-fail/clause_non_exhaustive.ori`
  - [ ] **LLVM Support**: LLVM codegen for clause exhaustiveness
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/function_clauses_tests.rs` — clause exhaustiveness codegen

- [ ] **Implement**: Unreachable clause detection and warnings
  - [ ] **Rust Tests**: `oric/src/typeck/exhaustiveness.rs` — unreachable clause warning
  - [ ] **Ori Tests**: `tests/warnings/unreachable_clause.ori`
  - [ ] **LLVM Support**: LLVM codegen for unreachable clause warning
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/function_clauses_tests.rs` — unreachable clause warning codegen

### Code Generation

- [ ] **Implement**: Desugar clauses to single function with `match`
  - [ ] **Rust Tests**: `oric/src/codegen/clauses.rs` — clause desugaring
  - [ ] **Ori Tests**: `tests/spec/declarations/clause_desugaring.ori`
  - [ ] **LLVM Support**: LLVM codegen for clause desugaring
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/function_clauses_tests.rs` — clause desugaring codegen

- [ ] **Implement**: Function clause `if` guards (compile to match arm guards)
  - [ ] **Rust Tests**: `oric/src/codegen/clauses.rs` — guard desugaring
  - [ ] **Ori Tests**: `tests/spec/declarations/guard_desugaring.ori`
  - [ ] **LLVM Support**: LLVM codegen for guard desugaring
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/function_clauses_tests.rs` — guard desugaring codegen

### Integration

- [ ] **Implement**: Named argument reordering before pattern matching
  - [ ] **Rust Tests**: `oric/src/codegen/call.rs` — argument reordering
  - [ ] **Ori Tests**: `tests/spec/expressions/clause_named_args.ori`
  - [ ] **LLVM Support**: LLVM codegen for argument reordering
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/function_clauses_tests.rs` — argument reordering codegen

- [ ] **Implement**: Default parameter filling before pattern matching
  - [ ] **Rust Tests**: `oric/src/codegen/call.rs` — default filling with clauses
  - [ ] **Ori Tests**: `tests/spec/expressions/clause_default_params.ori`
  - [ ] **LLVM Support**: LLVM codegen for default filling with clauses
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/function_clauses_tests.rs` — default filling codegen

- [ ] **Implement**: Tests target function name (cover all clauses)
  - [ ] **Rust Tests**: `oric/src/typeck/checker/test.rs` — clause test targeting
  - [ ] **Ori Tests**: `tests/spec/testing/clause_tests.ori`
  - [ ] **LLVM Support**: LLVM codegen for clause test targeting
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/function_clauses_tests.rs` — clause test targeting codegen

---

## 15B.4 Positional Lambdas for Single-Parameter Functions

**Proposal**: `proposals/approved/single-lambda-positional-proposal.md`

Allow omitting parameter names when calling single-parameter functions with inline lambda expressions.

```ori
// Before (required)
items.map(transform: x -> x * 2)
items.filter(predicate: x -> x > 0)

// After (allowed)
items.map(x -> x * 2)
items.filter(x -> x > 0)
```

### The Rule

When ALL of the following are true:
1. Function has exactly one explicit parameter (excluding `self` for methods)
2. The argument expression is a lambda literal

THEN: The parameter name may be omitted.

### What Counts as a Lambda?

Lambda expressions (allowed positional):
- `x -> expr` (single parameter)
- `(a, b) -> expr` (multiple parameters)
- `() -> expr` (no parameters)
- `(x: int) -> int = expr` (typed lambda)

NOT lambda expressions (named arg required):
- Variables holding functions: `let f = x -> x + 1; list.map(f)`
- Function references: `list.map(double)`

### Type Checker

- [ ] **Implement**: Check for lambda-literal positional argument exception in call resolution
  - [ ] **Rust Tests**: `ori_typeck/src/infer/call.rs` — lambda positional arg tests
  - [ ] **Ori Tests**: `tests/spec/expressions/lambda_positional.ori`
  - [ ] **LLVM Support**: LLVM codegen for lambda positional args
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/call_tests.rs` — lambda positional arg codegen

- [ ] **Implement**: Verify callee has exactly 1 explicit parameter (exclude `self`)
  - [ ] **Rust Tests**: `ori_typeck/src/infer/call.rs` — single param check
  - [ ] **Ori Tests**: `tests/spec/expressions/lambda_positional_single_param.ori`
  - [ ] **LLVM Support**: LLVM codegen for single param check
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/call_tests.rs` — single param check codegen

- [ ] **Implement**: Verify argument expression is a `LambdaExpr` AST node
  - [ ] **Rust Tests**: `ori_typeck/src/infer/call.rs` — lambda detection
  - [ ] **Ori Tests**: `tests/spec/expressions/lambda_positional_detection.ori`
  - [ ] **LLVM Support**: LLVM codegen for lambda detection
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/call_tests.rs` — lambda detection codegen

- [ ] **Implement**: Reject positional for function references/variables (not lambda literals)
  - [ ] **Rust Tests**: `ori_typeck/src/infer/call.rs` — function reference rejection
  - [ ] **Ori Tests**: `tests/compile-fail/positional_function_reference.ori`
  - [ ] **LLVM Support**: LLVM codegen for function reference rejection
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/call_tests.rs` — function reference rejection codegen

### Error Messages

- [ ] **Implement**: Clear error when using positional non-lambda for single-param function
  - [ ] **Rust Tests**: `ori_diagnostic/src/problem.rs` — positional non-lambda error
  - [ ] **Ori Tests**: `tests/compile-fail/positional_non_lambda_message.ori`

```
error[E2011]: named arguments required for direct function calls
  --> src/main.ori:5:12
   |
5  |     items.map(double)
   |               ^^^^^^
   |
   = help: use named argument syntax: `map(transform: double)`
   = note: positional arguments are only allowed for inline lambda
           expressions, not function references
```

### Edge Cases

- [ ] **Implement**: Nested lambdas work correctly
  - [ ] **Ori Tests**: `tests/spec/expressions/lambda_positional_nested.ori`
  - [ ] **LLVM Support**: LLVM codegen for nested lambdas
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/call_tests.rs` — nested lambdas codegen

- [ ] **Implement**: Chained method calls with lambdas
  - [ ] **Ori Tests**: `tests/spec/expressions/lambda_positional_chained.ori`
  - [ ] **LLVM Support**: LLVM codegen for chained lambdas
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/call_tests.rs` — chained lambdas codegen

- [ ] **Implement**: Lambda returning lambda
  - [ ] **Ori Tests**: `tests/spec/expressions/lambda_returning_lambda.ori`
  - [ ] **LLVM Support**: LLVM codegen for lambda returning lambda
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/call_tests.rs` — lambda returning lambda codegen

### Documentation

- [ ] **Implement**: Update spec `09-expressions.md` with lambda positional exception
- [ ] **Implement**: Update `CLAUDE.md` with lambda positional syntax

---

## 15B.5 Argument Punning (Call Arguments)

**Proposal**: `proposals/approved/argument-punning-proposal.md`

Allow omitting the value in a named function argument when the argument name matches the variable name: `f(x:)` for `f(x: x)`. Parser-only desugaring — type checker, evaluator, and LLVM see the expanded form.

```ori
// Before:
conv2d(input: input, weight: weight, bias: bias, stride: 2)

// After:
conv2d(input:, weight:, bias:, stride: 2)
```

### Parser

- [ ] **Implement**: In call argument parsing, when `name:` followed by `,` or `)`, create synthetic `Expr::Ident`
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr/postfix/tests.rs` — punned call arg parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/argument_punning.ori`

- [ ] **Implement**: Mixed punned and explicit arguments parse correctly
  - [ ] **Ori Tests**: `tests/spec/expressions/argument_punning_mixed.ori`

- [ ] **Implement**: `f(x)` positional unchanged (no regression)
  - [ ] **Ori Tests**: `tests/spec/expressions/positional_arg_regression.ori`

### Error Messages

- [ ] **Implement**: `f(x:)` when `x` not in scope produces "cannot find value `x`"
  - [ ] **Ori Tests**: `tests/compile-fail/punning_not_in_scope.ori`

- [ ] **Implement**: `f(x:)` when function has no param `x` produces existing "unknown parameter" error
  - [ ] **Ori Tests**: `tests/compile-fail/punning_unknown_param.ori`

### Formatter

- [ ] **Implement**: Detect `name == value_ident` in call args and emit `name:` form
  - [ ] **Rust Tests**: `ori_fmt/src/formatter/` — call arg punning canonicalization

- [ ] **Implement**: Preserve `f(x: other)` — no punning when names differ
  - [ ] **Rust Tests**: `ori_fmt/src/formatter/` — non-punning preservation

### Documentation

- [ ] **Implement**: Update spec `09-expressions.md` with call argument punning
- [ ] **Implement**: Update `grammar.ebnf` with optional expression in `named_arg`
- [ ] **Implement**: Update `.claude/rules/ori-syntax.md` with punning syntax

---

## 15B.6 Section Completion Checklist

- [ ] All implementation items have checkboxes marked `[ ]`
- [ ] All spec docs updated
- [ ] CLAUDE.md updated with syntax changes
- [ ] Migration tools working
- [ ] All tests pass: `./test-all.sh`

**Exit Criteria**: Function syntax proposals implemented
