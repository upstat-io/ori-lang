---
section: 7A
title: Core Built-ins
status: in-progress
reviewed: true
last_verified: "2026-03-29"
tier: 2
goal: Type conversions, assertions, I/O, and core built-in functions
spec:
  - spec/annex-c-built-in-functions.md
tpr_findings:
  - id: BUG-7A-02
    title: "assert_eq missing Debug bound per spec"
    status: open
    severity: minor
  - id: BUG-7A-03
    title: "compare/min/max are int-only, not generic per spec"
    status: open
    severity: medium
  - id: BUG-7A-04
    title: "assert_some/assert_ok/assert_err return void, not inner value per spec"
    status: open
    severity: medium
  - id: BUG-7A-05
    title: "as/as? conversion tests entirely commented out"
    status: open
    severity: high
sections:
  - id: "7A.1"
    title: Type Conversions
    status: partially-implemented
  - id: "7A.2"
    title: Assertions
    status: in-progress
  - id: "7A.3"
    title: I/O and Other
    status: in-progress
  - id: "7A.4"
    title: Float NaN Behavior
    status: not-started
  - id: "7A.5"
    title: Developer Functions
    status: in-progress
  - id: "7A.6"
    title: Additional Built-in Functions
    status: in-progress
  - id: "7A.7"
    title: Resource Management
    status: not-started
  - id: "7A.8"
    title: Compile-Time File Embedding
    status: not-started
  - id: "7A.9"
    title: Char and Byte Classification Methods
    status: in-progress
  - id: "7A.10"
    title: Byte-Level String Access
    status: in-progress
  - id: "7A.11"
    title: Section Completion Checklist
    status: not-started
---

# Section 7A: Core Built-ins

**Goal**: Type conversions, assertions, I/O, and core built-in functions

> **SPEC**: `spec/annex-c-built-in-functions.md`
> **PROPOSALS**:
> - `proposals/approved/as-conversion-proposal.md` — Type conversion syntax
> - `proposals/approved/developer-functions-proposal.md` — Developer convenience functions
> - `proposals/approved/embed-expression-proposal.md` — Compile-time file embedding
> - `proposals/approved/char-byte-classification-proposal.md` — Char/byte classification methods
> - `proposals/approved/byte-string-access-proposal.md` — Byte-level string access

---

## 7A.1 Type Conversions

> **PROPOSAL**: `proposals/approved/as-conversion-proposal.md`
>
> Type conversions use `as`/`as?` syntax instead of `int()`, `float()`, etc.
> This removes the special-case exception for positional arguments.
>
> **Partial implementation note** (verified 2026-03-29): `as`/`as?` syntax IS parsed (`ExprKind::Cast`),
> type-checked (`infer_cast`), evaluated (`eval_can_cast`), and lowered through ARC IR (`lower_cast`).
> The evaluator handles int-to-float, int-to-byte, int-to-char, char-to-int, byte-to-int, str-to-int (as?),
> str-to-float (as?) via hardcoded dispatch -- NOT trait-based as the As/TryAs proposal envisions.
> **BUG-7A-05**: `tests/spec/expressions/type_conversion.ori` is ENTIRELY COMMENTED OUT -- zero active tests.

- [ ] **Implement**: `As<T>` trait — infallible conversions
  - [ ] **Rust Tests**: `ori_types/src/check/traits/as_trait.rs` — As trait tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/conversions.ori`
  - [ ] **LLVM Support**: LLVM codegen for As trait
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/conversion_tests.rs` — As trait codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `TryAs<T>` trait — fallible conversions returning `Option<T>`
  - [ ] **Rust Tests**: `ori_types/src/check/traits/try_as_trait.rs` — TryAs trait tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/conversions.ori`
  - [ ] **LLVM Support**: LLVM codegen for TryAs trait
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/conversion_tests.rs` — TryAs trait codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `x as T` syntax — desugars to `As<T>.as(self: x)`
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/as_conversion.rs` — as syntax tests
  - [ ] **Ori Tests**: `tests/spec/expressions/as_conversion.ori`
  - [ ] **LLVM Support**: LLVM codegen for as syntax
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/conversion_tests.rs` — as syntax codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `x as? T` syntax — desugars to `TryAs<T>.try_as(self: x)`
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/as_conversion.rs` — as? syntax tests
  - [ ] **Ori Tests**: `tests/spec/expressions/as_conversion.ori`
  - [ ] **LLVM Support**: LLVM codegen for as? syntax
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/conversion_tests.rs` — as? syntax codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Standard `As` implementations
  - `impl int: As<float>` — widening (infallible)
  - `impl int: As<str>` — formatting (infallible)
  - `impl float: As<str>` — formatting (infallible)
  - `impl bool: As<str>` — "true"/"false" (infallible)
  - `impl char: As<int>` — codepoint (infallible)
  - [ ] **Ori Tests**: `tests/spec/stdlib/as_impls.ori`
  - [ ] **LLVM Support**: LLVM codegen for standard As implementations
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/conversion_tests.rs` — As implementations codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Standard `TryAs` implementations
  - `impl str: TryAs<int>` — parsing (fallible)
  - `impl str: TryAs<float>` — parsing (fallible)
  - `impl int: TryAs<byte>` — range check (fallible)
  - `impl int: TryAs<char>` — valid codepoint check (fallible)
  - [ ] **Ori Tests**: `tests/spec/stdlib/try_as_impls.ori`
  - [ ] **LLVM Support**: LLVM codegen for standard TryAs implementations
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/conversion_tests.rs` — TryAs implementations codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Compile-time enforcement — `as` only for infallible conversions
  - [ ] **Rust Tests**: `ori_types/src/check/as_conversion.rs` — enforcement tests
  - [ ] **Ori Tests**: `tests/compile-fail/as_fallible.ori`
  - [ ] **LLVM Support**: LLVM codegen for as conversion enforcement
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/conversion_tests.rs` — as enforcement codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Float truncation methods (not `as`)
  - `float.truncate() -> int` — toward zero
  - `float.round() -> int` — nearest
  - `float.floor() -> int` — toward negative infinity
  - `float.ceil() -> int` — toward positive infinity
  - [ ] **Ori Tests**: `tests/spec/stdlib/float_methods.ori`
  - [ ] **LLVM Support**: LLVM codegen for float truncation methods
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/conversion_tests.rs` — float truncation codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Remove**: `int()`, `float()`, `str()`, `byte()` function syntax
  - These are replaced by `as`/`as?` syntax
  - No migration period needed if implementing fresh
  - [ ] **LLVM Support**: LLVM codegen removal of legacy conversion functions
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/conversion_tests.rs` — verify legacy functions removed
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **BUG-7A-05**: Un-comment and fix `tests/spec/expressions/type_conversion.ori` — restore test coverage for hardcoded cast dispatch
  - [ ] Verify all currently-working conversions (int-to-float, int-to-byte, int-to-char, char-to-int, byte-to-int, str-to-int/float via as?)
  - [ ] Fix or `#skip` any tests that depend on unimplemented features (str-to-bool)

- [ ] **Subsection close-out (7A.1)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-7A.1 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 7A.1: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 7A.2 Assertions

- [x] **Implement**: `assert(cond:)` [done] (2026-02-10) (verified 2026-03-29)
  - [x] **Ori Tests**: 310+ usages across 20+ spec test files, all types exercised
  - [ ] **LLVM Support**: LLVM codegen for assert -- works transitively (desugars to `if !cond then panic(...)` and panic has LLVM support)
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/assertion_tests.rs` (file does not exist)
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: `assert_eq(actual:, expected:)` [done] (2026-02-10) (verified 2026-03-29)
  - [x] **Ori Tests**: 78+ usages across int, str, bool, float, list, map, tuple, Option, Result, struct
  - [ ] **BUG-7A-02**: Spec says `assert_eq<T: Eq + Debug>` but implementation is `assert_eq<T: Eq>` -- uses `str(actual)` instead of `actual.debug()`, poor messages for non-Printable types
  - [ ] **WEAK TESTS**: No negative test for non-Eq types
  - [ ] **LLVM Support**: LLVM codegen for assert_eq -- works transitively through panic
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/assertion_tests.rs` (file does not exist)
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: `assert_ne(actual:, expected:)` [done] (2026-02-10) (verified 2026-03-29)
  - [x] **Ori Tests**: WEAK TESTS -- only 3 usages, only int and str tested
  - [ ] **WEAK TESTS**: Missing matrix coverage for bool, float, list, map, struct, Option
  - [ ] **LLVM Support**: LLVM codegen for assert_ne
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/assertion_tests.rs` (file does not exist)
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: `assert_some(x)` — spec/annex-c-built-in-functions.md § assert_some [done] (verified 2026-03-29)
  - [x] **Implementation**: `library/std/testing.ori` — implemented in Ori stdlib
  - [ ] **BUG-7A-04**: Spec says `assert_some<T>(option: Option<T>) -> T` (returns inner value) but implementation returns `void`
  - [ ] **NEEDS TESTS**: Zero spec tests use assert_some
  - [ ] **LLVM Support**: LLVM codegen for assert_some

- [x] **Implement**: `assert_none(x)` — spec/annex-c-built-in-functions.md § assert_none [done] (verified 2026-03-29)
  - [x] **Implementation**: `library/std/testing.ori` — implemented in Ori stdlib
  - [ ] **NEEDS TESTS**: Zero spec tests use assert_none
  - [ ] **LLVM Support**: LLVM codegen for assert_none

- [x] **Implement**: `assert_ok(x)` — spec/annex-c-built-in-functions.md § assert_ok [done] (verified 2026-03-29)
  - [x] **Implementation**: `library/std/testing.ori` — implemented in Ori stdlib
  - [ ] **BUG-7A-04**: Spec says `assert_ok<T, E>(result: Result<T, E>) -> T` but implementation returns `void`
  - [ ] **NEEDS TESTS**: Zero spec tests use assert_ok
  - [ ] **LLVM Support**: LLVM codegen for assert_ok

- [x] **Implement**: `assert_err(x)` — spec/annex-c-built-in-functions.md § assert_err [done] (verified 2026-03-29)
  - [x] **Implementation**: `library/std/testing.ori` — implemented in Ori stdlib
  - [ ] **BUG-7A-04**: Spec says `assert_err<T, E>(result: Result<T, E>) -> E` but implementation returns `void`
  - [ ] **NEEDS TESTS**: Zero spec tests use assert_err
  - [ ] **LLVM Support**: LLVM codegen for assert_err

- [ ] **MISSING ITEM**: `assert_panics(expr:)` — implemented in `library/std/testing.ori`, used in 4 test files, listed in spec annex-c but not tracked in roadmap (verified 2026-03-29)
  - [ ] **Ori Tests**: Used in `integer_safety.ori`, `operators_bitwise.ori`, `ranges.ori`, `range_edge_cases.ori`
  - [ ] **LLVM Support**: LLVM codegen for assert_panics

- [ ] **MISSING ITEM**: `assert_panics_with(expr:, message:)` — implemented in `library/std/testing.ori`, listed in spec annex-c but not tracked in roadmap (verified 2026-03-29)
  - [ ] **Ori Tests**: Needs dedicated spec tests
  - [ ] **LLVM Support**: LLVM codegen for assert_panics_with

- [ ] **Subsection close-out (7A.2)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-7A.2 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 7A.2: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 7A.3 I/O and Other

- [x] **Implement**: `print(x)` [done] (2026-02-10) (verified 2026-03-29)
  - [x] **Ori Tests**: Used pervasively in test suite
  - [x] **LLVM Support**: LLVM codegen for print -- `_ori_print` in runtime, `test_aot_hello_world` verifies
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/io_tests.rs` (file does not exist)
  - [ ] **AOT Tests**: No dedicated AOT coverage yet

- [x] **Implement**: `compare(a, b)` [done] (2026-02-10) (verified 2026-03-29)
  - [x] **Ori Tests**: 58 tests in `comparable.ori` for `.compare(other:)` method
  - [ ] **BUG-7A-03**: Spec says `compare<T: Comparable>(left: T, right: T) -> Ordering` (generic) but implementation is `compare(a: int, b: int)` (int-only) -- cannot `compare(left: "apple", right: "banana")`
  - [x] **LLVM Support**: LLVM codegen for compare -- works transitively as Ori function
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/comparison_tests.rs` (file does not exist)
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: `min(a, b)`, `max(a, b)` [done] (2026-02-10) (verified 2026-03-29)
  - [x] **Ori Tests**: Dedicated tests in `comparable.ori` with int values including equal values, negatives
  - [ ] **BUG-7A-03**: Spec says `min<T: Comparable>` / `max<T: Comparable>` (generic) but implementation is int-only
  - [ ] **LLVM Support**: LLVM codegen for min/max -- works transitively as Ori functions
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/comparison_tests.rs` (file does not exist)
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: `panic(msg)` [done] (2026-02-10) (verified 2026-03-29)
  - [x] **Ori Tests**: 20+ tests in `never.ori`, used in `#fail` test attributes
  - [x] **LLVM Support**: LLVM codegen for panic -- `_ori_panic`/`_ori_panic_cstr` in runtime, 9 dedicated AOT tests in `panic.rs`
  - [x] **AOT Tests**: 33 AOT panic tests pass (`cargo test -p ori_llvm -- panic`)
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/panic_tests.rs` (file does not exist)

- [ ] **MISSING ITEM**: `hash_combine(seed:, value:)` — implemented in `ori_eval/src/function_val.rs` (`function_val_hash_combine`), typed in `ori_types/src/infer/expr/identifiers.rs`, listed in prelude, but not tracked in roadmap (verified 2026-03-29)

- [ ] **MISSING ITEM**: `is_cancelled()` — listed in ori-syntax.md prelude as a builtin function, but not tracked in section 7A (verified 2026-03-29)

- [ ] **Subsection close-out (7A.3)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-7A.3 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 7A.3: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 7A.4 Float NaN Behavior

> **Decision**: NaN comparisons panic (no proposal needed — behavioral decision)
>
> Fits Ori's "bugs should be caught" philosophy (same as integer overflow).
>
> **Status** (verified 2026-03-29): NOT STARTED. Current behavior follows IEEE 754 (NaN == NaN is false,
> NaN != NaN is true, no panic). Tests in `operators_comparison.ori` assert IEEE 754 behavior.
> When 7A.4 is implemented, those existing tests will need to change.

- [ ] **Implement**: NaN comparison panics
  - `NaN == NaN` → PANIC
  - `NaN < x` → PANIC
  - `NaN > x` → PANIC
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/binary.rs` — NaN comparison tests
  - [ ] **Ori Tests**: `tests/spec/types/float_nan.ori`
  - [ ] **LLVM Support**: LLVM codegen for NaN comparison panic
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/float_tests.rs` — NaN comparison panic codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: NaN-producing operations don't panic (only comparisons)
  - `0.0 / 0.0` → NaN (allowed)
  - Using NaN in arithmetic → NaN (allowed)
  - Comparing NaN → PANIC
  - [ ] **Ori Tests**: `tests/spec/types/float_nan_ops.ori`
  - [ ] **LLVM Support**: LLVM codegen for NaN-producing operations
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/float_tests.rs` — NaN operations codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Subsection close-out (7A.4)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-7A.4 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 7A.4: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 7A.5 Developer Functions

> **PROPOSAL**: `proposals/approved/developer-functions-proposal.md`
>
> `todo`, `unreachable`, and `dbg` for developer convenience. These provide
> semantic meaning (unfinished vs. impossible code) and inline debugging.

- [x] **Implement**: `todo()` and `todo(reason:)` — Mark unfinished code [done] (verified 2026-03-29)
  - Returns `Never`, panics with "not yet implemented at file:line"
  - Location information captured at compile time
  - [x] **Rust Tests**: `FunctionExpKind::Todo` in `ori_eval/src/interpreter/can_eval/function_exp.rs`
  - [x] **Ori Tests**: `tests/spec/types/never.ori` lines 76-89 -- tests todo() and todo(reason:) as Never-producing expressions with coercion to int and str (semantic pin)
  - [ ] **LLVM Support**: LLVM codegen for todo
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/developer_tests.rs` — todo codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: `unreachable()` and `unreachable(reason:)` — Mark impossible code [done] (verified 2026-03-29)
  - Returns `Never`, panics with "unreachable code reached at file:line"
  - Semantically distinct from `todo` (impossible vs. not done)
  - [x] **Rust Tests**: `FunctionExpKind::Unreachable` in `ori_eval/src/interpreter/can_eval/function_exp.rs`
  - [x] **Ori Tests**: `tests/spec/types/never.ori` lines 92-105 -- tests both forms (semantic pin via coercion tests)
  - [ ] **LLVM Support**: LLVM codegen for unreachable
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/developer_tests.rs` — unreachable codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `dbg(value:)` and `dbg(value:, label:)` — Debug printing
  - Generic: `dbg<T: Debug>(value: T) -> T`
  - Writes to stderr via Print capability
  - Output format: `[file:line] = value` or `[file:line] label = value`
  - Returns value unchanged for inline use
  - [ ] **Rust Tests**: `ori_eval/src/function_val.rs` — dbg tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/developer_functions.ori`
  - [ ] **LLVM Support**: LLVM codegen for dbg
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/developer_tests.rs` — dbg codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Compile-time location capture for all three functions
  - Location passed implicitly by compiler, not visible in user signature
  - [ ] **Rust Tests**: `ori_types/src/infer/expr/identifiers.rs` — location capture tests
  - [ ] **Ori Tests**: Verify location appears in panic messages/dbg output

- [ ] **Subsection close-out (7A.5)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-7A.5 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 7A.5: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 7A.6 Additional Built-in Functions

**Proposal**: `proposals/approved/additional-builtins-proposal.md`

Formalizes `repeat`, `compile_error`, `PanicInfo`, and clarifies `??` operator semantics.

### repeat Function

- [x] **Implement**: `repeat<T: Clone>(value: T) -> impl Iterator` — infinite iterator of cloned values [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `function_val_repeat` in `ori_eval/src/function_val.rs`, type signature in `ori_types/src/infer/expr/identifiers.rs`
  - [x] **Ori Tests**: `tests/spec/traits/iterator/infinite.ori` -- 14 tests covering repeat with int, str, bool, take(0), map, filter, enumerate, skip, count, fold, any, all, zip
  - [ ] **LLVM Support**: LLVM codegen for repeat
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/iterator_tests.rs` — repeat codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Clone requirement enforcement — T must implement Clone
  - [ ] **Rust Tests**: `ori_types/src/infer/expr/identifiers.rs` — repeat type checking
  - [ ] **Ori Tests**: `tests/compile-fail/repeat_not_clone.ori`

- [ ] **Implement**: Integration with Iterator trait — .take(), .collect(), etc.
  - [ ] **Ori Tests**: `tests/spec/stdlib/repeat_iterator.ori`

### PanicInfo Type

**Proposal**: `proposals/approved/panic-handler-proposal.md` (extends basic definition)

**Spec**: `spec/17-errors-and-panics.md` § PanicInfo Type (updated with full structure)

- [ ] **Spec**: PanicInfo type definition — `{ message, location, stack_trace, thread_id }` DONE

- [ ] **Implement**: `PanicInfo` struct type — `{ message: str, location: TraceEntry, stack_trace: [TraceEntry], thread_id: Option<int> }` (verified 2026-03-29: partially implemented in LLVM IR in `panic_trampoline.rs`, AOT tests define it locally; no interpreter/typeck/prelude registration)
  - [ ] **Rust Tests**: `ori_types/src/check/registration/mod.rs` — PanicInfo type tests
  - [ ] **Ori Tests**: `tests/spec/types/panic_info.ori`
  - [ ] **LLVM Support**: LLVM codegen for PanicInfo
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/type_tests.rs` — PanicInfo codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `Printable` impl for PanicInfo
  - [ ] **Ori Tests**: `tests/spec/types/panic_info_printable.ori`

- [ ] **Implement**: `Debug` impl for PanicInfo
  - [ ] **Ori Tests**: `tests/spec/types/panic_info_debug.ori`

- [ ] **Add to prelude**: PanicInfo available without import
  - [ ] **Ori Tests**: `tests/spec/prelude/panic_info.ori`

### @panic Handler

**Proposal**: `proposals/approved/panic-handler-proposal.md`

App-wide panic handler function that executes before program termination.

- [x] **Implement**: Recognize `@panic` as special function (like `@main`) [done] (verified 2026-03-29)
  - [x] **LLVM Support**: Implemented in `panic_trampoline.rs` and `entry_point.rs`
  - [x] **AOT Tests**: 9 dedicated panic tests in `compiler/ori_llvm/tests/aot/panic.rs` (33 total AOT panic tests pass)
  - [ ] **Rust Tests**: `ori_types/src/check/special_functions.rs` — @panic recognition
  - [ ] **Ori Tests**: `tests/spec/declarations/panic_handler.ori`
  - [ ] **Interpreter Support**: Not implemented -- GAP: LLVM-only, breaks dual-execution parity

- [x] **Implement**: Validate signature `(PanicInfo) -> void` [done] (verified 2026-03-29)
  - [x] **LLVM Support**: Signature validation in panic_trampoline.rs
  - [ ] **Rust Tests**: `ori_types/src/check/special_fns.rs` — @panic signature validation
  - [ ] **Ori Tests**: `tests/compile-fail/panic_handler_wrong_sig.ori`

- [ ] **Implement**: Error if multiple `@panic` definitions
  - [ ] **Rust Tests**: `ori_types/src/check/special_functions.rs` — multiple @panic error
  - [ ] **Ori Tests**: `tests/compile-fail/multiple_panic_handlers.ori`

- [ ] **Implement**: Implicit stderr for print() inside @panic
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/panic_handler.rs` — stderr redirection
  - [ ] **Ori Tests**: `tests/spec/declarations/panic_print_stderr.ori`
  - [ ] **LLVM Support**: LLVM codegen for stderr redirection in @panic
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/panic_tests.rs` — stderr redirection codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: Runtime panic hook installation at program start [done] (verified 2026-03-29)
  - [x] **LLVM Support**: `ori_register_panic_handler` in `ori_rt`
  - [x] **AOT Tests**: Tested in `compiler/ori_llvm/tests/aot/panic.rs`
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/panic.rs` — hook installation
  - [ ] **Interpreter Support**: Not implemented -- GAP: LLVM-only, breaks dual-execution parity

- [ ] **Implement**: Construct PanicInfo (message, location, stack_trace, thread_id) on panic
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/panic.rs` — PanicInfo construction
  - [ ] **Ori Tests**: `tests/spec/runtime/panic_info_construction.ori`
  - [ ] **LLVM Support**: LLVM codegen for PanicInfo construction
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/panic_tests.rs` — PanicInfo construction codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: Re-panic detection — immediate termination if handler panics [done] (verified 2026-03-29)
  - [x] **LLVM Support**: Implemented in panic_trampoline.rs
  - [x] **AOT Tests**: `test_panic_handler_re_entrancy` in `compiler/ori_llvm/tests/aot/panic.rs`
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/panic.rs` — re-panic detection
  - [ ] **Ori Tests**: `tests/spec/runtime/panic_in_handler.ori`
  - [ ] **Interpreter Support**: Not implemented -- GAP: LLVM-only, breaks dual-execution parity

- [ ] **Implement**: First panic wins in concurrent context
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/panic.rs` — concurrent panic handling
  - [ ] **Ori Tests**: `tests/spec/runtime/concurrent_panic.ori`
  - [ ] **LLVM Support**: LLVM codegen for concurrent panic handling
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/panic_tests.rs` — concurrent panic codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: Default handler (when no @panic defined) — print to stderr [done] (verified 2026-03-29)
  - [x] **LLVM Support**: Implemented -- `test_panic_default_nonzero_exit` in `compiler/ori_llvm/tests/aot/panic.rs`
  - [x] **AOT Tests**: Tested in `compiler/ori_llvm/tests/aot/panic.rs`
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/panic.rs` — default handler
  - [ ] **Ori Tests**: `tests/spec/runtime/default_panic_handler.ori`
  - [ ] **Interpreter Support**: Not implemented -- GAP: LLVM-only, breaks dual-execution parity

- [x] **Implement**: Exit with non-zero code after handler returns [done] (verified 2026-03-29)
  - [x] **LLVM Support**: Implemented and tested
  - [x] **AOT Tests**: `test_panic_default_nonzero_exit` in `compiler/ori_llvm/tests/aot/panic.rs`
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/panic.rs` — exit code
  - [ ] **Interpreter Support**: Not implemented -- GAP: LLVM-only, breaks dual-execution parity

### compile_error Function

- [ ] **MISSING ITEM**: `compile_error(msg:)` — listed in spec annex-c and ori-syntax.md prelude but not previously tracked in roadmap (verified 2026-03-29)
  - [ ] **Implement**: Compile-time error emission -- stops compilation with user message
  - [ ] **Rust Tests**: `ori_types/src/infer/expr/identifiers.rs` — compile_error type checking
  - [ ] **Ori Tests**: `tests/compile-fail/compile_error.ori`
  - [ ] **LLVM Support**: No codegen needed (compile-time only)

- [ ] **Subsection close-out (7A.6)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-7A.6 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 7A.6: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 7A.7 Resource Management

**Proposal**: `proposals/approved/drop-trait-proposal.md`

Adds `drop_early` function for explicit early resource release.

### drop_early Function

- [ ] **Implement**: `drop_early<T>(value: T) -> void` — Force drop before scope exit
  - [ ] **Rust Tests**: `ori_eval/src/function_val.rs` — drop_early tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/drop_early.ori`
  - [ ] **LLVM Support**: LLVM codegen for drop_early
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/drop_tests.rs` — drop_early codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: drop_early works for any type (not restricted to T: Drop)
  - Types with Drop: drop method called, then memory reclaimed
  - Types without Drop: memory reclaimed immediately
  - [ ] **Ori Tests**: `tests/spec/stdlib/drop_early_any_type.ori`

- [ ] **Add to prelude**: drop_early available without import
  - [ ] **Ori Tests**: `tests/spec/prelude/drop_early.ori`

- [ ] **Update Spec**: `spec/annex-c-built-in-functions.md` — add drop_early documentation
  - [ ] Signature: `drop_early<T>(value: T) -> void`
  - [ ] Semantics: Takes ownership, value is dropped immediately
  - [ ] Use case: Release resources before scope exit

- [ ] **Subsection close-out (7A.7)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-7A.7 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 7A.7: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 7A.8 Compile-Time File Embedding

> **PROPOSAL**: `proposals/approved/embed-expression-proposal.md`
>
> `embed` and `has_embed` built-in expressions for compile-time file embedding.
> Type-driven: `str` (UTF-8 validated) or `[byte]` (raw binary) based on expected type.
> Paths are const-evaluable expressions, relative to source file, restricted to project root.

- [ ] **Implement**: `embed(path)` — Compile-time file embedding
  - Context-sensitive keyword, parsed as `EmbedExpr` node
  - Type-driven: `str` → UTF-8 read + validation, `[byte]` → raw bytes
  - Path must be const-evaluable `str` (supports interpolation, const functions)
  - Path resolution relative to source file, no absolute paths, no project escape
  - [ ] **Rust Tests**: `ori_types/src/infer/expr/embed.rs` — type inference for embed
  - [ ] **Ori Tests**: `tests/spec/embed/embed_str.ori`, `tests/spec/embed/embed_bytes.ori`
  - [ ] **LLVM Support**: Emit embedded data in `.rodata` section
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/embed_tests.rs` — embed codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `has_embed(path)` — Compile-time file existence check
  - Returns compile-time `bool`
  - Same path resolution rules as `embed`
  - [ ] **Rust Tests**: `ori_types/src/infer/expr/embed.rs` — has_embed type checking
  - [ ] **Ori Tests**: `tests/spec/embed/has_embed.ori`
  - [ ] **LLVM Support**: LLVM codegen for has_embed
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/embed_tests.rs` — has_embed codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: File size limit enforcement (10 MB default)
  - `#embed_limit(size:)` attribute for per-expression override
  - `ori.toml` `[embed] max_file_size` for project-wide override
  - [ ] **Ori Tests**: `tests/compile-fail/embed_size_limit.ori`

- [ ] **Implement**: File dependency tracking in Salsa
  - Hash-based invalidation (content hash, not mtime)
  - Embedded file changes trigger recompilation
  - `has_embed` file existence changes trigger recompilation
  - [ ] **Rust Tests**: `oric/src/query/embed.rs` — dependency tracking

- [ ] **Implement**: `embed`/`has_embed` error diagnostics — add error codes to `ori_diagnostic` for file-not-found, invalid path, bad UTF-8
  - File not found (with "did you mean?" suggestions)
  - Absolute path error
  - Path escapes project root error
  - Invalid UTF-8 error (when `str` expected)
  - Cannot infer embed type error
  - File exceeds size limit error
  - [ ] **Ori Tests**: `tests/compile-fail/embed_errors.ori`

- [ ] **Implement**: Binary deduplication — multiple references to same file share one copy
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/embed_tests.rs` — deduplication
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Subsection close-out (7A.8)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-7A.8 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 7A.8: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 7A.9 Char and Byte Classification Methods

**Proposal**: `proposals/approved/char-byte-classification-proposal.md`

Standard classification methods on `char` and `byte` types for character category testing. `char` methods are Unicode-aware; `byte` methods are ASCII-only with short aliases.

### Char Unicode Methods

- [x] **Implement**: `char.is_alphabetic()` (as `is_alpha`) — Unicode `L*` categories [done] (verified 2026-03-29)
  - [x] **Registry**: `compiler/ori_registry/src/defs/char.rs`
  - [x] **Evaluator**: `compiler/ori_eval/src/methods/variants.rs`
  - [ ] **NEEDS TESTS**: Zero spec tests -- needs `tests/spec/types/char_classification.ori`
  - [ ] **Note**: Registered as `is_alpha`, not `is_alphabetic` per spec -- naming gap
- [x] **Implement**: `char.is_digit()` — Unicode `Nd` category [done] (verified 2026-03-29) -- NEEDS TESTS: zero spec tests
- [ ] **Implement**: `char.is_alphanumeric()` — `L*` or `Nd`
- [x] **Implement**: `char.is_whitespace()` — `Zs` + control whitespace [done] (verified 2026-03-29) -- NEEDS TESTS: zero spec tests
- [x] **Implement**: `char.is_uppercase()` — `Lu` [done] (verified 2026-03-29) -- NEEDS TESTS: zero spec tests
- [x] **Implement**: `char.is_lowercase()` — `Ll` [done] (verified 2026-03-29) -- NEEDS TESTS: zero spec tests
- [x] **Implement**: `char.is_ascii()` — U+0000..U+007F [done] (verified 2026-03-29) -- NEEDS TESTS: zero spec tests
- [ ] **Implement**: `char.is_control()` — `Cc`
- [ ] **Implement**: Unicode lookup tables from UCD — compressed tables for L*, Nd, Zs, Lu, Ll categories

### Char ASCII Methods

- [ ] **Implement**: `char.is_ascii_alphabetic()` — `a-z`, `A-Z`
  - [ ] **Rust Tests**: `ori_eval/src/methods/char/tests.rs` — ASCII classification
  - [ ] **Ori Tests**: `tests/spec/types/char_ascii_classification.ori`
- [ ] **Implement**: `char.is_ascii_digit()` — `0-9`
- [ ] **Implement**: `char.is_ascii_alphanumeric()` — `a-z`, `A-Z`, `0-9`
- [ ] **Implement**: `char.is_ascii_whitespace()` — space, tab, newline, CR, VT, FF
- [ ] **Implement**: `char.is_ascii_uppercase()` — `A-Z`
- [ ] **Implement**: `char.is_ascii_lowercase()` — `a-z`
- [ ] **Implement**: `char.is_ascii_hex_digit()` — `0-9`, `a-f`, `A-F`
- [ ] **Implement**: `char.is_ascii_punctuation()` — ASCII punctuation ranges
- [ ] **Implement**: `char.is_ascii_control()` — 0x00..0x1F, 0x7F

### Byte Methods (Full + Short Aliases)

- [x] **Implement**: `byte.is_ascii()`, `byte.is_ascii_alpha()`, `byte.is_ascii_digit()`, `byte.is_ascii_whitespace()` [done] (verified 2026-03-29)
  - [x] **Registry**: `compiler/ori_registry/src/defs/byte.rs` — 4 methods registered
  - [x] **Evaluator**: `compiler/ori_eval/src/methods/variants.rs` — 4 methods implemented
  - [ ] **NEEDS TESTS**: Zero spec tests for all 4 methods -- needs `tests/spec/types/byte_classification.ori`
- [ ] **Implement**: Remaining byte methods + 7 short aliases — 6+ full methods not yet implemented
- [ ] **LLVM Support**: LLVM codegen for byte/char classification methods
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/aot/primitives.rs` — byte/char classification codegen
  - [ ] **AOT Tests**: No AOT coverage yet

### Conversion Methods

- [ ] **Implement**: `char.to_ascii_uppercase()`, `char.to_ascii_lowercase()` — returns self if not ASCII letter
  - [ ] **Ori Tests**: `tests/spec/types/char_conversion.ori`
- [ ] **Implement**: `byte.to_ascii_uppercase()`, `byte.to_ascii_lowercase()` — returns self if not ASCII letter
  - [ ] **Ori Tests**: `tests/spec/types/byte_conversion.ori`
- [ ] **Implement**: `char.to_digit(radix:)`, `byte.to_digit(radix:)` — returns `Option<int>`; radix 2..=36, panic on invalid
  - [ ] **Ori Tests**: `tests/spec/types/char_to_digit.ori`

- [ ] **Subsection close-out (7A.9)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-7A.9 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 7A.9: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 7A.10 Byte-Level String Access

**Proposal**: `proposals/approved/byte-string-access-proposal.md`

Methods for accessing raw UTF-8 bytes of a `str` value, enabling O(1) byte-level indexing for lexers and parsers.

### str Methods

- [x] **Implement**: `str.as_bytes()` — zero-copy `[byte]` view via seamless slicing [done] (verified 2026-03-29)
  - [x] **Registry**: `compiler/ori_registry/src/defs/str.rs`
  - [x] **Evaluator**: `compiler/ori_eval/src/methods/collections.rs` — converts str to `[byte]`
  - [ ] **NEEDS TESTS**: Zero dedicated spec tests -- needs `tests/spec/types/str_as_bytes.ori`
  - [ ] **LLVM Support**: Seamless slicing behavior not implemented in LLVM
- [x] **Implement**: `str.to_bytes()` — owned `[byte]` copy [done] (verified 2026-03-29)
  - [x] **Evaluator**: `compiler/ori_eval/src/methods/collections.rs`
  - [ ] **NEEDS TESTS**: Zero dedicated spec tests -- needs `tests/spec/types/str_to_bytes.ori`
- [x] **Implement**: `str.byte_len()` — O(1) UTF-8 byte count [done] (verified 2026-03-29)
  - [x] **Evaluator**: `compiler/ori_eval/src/methods/collections.rs`
  - [ ] **NEEDS TESTS**: Only 1 indirect test in `primitives.ori` -- needs dedicated `tests/spec/types/str_byte_len.ori`
- [ ] **Implement**: Flatten behavior — `as_bytes()` on substring seamless slice produces single-level `[byte]` view

### str Associated Functions

- [x] **Implement**: `str.from_utf8(bytes:)` — validate UTF-8, return `Result<str, Error>` [done] (verified 2026-03-29)
  - [x] **Registry**: `compiler/ori_registry/src/defs/str.rs`
  - [x] **Evaluator**: `compiler/ori_eval/src/methods/collections.rs`
  - [ ] **NEEDS TESTS**: Zero dedicated spec tests -- needs `tests/spec/types/str_from_utf8.ori`
- [x] **Implement**: `str.from_utf8_unchecked(bytes:)` — skip validation, requires `unsafe` [done] (verified 2026-03-29)
  - [x] **Registry**: `compiler/ori_registry/src/defs/str.rs`
  - [x] **Evaluator**: `compiler/ori_eval/src/methods/collections.rs`
  - [ ] **NEEDS TESTS**: Zero dedicated spec tests -- needs `tests/spec/types/str_from_utf8_unchecked.ori`

### LLVM Support

- [ ] **LLVM Support**: Codegen for `as_bytes`, `to_bytes`, `byte_len`, `from_utf8`, `from_utf8_unchecked`
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/aot/strings.rs` — byte access codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Subsection close-out (7A.10)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-7A.10 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 7A.10: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 7A.11 Section Completion Checklist

- [ ] All items above have all checkboxes marked `[ ]`
- [ ] Re-evaluate against docs/compiler-design/v2/02-design-principles.md
- [ ] 80+% test coverage, tests against spec/design
- [ ] Run full test suite: `./test-all.sh`
- [ ] **LLVM Support**: All LLVM codegen tests pass
- [ ] `/tpr-review` passed — independent Codex review found no critical or major issues (or all findings triaged)
- [ ] `/impl-hygiene-review` passed — implementation hygiene review clean (phase boundaries, SSOT, algorithmic DRY, naming). MUST run AFTER `/tpr-review` is clean.
- [ ] `/improve-tooling` retrospective completed — MANDATORY at section close, after both reviews are clean. Reflect on the section's debugging journey (which `diagnostics/` scripts you ran, which command sequences you repeated, where you added ad-hoc `dbg!`/`tracing` calls, where output was hard to interpret) and identify any tool/log/diagnostic improvement that would have made this section materially easier OR that would help the next section touching this area. Implement every accepted improvement NOW (zero deferral) and commit each via SEPARATE `/commit-push`. The retrospective is mandatory even when nothing felt painful — that is exactly when blind spots accumulate. See `.claude/skills/improve-tooling/SKILL.md` "Retrospective Mode" for the full protocol.

**Exit Criteria**: Core built-in functions working correctly

- [ ] **Subsection close-out (7A.11)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-7A.11 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 7A.11: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

