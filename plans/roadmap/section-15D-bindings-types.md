---
section: "15D"
title: Bindings & Types
status: not-started
tier: 1
goal: Implement binding syntax changes and type system simplifications
priority_note: "15D.3 escalated from Tier 5 → Tier 1 (2026-02-19). Spec/grammar already removed mut but compiler still accepts it. 163 let mut occurrences across 28 test files — migration cost grows with every commit."
sections:
  - id: "15D.1"
    title: Pre/Post Checks for run Pattern
    status: not-started
  - id: "15D.2"
    title: as Conversion Syntax
    status: not-started
  - id: "15D.3"
    title: Simplified Bindings with $ for Immutability
    status: not-started
  - id: "15D.4"
    title: Remove dyn Keyword for Trait Objects
    status: not-started
  - id: "15D.5"
    title: Index and Field Assignment
    status: not-started
  - id: "15D.6"
    title: Section Completion Checklist
    status: not-started
---

# Section 15D: Bindings & Types

**Goal**: Implement binding syntax changes and type system simplifications

> **Source**: `docs/ori_lang/proposals/approved/`

---

## 15D.1 Pre/Post Checks for `run` Pattern

**Proposal**: `proposals/approved/checks-proposal.md`

Extend `run` pattern with `pre_check:` and `post_check:` properties for contract-style defensive programming.

```ori
@divide (a: int, b: int) -> int = run(
    pre_check: b != 0,
    a div b,
    post_check: r -> r * b <= a,
)

// Multiple conditions via multiple properties
@transfer (from: Account, to: Account, amount: int) -> (Account, Account) = run(
    pre_check: amount > 0 | "amount must be positive",
    pre_check: from.balance >= amount | "insufficient funds",
    // ... body ...,
    post_check: (f, t) -> f.balance == from.balance - amount,
    post_check: (f, t) -> t.balance == to.balance + amount,
)
```

### Key Design Decisions

- **Multiple properties, not list syntax**: Use multiple `pre_check:` / `post_check:` properties instead of `[cond1, cond2]` lists
- **`|` for messages**: Custom messages use `condition | "message"` syntax (parser disambiguates by context)
- **Scope constraints**: `pre_check:` can only access outer scope; `post_check:` can access body bindings
- **Void body**: Compile error if `post_check:` used with void body
- **Check modes deferred**: `check_mode:` (enforce/observe/ignore) deferred to future proposal

### Implementation

- [ ] **Implement**: Parser: Add `pre_check:` and `post_check:` to run pattern
  - [ ] **Rust Tests**: `ori_parse/src/grammar/pattern.rs` — check property parsing
  - [ ] **Ori Tests**: `tests/spec/patterns/checks.ori`
  - [ ] **LLVM Support**: LLVM codegen for pre_check and post_check parsing
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/syntax_tests.rs` — pre_check/post_check parsing codegen

- [ ] **Implement**: Parser: Enforce position (pre_check first, post_check last)
  - [ ] **Rust Tests**: `ori_parse/src/grammar/pattern.rs` — position enforcement
  - [ ] **Ori Tests**: `tests/compile-fail/checks/mispositioned_checks.ori`
  - [ ] **LLVM Support**: LLVM codegen for check position enforcement
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/syntax_tests.rs` — check position enforcement codegen

- [ ] **Implement**: Parser: Support `| "message"` custom message syntax
  - [ ] **Rust Tests**: `ori_parse/src/grammar/pattern.rs` — message parsing
  - [ ] **Ori Tests**: `tests/spec/patterns/check_messages.ori`
  - [ ] **LLVM Support**: LLVM codegen for custom check messages
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/syntax_tests.rs` — custom check messages codegen

- [ ] **Implement**: Type checker: Validate pre_check is `bool`
  - [ ] **Rust Tests**: `oric/src/typeck/checker/pattern.rs` — pre_check type validation
  - [ ] **Ori Tests**: `tests/compile-fail/checks/pre_check_not_bool.ori`
  - [ ] **LLVM Support**: LLVM codegen for pre_check type validation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/syntax_tests.rs` — pre_check type validation codegen

- [ ] **Implement**: Type checker: Validate post_check is `T -> bool` lambda
  - [ ] **Rust Tests**: `oric/src/typeck/checker/pattern.rs` — post_check type validation
  - [ ] **Ori Tests**: `tests/compile-fail/checks/post_check_not_lambda.ori`
  - [ ] **LLVM Support**: LLVM codegen for post_check type validation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/syntax_tests.rs` — post_check type validation codegen

- [ ] **Implement**: Type checker: Error when post_check used with void body
  - [ ] **Rust Tests**: `oric/src/typeck/checker/pattern.rs` — void body error
  - [ ] **Ori Tests**: `tests/compile-fail/checks/post_check_void_body.ori`
  - [ ] **LLVM Support**: LLVM codegen for void body error
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/syntax_tests.rs` — void body error codegen

- [ ] **Implement**: Scope checker: pre_check can only access outer scope
  - [ ] **Rust Tests**: `oric/src/resolve/scope.rs` — pre_check scope validation
  - [ ] **Ori Tests**: `tests/compile-fail/checks/pre_check_scope.ori`
  - [ ] **LLVM Support**: LLVM codegen for pre_check scope validation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/syntax_tests.rs` — pre_check scope validation codegen

- [ ] **Implement**: Codegen: Desugar to conditional checks and panics
  - [ ] **Rust Tests**: `oric/src/desugar/checks.rs` — check desugaring
  - [ ] **Ori Tests**: `tests/spec/patterns/checks_desugaring.ori`
  - [ ] **LLVM Support**: LLVM codegen for check desugaring
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/syntax_tests.rs` — check desugaring codegen

- [ ] **Implement**: Codegen: Embed source text for default error messages
  - [ ] **Rust Tests**: `oric/src/desugar/checks.rs` — source text embedding
  - [ ] **Ori Tests**: `tests/spec/patterns/checks_error_messages.ori`
  - [ ] **LLVM Support**: LLVM codegen for source text embedding
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/syntax_tests.rs` — source text embedding codegen

---

## 15D.2 `as` Conversion Syntax

**Proposal**: `proposals/approved/as-conversion-proposal.md`

Replace `int()`, `float()`, `str()`, `byte()` with `as`/`as?` keyword syntax backed by `As<T>` and `TryAs<T>` traits.

```ori
// Before (special-cased positional args)
let x = int("42")
let y = float(value)

// After (consistent keyword syntax)
let x = "42" as? int
let y = value as float
```

### Lexer

- [ ] **Implement**: `as` keyword token (if not already reserved)
  - [ ] **Rust Tests**: `ori_lexer/src/lib.rs` — as keyword tokenization
  - [ ] **Ori Tests**: `tests/spec/lexical/as_keyword.ori`
  - [ ] **LLVM Support**: LLVM codegen for as keyword
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/as_conversion_tests.rs` — as keyword codegen

### Parser

- [ ] **Implement**: Parse `expression as Type` as conversion expression
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr.rs` — as expression parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/as_syntax.ori`
  - [ ] **LLVM Support**: LLVM codegen for as expression
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/as_conversion_tests.rs` — as expression codegen

- [ ] **Implement**: Parse `expression as? Type` as fallible conversion
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr.rs` — as? expression parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/as_fallible_syntax.ori`
  - [ ] **LLVM Support**: LLVM codegen for as? expression
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/as_conversion_tests.rs` — as? expression codegen

### Type Checker

- [ ] **Implement**: Validate `as` only used with `As<T>` trait implementations
  - [ ] **Rust Tests**: `oric/src/typeck/checker/as_expr.rs`
  - [ ] **Ori Tests**: `tests/compile-fail/as_not_implemented.ori`
  - [ ] **LLVM Support**: LLVM codegen for As<T> trait validation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/as_conversion_tests.rs` — As<T> trait validation codegen

- [ ] **Implement**: Validate `as?` only used with `TryAs<T>` trait implementations
  - [ ] **Rust Tests**: `oric/src/typeck/checker/as_expr.rs`
  - [ ] **Ori Tests**: `tests/compile-fail/try_as_not_implemented.ori`
  - [ ] **LLVM Support**: LLVM codegen for TryAs<T> trait validation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/as_conversion_tests.rs` — TryAs<T> trait validation codegen

- [ ] **Implement**: Error when using `as` for fallible conversion (must use `as?`)
  - [ ] **Rust Tests**: `oric/src/typeck/checker/as_expr.rs`
  - [ ] **Ori Tests**: `tests/compile-fail/as_fallible_conversion.ori`
  - [ ] **LLVM Support**: LLVM codegen for fallible conversion error
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/as_conversion_tests.rs` — fallible conversion error codegen

### Codegen

- [ ] **Implement**: Desugar `x as T` to `As<T>.as(self: x)`
  - [ ] **LLVM Support**: LLVM codegen for as desugaring
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/as_conversion_tests.rs` — as desugaring codegen
- [ ] **Implement**: Desugar `x as? T` to `TryAs<T>.try_as(self: x)`
  - [ ] **LLVM Support**: LLVM codegen for as? desugaring
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/as_conversion_tests.rs` — as? desugaring codegen

### Migration

- [ ] **Implement**: Remove `int()`, `float()`, `str()`, `byte()` from parser
  - [ ] **LLVM Support**: LLVM codegen for type conversion removal
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/as_conversion_tests.rs` — type conversion removal codegen
- [ ] **Implement**: Update error messages to suggest `as` syntax
  - [ ] **LLVM Support**: LLVM codegen for as syntax suggestion
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/as_conversion_tests.rs` — as syntax suggestion codegen

---

## 15D.3 Simplified Bindings with `$` for Immutability

**Proposal**: `proposals/approved/simplified-bindings-proposal.md`

Simplify the binding model: `let x` is mutable, `let $x` is immutable. Remove `mut` keyword. Module-level bindings require `$` prefix.

```ori
// Before
let x = 5         // immutable
let mut x = 5     // mutable
$timeout = 30s    // config variable

// After
let x = 5         // mutable
let $x = 5        // immutable
let $timeout = 30s // module-level constant (let and $ required)
```

### Lexer

- [ ] **Implement**: Remove `mut` from reserved keywords
  - [ ] **Rust Tests**: `ori_lexer/src/lib.rs` — keyword list update
  - [ ] **Ori Tests**: `tests/spec/lexical/mut_not_keyword.ori`
  - [ ] **LLVM Support**: LLVM codegen for mut removal
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/binding_tests.rs` — mut removal codegen

### Parser

- [ ] **Implement**: Update `let_expr` to accept `$` prefix in binding pattern
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr.rs` — immutable binding parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/immutable_bindings.ori`
  - [ ] **LLVM Support**: LLVM codegen for immutable binding parsing
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/binding_tests.rs` — immutable binding parsing codegen

- [ ] **Implement**: Remove `mut` from `let_expr` grammar
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr.rs` — mut removal
  - [ ] **Ori Tests**: `tests/compile-fail/let_mut_removed.ori`
  - [ ] **LLVM Support**: LLVM codegen for mut removal
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/binding_tests.rs` — mut removal codegen

- [ ] **Implement**: Update `constant_decl` to require `let $name = expr`
  - [ ] **Rust Tests**: `ori_parse/src/grammar/decl.rs` — constant declaration parsing
  - [ ] **Ori Tests**: `tests/spec/declarations/constants.ori`
  - [ ] **LLVM Support**: LLVM codegen for constant declaration
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/binding_tests.rs` — constant declaration codegen

- [ ] **Implement**: Remove old const function syntax `$name (params) -> Type`
  - [ ] **Rust Tests**: `ori_parse/src/grammar/decl.rs` — const function removal
  - [ ] **Ori Tests**: `tests/compile-fail/old_const_function_syntax.ori`
  - [ ] **LLVM Support**: LLVM codegen for const function removal
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/binding_tests.rs` — const function removal codegen

- [ ] **Implement**: Support `$` prefix in destructuring patterns
  - [ ] **Rust Tests**: `ori_parse/src/grammar/pattern.rs` — destructure immutable parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/destructure_immutable.ori`
  - [ ] **LLVM Support**: LLVM codegen for destructure immutable
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/binding_tests.rs` — destructure immutable codegen

### Semantic Analysis

- [ ] **Implement**: Track `$` modifier separately from identifier name
  - [ ] **Rust Tests**: `oric/src/resolve/binding.rs` — binding modifier tracking
  - [ ] **Ori Tests**: `tests/spec/expressions/binding_modifiers.ori`
  - [ ] **LLVM Support**: LLVM codegen for binding modifier tracking
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/binding_tests.rs` — binding modifier tracking codegen

- [ ] **Implement**: Prevent `$x` and `x` coexisting in same scope
  - [ ] **Rust Tests**: `oric/src/resolve/binding.rs` — same-name conflict detection
  - [ ] **Ori Tests**: `tests/compile-fail/dollar_and_non_dollar_conflict.ori`
  - [ ] **LLVM Support**: LLVM codegen for same-name conflict detection
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/binding_tests.rs` — same-name conflict detection codegen

- [ ] **Implement**: Enforce module-level bindings require `$` prefix
  - [ ] **Rust Tests**: `oric/src/resolve/module.rs` — module binding immutability
  - [ ] **Ori Tests**: `tests/compile-fail/module_level_mutable.ori`
  - [ ] **LLVM Support**: LLVM codegen for module binding immutability
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/binding_tests.rs` — module binding immutability codegen

- [ ] **Implement**: Enforce `$`-prefixed bindings cannot be reassigned
  - [ ] **Rust Tests**: `oric/src/typeck/checker/assignment.rs` — immutable assignment error
  - [ ] **Ori Tests**: `tests/compile-fail/assign_to_immutable.ori`
  - [ ] **LLVM Support**: LLVM codegen for immutable assignment error
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/binding_tests.rs` — immutable assignment error codegen

### Imports

- [ ] **Implement**: Require `$` in import statements for immutable bindings
  - [ ] **Rust Tests**: `oric/src/resolve/import.rs` — import with dollar
  - [ ] **Ori Tests**: `tests/spec/modules/import_immutable.ori`
  - [ ] **LLVM Support**: LLVM codegen for import with dollar
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/binding_tests.rs` — import with dollar codegen

- [ ] **Implement**: Error when importing `$x` as `x` or vice versa
  - [ ] **Rust Tests**: `oric/src/resolve/import.rs` — import modifier mismatch
  - [ ] **Ori Tests**: `tests/compile-fail/import_dollar_mismatch.ori`
  - [ ] **LLVM Support**: LLVM codegen for import modifier mismatch
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/binding_tests.rs` — import modifier mismatch codegen

### Shadowing

- [ ] **Implement**: Allow shadowing to change mutability
  - [ ] **Rust Tests**: `oric/src/resolve/binding.rs` — shadowing mutability change
  - [ ] **Ori Tests**: `tests/spec/expressions/shadow_mutability.ori`
  - [ ] **LLVM Support**: LLVM codegen for shadowing mutability change
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/binding_tests.rs` — shadowing mutability change codegen

### Error Messages

- [ ] **Implement**: Clear error for reassignment to immutable binding
  - [ ] **Rust Tests**: `ori_diagnostic/src/problem.rs` — immutable reassign error
  - [ ] **Ori Tests**: `tests/compile-fail/immutable_reassign_message.ori`
  - [ ] **LLVM Support**: LLVM codegen for immutable reassign error
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/binding_tests.rs` — immutable reassign error codegen

- [ ] **Implement**: Clear error for module-level mutable binding
  - [ ] **Rust Tests**: `ori_diagnostic/src/problem.rs` — module mutable error
  - [ ] **Ori Tests**: `tests/compile-fail/module_mutable_message.ori`
  - [ ] **LLVM Support**: LLVM codegen for module mutable error
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/binding_tests.rs` — module mutable error codegen

- [ ] **Implement**: Migration hint for old `let mut` syntax
  - [ ] **Rust Tests**: `ori_diagnostic/src/problem.rs` — let mut migration hint
  - [ ] **Ori Tests**: `tests/compile-fail/let_mut_migration.ori`
  - [ ] **LLVM Support**: LLVM codegen for let mut migration hint
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/binding_tests.rs` — let mut migration hint codegen

---

## 15D.4 Remove `dyn` Keyword for Trait Objects

**Proposal**: `proposals/approved/remove-dyn-keyword-proposal.md`

Remove the `dyn` keyword for trait objects. Trait names used directly as types mean "any value implementing this trait."

```ori
// Before
@process (item: dyn Printable) -> void = ...
let items: [dyn Serializable] = ...

// After
@process (item: Printable) -> void = ...
let items: [Serializable] = ...
```

### Implementation

- [ ] **Implement**: Remove `"dyn" type` from grammar type production
  - [ ] **Rust Tests**: `ori_parse/src/grammar/ty.rs` — dyn removal tests
  - [ ] **Ori Tests**: `tests/spec/types/trait_objects.ori`
  - [ ] **LLVM Support**: LLVM codegen for trait objects without dyn
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/trait_object_tests.rs`

- [ ] **Implement**: Parser recognizes trait name in type position as trait object
  - [ ] **Rust Tests**: `ori_parse/src/grammar/ty.rs` — trait-as-type parsing
  - [ ] **Ori Tests**: `tests/spec/types/trait_objects.ori`
  - [ ] **LLVM Support**: LLVM codegen for trait-as-type
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/trait_object_tests.rs`

- [ ] **Implement**: Type checker distinguishes `item: Trait` (trait object) vs `<T: Trait>` (generic bound)
  - [ ] **Rust Tests**: `oric/src/typeck/checker/trait_objects.rs`
  - [ ] **Ori Tests**: `tests/spec/types/trait_vs_bound.ori`
  - [ ] **LLVM Support**: LLVM codegen for trait object vs bound distinction
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/trait_object_tests.rs`

- [ ] **Implement**: Object safety validation with clear error messages
  - [ ] **Rust Tests**: `oric/src/typeck/checker/object_safety.rs`
  - [ ] **Ori Tests**: `tests/compile-fail/non_object_safe_trait.ori`
  - [ ] **LLVM Support**: LLVM codegen for object safety validation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/trait_object_tests.rs`

- [ ] **Implement**: Error if `dyn` keyword is used (helpful migration message)
  - [ ] **Rust Tests**: `ori_parse/src/grammar/ty.rs` — dyn keyword error
  - [ ] **Ori Tests**: `tests/compile-fail/dyn_keyword_removed.ori`

---

## 15D.5 Index and Field Assignment

**Proposal**: `proposals/approved/index-assignment-proposal.md`

Extend assignment targets to support index expressions (`list[i] = x`), field access (`state.name = x`), mixed chains (`state.items[i] = x`, `list[i].name = x`), and compound assignment on all forms (`list[i] += 1`). All forms desugar to copy-on-write reassignment via `IndexSet` trait (for index) or struct spread (for fields).

```ori
let list = [1, 2, 3]
list[0] = 10                          // list = list.updated(key: 0, value: 10)

let state = GameState { score: 0, level: 1 }
state.score = 100                     // state = { ...state, score: 100 }
state.items[i] = new_item             // mixed chain
list[i].name = "new"                  // index then field

list[0] += 5                          // compound: list[0] = list[0] + 5
```

### Phase 1: `IndexSet` Trait and `updated` Method

- [ ] **Implement**: Define `IndexSet<Key, Value>` trait in prelude — `@updated (self, key: Key, value: Value) -> Self`
  - [ ] **Rust Tests**: `ori_types/src/infer/` — IndexSet trait resolution
  - [ ] **Ori Tests**: `tests/spec/traits/index_set/basic.ori`

- [ ] **Implement**: Register `updated` as built-in method on `[T]`, `{K: V}`, `[T, max N]` in evaluator
  - [ ] **Rust Tests**: `ori_eval/src/method_dispatch/` — updated method dispatch
  - [ ] **Ori Tests**: `tests/spec/traits/index_set/updated_method.ori`

- [ ] **Implement**: `updated` with ARC-aware copy-on-write in `ori_patterns`/`ori_eval`
  - [ ] **Rust Tests**: `ori_patterns/src/value/` — copy-on-write behavior
  - [ ] **Ori Tests**: `tests/spec/traits/index_set/cow_behavior.ori`

### Phase 2: Parser Changes

- [ ] **Implement**: Extend parser to accept `assignment_target` (identifier + index/field chains) on LHS of `=` and compound operators
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr/` — assignment target parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/index_assignment_syntax.ori`

- [ ] **Implement**: Emit AST node capturing chain of index/field accesses in assignment target
  - [ ] **Rust Tests**: `ori_ir/src/ast/expr.rs` — AssignTarget AST node
  - [ ] **Ori Tests**: `tests/spec/expressions/field_assignment_syntax.ori`

### Phase 3: Type-Directed Desugaring

- [ ] **Implement**: Desugar `[key]` steps to `updated()` calls (requires `IndexSet` trait resolution)
  - [ ] **Rust Tests**: `ori_types/src/infer/expr/` — index assignment desugaring
  - [ ] **Ori Tests**: `tests/spec/expressions/index_assignment_desugar.ori`

- [ ] **Implement**: Desugar `.field` steps to struct spread reconstruction (requires struct type info)
  - [ ] **Rust Tests**: `ori_types/src/infer/expr/` — field assignment desugaring
  - [ ] **Ori Tests**: `tests/spec/expressions/field_assignment_desugar.ori`

- [ ] **Implement**: Handle nested cases, mixed field-index chains, and compound assignment
  - [ ] **Rust Tests**: `ori_types/src/infer/expr/` — mixed chain desugaring
  - [ ] **Ori Tests**: `tests/spec/expressions/mixed_chain_assignment.ori`

### Phase 4: Type Checker Integration

- [ ] **Implement**: Validate mutability of root binding (not `$`, not parameter, not loop variable)
  - [ ] **Rust Tests**: `ori_types/src/infer/expr/` — mutability validation
  - [ ] **Ori Tests**: `tests/spec/expressions/assignment_mutability.ori`

- [ ] **Implement**: Validate field names against struct types in assignment chains
  - [ ] **Rust Tests**: `ori_types/src/infer/expr/` — field validation
  - [ ] **Ori Tests**: `tests/compile-fail/assignment/invalid_field.ori`

- [ ] **Implement**: Validate key and value types against `IndexSet` impl
  - [ ] **Rust Tests**: `ori_types/src/infer/expr/` — IndexSet type validation
  - [ ] **Ori Tests**: `tests/compile-fail/assignment/type_mismatch.ori`

- [ ] **Implement**: Emit diagnostics for all error cases (immutable binding, parameter, loop var, missing IndexSet, field mismatch, type mismatch)
  - [ ] **Rust Tests**: `ori_diagnostic/` — assignment error diagnostics
  - [ ] **Ori Tests**: `tests/compile-fail/assignment/all_errors.ori`

### Phase 5: LLVM Support

- [ ] **LLVM Support**: LLVM codegen for index assignment desugaring
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/` — index assignment codegen
- [ ] **LLVM Support**: LLVM codegen for field assignment desugaring
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/` — field assignment codegen
- [ ] **LLVM Support**: LLVM codegen for compound assignment on extended targets
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/` — compound assignment codegen

---

## 15D.6 Section Completion Checklist

- [ ] All implementation items have checkboxes marked `[ ]`
- [ ] All spec docs updated
- [ ] CLAUDE.md updated with syntax changes
- [ ] Migration tools working
- [ ] All tests pass: `./test-all.sh`

**Exit Criteria**: Binding and type syntax proposals implemented
