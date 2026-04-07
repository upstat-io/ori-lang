---
section: "15B"
title: Function Syntax
status: in-progress
reviewed: true
last_verified: "2026-03-29"
tier: 5
goal: Implement function-related syntax proposals
sections:
  - id: "15B.1"
    title: Remove Dot Prefix from Named Arguments
    status: in-progress
  - id: "15B.2"
    title: Default Parameter Values
    status: in-progress
  - id: "15B.3"
    title: Multiple Function Clauses
    status: in-progress
  - id: "15B.4"
    title: Positional Lambdas for Single-Parameter Functions
    status: done
  - id: "15B.5"
    title: Argument Punning (Call Arguments)
    status: done
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

- [x] **Done**: Parser accepts `IDENTIFIER ':'` instead of `'.' IDENTIFIER ':'` (verified 2026-03-29)
  - `is_named_arg_start()` in `ori_parse/src/cursor/mod.rs:354` detects `IDENTIFIER ':'`
  - `postfix.rs:397-416` parses call args using this detection
  - All 4181 spec tests use named args exclusively (zero positional `print("...")`calls found)

- [ ] **Implement**: Enforce named arguments for built-in functions
  - Parser creates `Call` (positional) or `CallNamed` (named) but type checker does not reject positional calls to builtins -- convention holds by test discipline only, no compiler enforcement
  - [ ] **Rust Tests**: `ori_parse/src/grammar/call.rs` -- builtin named arg enforcement
  - [ ] **Ori Tests**: `tests/spec/expressions/builtin_named_args.ori`
  - [ ] **Ori Tests**: `tests/compile-fail/builtin_positional_args.ori`

- [ ] **Implement**: Allow positional only for function variable calls
  - [ ] **Rust Tests**: `ori_parse/src/grammar/call.rs` -- function var positional
  - [ ] **Ori Tests**: `tests/spec/expressions/function_var_positional.ori`

- [ ] **Implement**: Clear error message when positional used incorrectly
  - [ ] **Rust Tests**: `ori_diagnostic/src/problem.rs` -- positional arg error
  - [ ] **Ori Tests**: `tests/compile-fail/positional_arg_error.ori`

#### Built-in Function Updates

All built-in functions already accept named arguments by convention. All spec tests use named args. The remaining work is **enforcement** (rejecting positional calls), not syntax changes. (verified 2026-03-29)

- [ ] **Implement**: Enforce `print` requires `msg:` parameter (currently works but not enforced)

- [ ] **Implement**: Enforce `len` requires `collection:` parameter

- [ ] **Implement**: Enforce `is_empty` requires `collection:` parameter

- [ ] **Implement**: Enforce `assert` requires `condition:` parameter

- [ ] **Implement**: Enforce `assert_eq` requires `actual:`, `expected:` parameters

- [ ] **Implement**: Enforce `assert_ne` requires `actual:`, `unexpected:` parameters

- [ ] **Implement**: Enforce `assert_some`, `assert_none` require `option:` parameter

- [ ] **Implement**: Enforce `assert_ok`, `assert_err` require `result:` parameter

- [ ] **Implement**: Enforce `assert_panics` requires `f:` parameter

- [ ] **Implement**: Enforce `assert_panics_with` requires `f:`, `msg:` parameters

- [ ] **Implement**: Enforce `panic` requires `msg:` parameter

- [ ] **Implement**: Enforce `compare`, `min`, `max` require `left:`, `right:` parameters

- [ ] **Implement**: Enforce `repeat` requires `value:` parameter

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

### Parser (verified 2026-03-29)

- [x] **Done**: Extend `param` production to accept `= expression` after type (verified 2026-03-29)
  - `ori_parse/src/grammar/item/function/mod.rs:502-508` parses `= expression` after type annotation
  - `Param` struct has `default: Option<ExprId>` field

- [x] **Done**: Parse default expressions with correct precedence (verified 2026-03-29)
  - Default expressions parsed via standard `parse_expr()` call

### IR & Desugaring (verified 2026-03-29)

- [x] **Done**: IR representation for default values (verified 2026-03-29)
  - `ori_ir/src/ast/items/traits.rs:41` has `default_value: Option<ExprId>`
  - Function params have `default` field

- [x] **Done**: Canon desugaring fills omitted args with defaults (verified 2026-03-29)
  - `ori_canon/src/desugar/calls.rs:122-133` fills empty slots with default expressions
  - Named arg reordering + default filling is implemented
  - Default filling is canon-layer desugaring -- LLVM sees regular function calls (no dedicated LLVM work needed)

- [x] **Done**: Formatter handles default values (verified 2026-03-29)
  - `ori_fmt/src/declarations/functions.rs:338` formats default value expressions

- [x] **Done**: Type checker propagates `default_value` (verified 2026-03-29)
  - `ori_types/src/check/signatures/mod.rs:154` propagates `default_value`
  - `ori_types/src/output/mod.rs:347` has `default_value: Option<ExprId>`

### Type Checker (remaining work)

- [ ] **Implement**: Verify default expression has parameter's type
  - [ ] **Rust Tests**: `ori_types/src/check/params.rs` -- default type checking
  - [ ] **Ori Tests**: `tests/compile-fail/default_param_type_mismatch.ori`

- [ ] **Implement**: Verify default doesn't reference other parameters
  - [ ] **Rust Tests**: `ori_types/src/check/params.rs` -- default param reference checking
  - [ ] **Ori Tests**: `tests/compile-fail/default_param_references_other.ori`

- [ ] **Implement**: Track which parameters have defaults for call validation
  - [ ] **Rust Tests**: `ori_types/src/check/call.rs` -- optional parameter tracking
  - [ ] **Ori Tests**: `tests/spec/expressions/call_with_defaults.ori`

- [ ] **Implement**: Capability checking for default expressions
  - [ ] **Rust Tests**: `ori_types/src/check/params.rs` -- default capability checking
  - [ ] **Ori Tests**: `tests/spec/capabilities/default_param_capabilities.ori`

### Call Site Validation

- [ ] **Implement**: Required parameters (no default) must be provided
  - [ ] **Rust Tests**: `ori_types/src/check/call.rs` -- required param validation
  - [ ] **Ori Tests**: `tests/compile-fail/missing_required_param.ori`

- [ ] **Implement**: Allow omitting parameters with defaults
  - [ ] **Rust Tests**: `ori_types/src/check/call.rs` -- optional param omission
  - [ ] **Ori Tests**: `tests/spec/expressions/omit_default_params.ori`

- [ ] **Implement**: Clear error message when required param missing
  - [ ] **Rust Tests**: `ori_diagnostic/src/problem.rs` -- missing param error
  - [ ] **Ori Tests**: `tests/compile-fail/missing_required_param_message.ori`

### End-to-End Verification (verified 2026-03-29 -- UNVERIFIED)

Default parameter filling happens in the canon layer (`ori_canon/src/desugar/calls.rs`), so LLVM sees regular calls with all args filled. However, end-to-end status is **unverified** -- all tests in `clause_params.ori` (lines 43-130, 319-368) are COMMENTED OUT (~350 lines). The canon desugaring path may not correctly feed defaults through the ARC pipeline.

- [ ] **Verify**: Un-comment default parameter tests in `clause_params.ori` or create `tests/spec/declarations/default_params.ori` to confirm end-to-end path works
- [ ] **Verify**: Evaluate defaults at call time (not definition time)
- [ ] **Verify**: Correct evaluation order (explicit args first, then defaults in param order)

### Trait Method Defaults

- [ ] **Implement**: Allow defaults in trait method signatures
  - Trait system has `has_default` for default method *implementations*, but default *parameter values* in trait method signatures are not confirmed
  - [ ] **Ori Tests**: `tests/spec/traits/method_defaults.ori`

- [ ] **Implement**: Allow implementations to override/remove defaults
  - [ ] **Ori Tests**: `tests/spec/traits/impl_override_defaults.ori`

- [ ] **Implement**: Trait object calls use trait's declared default
  - [ ] **Ori Tests**: `tests/spec/traits/dyn_trait_defaults.ori`

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

### Parser (verified 2026-03-29)

- [x] **Done**: Allow `match_pattern` in parameter position (`clause_param`) (verified 2026-03-29)
  - `ori_parse/src/grammar/item/function/mod.rs:373-450` supports clause parameters:
    - Literal patterns: `(0: int)`, `(true: bool)`, `("hello": str)` -- lines 406-417
    - List patterns: `([]: [T])`, `([_, ..tail]: [T])` -- lines 420-428
    - Negative literals: `(-42: int)` -- lines 429-433

- [x] **Done**: Parse `if` guard clause between `where_clause` and `=` (verified 2026-03-29)
  - Guard clauses parsed at lines 115-127

- [x] **Done**: Group multiple declarations with same name into single function (verified 2026-03-29)
  - Canon grouping: `ori_canon/src/lower/mod.rs:112-141` groups functions by name
  - Evaluator grouping: `ori_eval/src/module_registration/mod.rs:54-94` groups functions by name
  - Comment at line 54: "Functions with the same name are grouped as multi-clause functions"

### Canon Multi-Clause Lowering (verified 2026-03-29)

- [x] **Done**: Desugar clauses to single function with `match` (verified 2026-03-29)
  - `ori_canon/src/lower/patterns.rs:117-180+` implements full multi-clause lowering:
    - Uses first clause's span/type as overall function info (line 121-122)
    - Determines parameter names from first clause (line 125)
    - Flattens each clause's parameter patterns into FlatPattern rows (line 147-150)
    - Extracts guards per clause (line 157)
    - Compiles patterns via `compile_multi_clause_patterns()` (line 161)
    - Performs exhaustiveness checking with clause spans (lines 163-169)
    - Lowers each clause body (lines 178-181)
  - This is canon-layer desugaring -- LLVM sees a regular function with a match expression (no dedicated LLVM work needed)

- [x] **Done**: Function clause `if` guards (compile to match arm guards) (verified 2026-03-29)
  - Guard extraction at line 157 of `patterns.rs`

- [x] **Done**: Exhaustiveness checking across all clauses (verified 2026-03-29)
  - Canon layer has exhaustiveness checking (lines 163-169)

### Semantic Analysis (remaining work)

- [ ] **Implement**: Validate all clauses have same parameter count
  - [ ] **Ori Tests**: `tests/compile-fail/clause_param_count_mismatch.ori`

- [ ] **Implement**: Validate all clauses have same return type
  - [ ] **Ori Tests**: `tests/compile-fail/clause_return_type_mismatch.ori`

- [ ] **Implement**: Validate all clauses have same capabilities (`uses`)
  - [ ] **Ori Tests**: `tests/compile-fail/clause_capability_mismatch.ori`

- [ ] **Implement**: First clause rules (visibility, generics, types)
  - [ ] **Ori Tests**: `tests/spec/declarations/first_clause_rules.ori`

- [ ] **Implement**: Type inference for subsequent clause parameters
  - [ ] **Ori Tests**: `tests/spec/declarations/clause_type_inference.ori`

- [ ] **Implement**: Error if visibility/generics repeated on subsequent clauses
  - [ ] **Ori Tests**: `tests/compile-fail/clause_duplicate_modifiers.ori`

### Exhaustiveness & Reachability (remaining work)

- [ ] **Implement**: Unreachable clause detection and warnings
  - [ ] **Ori Tests**: `tests/warnings/unreachable_clause.ori`

### End-to-End Verification (verified 2026-03-29 -- UNVERIFIED)

All tests in `clause_params.ori` (lines 138-314, ~350 lines) are COMMENTED OUT. The inline status comments ("TypeChecker [NEEDS IMPL]", "Evaluator [NEEDS IMPL]") may be STALE -- the canon layer implementation may have superseded these blockers. Whether the full pipeline (typeck through eval/LLVM) works is unverified because no tests are active.

RECOMMENDED: Un-comment the simplest tests (factorial, fibonacci) with `#skip` if they fail, to determine actual end-to-end status.

- [ ] **Verify**: Un-comment or recreate clause tests to confirm end-to-end pipeline works
- [ ] **Ori Tests**: `tests/spec/declarations/function_clauses.ori`

### Integration

- [ ] **Implement**: Named argument reordering before pattern matching
  - [ ] **Ori Tests**: `tests/spec/expressions/clause_named_args.ori`

- [ ] **Implement**: Default parameter filling before pattern matching
  - [ ] **Ori Tests**: `tests/spec/expressions/clause_default_params.ori`

- [ ] **Implement**: Tests target function name (cover all clauses)
  - [ ] **Ori Tests**: `tests/spec/testing/clause_tests.ori`

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

### Core Functionality (verified 2026-03-29)

Positional lambdas for single-parameter method calls work throughout the codebase and are used extensively in production tests. The feature works because the type system naturally handles positional `Call` with lambda arguments -- no dedicated implementation was required. Lambda desugaring is parser-level, so LLVM sees regular function calls (no dedicated LLVM work needed).

- [x] **Done**: Lambda-literal positional argument works for single-param methods (verified 2026-03-29)
  - Evidence from passing tests (4181 passed):
    - `[1, 2, 3].iter().map(x -> x * 10)` -- `tests/spec/traits/iterator/double_ended.ori:183`
    - `[1, 2, 3, 4, 5, 6].iter().filter(x -> x % 2 == 0)` -- same file line 217
    - `[1, 2, 3].iter().map(x -> x * 10).rev().collect()` -- `double_ended_gating.ori:65`

- [x] **Done**: Parser creates `Call` (positional) for non-named args (verified 2026-03-29)
  - Lambda expressions passed positionally flow through the `Call` path naturally

- [x] **Done**: Lambda argument type inference from expected function type (verified 2026-03-29)
  - Works through standard call inference

### Enforcement (remaining work)

- [ ] **Implement**: Reject positional for function references/variables (not lambda literals)
  - The proposal says `list.map(double)` where `double` is a function reference should require `list.map(transform: double)` -- not currently enforced
  - [ ] **Ori Tests**: `tests/compile-fail/positional_function_reference.ori`

### Error Messages

- [ ] **Implement**: Clear error when using positional non-lambda for single-param function (E2011)
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

### Dedicated Tests (remaining work)

No dedicated test files exist. Feature is tested incidentally through iterator tests.

- [ ] **Implement**: Dedicated positional lambda test file
  - [ ] **Ori Tests**: `tests/spec/expressions/lambda_positional.ori`

- [ ] **Implement**: Nested lambdas test
  - [ ] **Ori Tests**: `tests/spec/expressions/lambda_positional_nested.ori`

- [ ] **Implement**: Chained method calls with lambdas test
  - [ ] **Ori Tests**: `tests/spec/expressions/lambda_positional_chained.ori`

- [ ] **Implement**: Lambda returning lambda test
  - [ ] **Ori Tests**: `tests/spec/expressions/lambda_returning_lambda.ori`

### Documentation

- [ ] **Implement**: Update spec `09-expressions.md` with lambda positional exception
- [x] **Done**: `CLAUDE.md` documents lambda positional syntax (verified 2026-03-29)

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

### Parser (verified 2026-03-29)

- [x] **Done**: In call argument parsing, when `name:` followed by `,` or `)`, create synthetic `Expr::Ident` (verified 2026-03-29)
  - `ori_parse/src/grammar/expr/postfix.rs:402-411` implements punning detection
  - Creates synthetic `Expr::Ident` with the argument name when no value follows

- [x] **Done**: Variant pattern field punning (verified 2026-03-29)
  - `ori_parse/src/grammar/expr/patterns/match_patterns.rs:443` uses `is_named_arg_start()` for variant pattern field punning

### Tests (verified 2026-03-29)

- [x] **Done**: Core positive tests pass (verified 2026-03-29)
  - `tests/spec/declarations/argument_punning.ori` -- 6 tests: single, multi, mixed, string, method, partial punning
  - `tests/spec/patterns/variant_punning.ori` -- 6 tests: single-field, multi-field, Option, Result, positional regression
  - All pass as part of full suite (4181 passed)

- [x] **Done**: Mixed punned and explicit arguments work (verified 2026-03-29)
  - Covered by `argument_punning.ori` mixed test

- [x] **Done**: `f(x)` positional unchanged -- no regression (verified 2026-03-29)
  - Covered by `variant_punning.ori` positional regression test

### Error Messages (remaining work)

- [ ] **Implement**: `f(x:)` when `x` not in scope produces "cannot find value `x`"
  - [ ] **Ori Tests**: `tests/compile-fail/punning_not_in_scope.ori`

- [ ] **Implement**: `f(x:)` when function has no param `x` produces existing "unknown parameter" error
  - [ ] **Ori Tests**: `tests/compile-fail/punning_unknown_param.ori`

### Interaction Tests (remaining work)

- [ ] **Implement**: Punning + default parameters
- [ ] **Implement**: Punning + generic functions
- [ ] **Implement**: Punning + trait methods

### Formatter (remaining work)

- [ ] **Implement**: Detect `name == value_ident` in call args and emit `name:` form
  - `ori fmt` does NOT canonicalize `f(x: x)` to `f(x:)` -- grep for "pun" in `ori_fmt` finds nothing
  - [ ] **Rust Tests**: `ori_fmt/src/formatter/` -- call arg punning canonicalization

- [ ] **Implement**: Preserve `f(x: other)` -- no punning when names differ
  - [ ] **Rust Tests**: `ori_fmt/src/formatter/` -- non-punning preservation

### Documentation (verified 2026-03-29)

- [x] **Done**: `grammar.ebnf` shows `named_arg = identifier ":" [ expression ]` (verified 2026-03-29)
- [x] **Done**: `.claude/rules/ori-syntax.md` documents `f(x:) = f(x: x)` (verified 2026-03-29)
- [ ] **Implement**: Update spec `09-expressions.md` with call argument punning

---

## 15B.6 Section Completion Checklist

- [ ] All implementation items have checkboxes marked `[x]`
- [ ] All spec docs updated
- [x] CLAUDE.md updated with punning and lambda positional syntax (verified 2026-03-29)
- [ ] Migration tools working (`ori migrate remove-dot-prefix` not implemented)
- [ ] All tests pass: `./test-all.sh` -- 4181 pass, 0 fail, but ~350 lines of tests in `clause_params.ori` are COMMENTED OUT (verified 2026-03-29)
- [ ] `/tpr-review` passed -- independent Codex review found no critical or major issues (or all findings triaged)
- [ ] `/impl-hygiene-review` passed — implementation hygiene review clean (phase boundaries, SSOT, algorithmic DRY, naming). MUST run AFTER `/tpr-review` is clean.
- [ ] `/improve-tooling` retrospective completed — MANDATORY at section close, after both reviews are clean. Reflect on the section's debugging journey (which `diagnostics/` scripts you ran, which command sequences you repeated, where you added ad-hoc `dbg!`/`tracing` calls, where output was hard to interpret) and identify any tool/log/diagnostic improvement that would have made this section materially easier OR that would help the next section touching this area. Implement every accepted improvement NOW (zero deferral) and commit each via SEPARATE `/commit-push`. The retrospective is mandatory even when nothing felt painful — that is exactly when blind spots accumulate. See `.claude/skills/improve-tooling/SKILL.md` "Retrospective Mode" for the full protocol.
- [ ] Commented-out tests in `clause_params.ori` un-commented or converted to `#skip("reason")` -- per coding rules, commented-out code is never acceptable

**Exit Criteria**: Function syntax proposals implemented
