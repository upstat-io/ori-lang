---
section: 7A
title: Core Built-ins
status: in-progress
reviewed: false
tier: 2
goal: Type conversions, assertions, I/O, and core built-in functions
spec:
  - spec/annex-c-built-in-functions.md
sections:
  - id: "7A.1"
    title: Type Conversions
    status: not-started
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
    status: in-progress
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

---

## 7A.2 Assertions

- [x] **Implement**: `assert(cond:)` [done] (2026-02-10)
  - [x] **Ori Tests**: Used in hundreds of tests across test suite (`assert(cond: ...)`)
  - [x] **LLVM Support**: `ori_assert` runtime function in `ori_rt`, declared in `runtime_functions.rs`, mapped in JIT `runtime_mappings.rs`
  - [x] **AOT Tests**: `ori_llvm/tests/aot/` — `test_assert_false_panics` and related assert tests (10 tests pass)

- [x] **Implement**: `assert_eq(actual:, expected:)` [done] (2026-02-10)
  - [x] **Ori Tests**: Used in hundreds of tests across test suite
  - [x] **LLVM Support**: `ori_assert_eq_int`, `ori_assert_eq_bool`, `ori_assert_eq_float`, `ori_assert_eq_str` runtime functions
  - [x] **AOT Tests**: `test_assert_eq_int_mismatch_panics`, `test_assert_eq_bool_mismatch_panics`, `test_assert_eq_str_mismatch_panics`

- [x] **Implement**: `assert_ne(actual:, expected:)` [done] (2026-02-10)
  - [x] **Ori Tests**: Used in 20+ spec test files (`duration_size_hashable.ori`, `eq.ori`, `data.ori`, `try.ori`, etc.)
  - [x] **LLVM Support**: Desugars to `if actual == unexpected then panic(...)` — uses existing LLVM infrastructure
  - [ ] **AOT Tests**: No dedicated AOT tests

- [x] **Implement**: `assert_some(x)` — spec/annex-c-built-in-functions.md § assert_some [done]
  - Defined in `library/std/testing.ori`
  - [ ] **Ori Tests**: NEEDS TESTS — zero spec test files use assert_some
  - [ ] **LLVM Support**: LLVM codegen for assert_some

- [x] **Implement**: `assert_none(x)` — spec/annex-c-built-in-functions.md § assert_none [done]
  - Defined in `library/std/testing.ori`
  - [ ] **Ori Tests**: NEEDS TESTS — zero spec test files use assert_none
  - [ ] **LLVM Support**: LLVM codegen for assert_none

- [x] **Implement**: `assert_ok(x)` — spec/annex-c-built-in-functions.md § assert_ok [done]
  - Defined in `library/std/testing.ori`
  - [ ] **Ori Tests**: NEEDS TESTS — zero spec test files use assert_ok
  - [ ] **LLVM Support**: LLVM codegen for assert_ok

- [x] **Implement**: `assert_err(x)` — spec/annex-c-built-in-functions.md § assert_err [done]
  - Defined in `library/std/testing.ori`
  - [ ] **Ori Tests**: NEEDS TESTS — zero spec test files use assert_err
  - [ ] **LLVM Support**: LLVM codegen for assert_err

---

## 7A.3 I/O and Other

- [x] **Implement**: `print(x)` [done] (2026-02-10)
  - [x] **Ori Tests**: Used in test suite; LLVM has `_ori_print` runtime function
  - [x] **LLVM Support**: LLVM codegen for print — `_ori_print` in runtime
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/io_tests.rs` (file does not exist)
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: `compare(a, b)` [done] (2026-02-10)
  - [x] **Ori Tests**: `tests/spec/traits/core/comparable.ori` — 58 tests for `.compare(other:)`
  - [x] **LLVM Support**: LLVM codegen for compare — inline IR in lower_calls.rs
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/comparison_tests.rs` (file does not exist)
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: `min(a, b)`, `max(a, b)` [done] (2026-02-10)
  - [x] **Ori Tests**: Prelude functions available, verified in Section 4.6
  - [ ] **LLVM Support**: LLVM codegen for min/max
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/comparison_tests.rs` (file does not exist)
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: `panic(msg)` [done] (2026-02-10)
  - [x] **Ori Tests**: Used in `#fail` test attributes (division by zero, index out of bounds)
  - [x] **LLVM Support**: LLVM codegen for panic — `_ori_panic` in runtime
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/panic_tests.rs` (file does not exist)
  - [ ] **AOT Tests**: No AOT coverage yet

---

## 7A.4 Float NaN Behavior

> **Decision**: NaN comparisons panic (no proposal needed — behavioral decision)
>
> Fits Ori's "bugs should be caught" philosophy (same as integer overflow).

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

---

## 7A.5 Developer Functions

> **PROPOSAL**: `proposals/approved/developer-functions-proposal.md`
>
> `todo`, `unreachable`, and `dbg` for developer convenience. These provide
> semantic meaning (unfinished vs. impossible code) and inline debugging.

- [x] **Implement**: `todo()` and `todo(reason:)` — Mark unfinished code [done]
  - `FunctionExpKind::Todo` in parser/IR, returns `Never` in type checker, evaluator produces `EvalError("not yet implemented")`
  - [ ] **Ori Tests**: NEEDS TESTS — no dedicated spec tests for todo
  - [ ] **LLVM Support**: LLVM codegen for todo
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: `unreachable()` and `unreachable(reason:)` — Mark impossible code [done]
  - `FunctionExpKind::Unreachable` in parser/IR, returns `Never` in type checker, evaluator produces `EvalError("reached unreachable code")`
  - Semantically distinct from `todo` (impossible vs. not done)
  - [ ] **Ori Tests**: NEEDS TESTS — no dedicated spec tests for unreachable
  - [ ] **LLVM Support**: LLVM codegen for unreachable
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

---

## 7A.6 Additional Built-in Functions

**Proposal**: `proposals/approved/additional-builtins-proposal.md`

Formalizes `repeat`, `compile_error`, `PanicInfo`, and clarifies `??` operator semantics.

### repeat Function

- [x] **Implement**: `repeat<T: Clone>(value: T) -> impl Iterator` — infinite iterator of cloned values [done]
  - `IteratorValue::Repeat` variant in `ori_patterns`, `function_val_repeat` in evaluator, type signature in `identifiers.rs`
  - [x] **Ori Tests**: 14+ dedicated tests in `tests/spec/traits/iterator/infinite.ori`
  - [ ] **LLVM Support**: LLVM codegen for repeat (works through standard iterator codegen)
  - [ ] **AOT Tests**: No dedicated AOT coverage yet

- [ ] **Implement**: Clone requirement enforcement — T must implement Clone
  - [ ] **Rust Tests**: `ori_types/src/infer/expr/identifiers.rs` — repeat type checking
  - [ ] **Ori Tests**: `tests/compile-fail/repeat_not_clone.ori`

- [x] **Implement**: Integration with Iterator trait — .take(), .collect(), etc. [done]
  - Works through standard iterator pipeline (take, collect, etc.)
  - [x] **Ori Tests**: `tests/spec/traits/iterator/infinite.ori` — tests repeat with take/collect

### PanicInfo Type

**Proposal**: `proposals/approved/panic-handler-proposal.md` (extends basic definition)

**Spec**: `spec/17-errors-and-panics.md` § PanicInfo Type (updated with full structure)

- [ ] **Spec**: PanicInfo type definition — `{ message, location, stack_trace, thread_id }` DONE

- [ ] **Implement**: `PanicInfo` struct type — `{ message: str, location: TraceEntry, stack_trace: [TraceEntry], thread_id: Option<int> }`
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

- [x] **Implement**: Recognize `@panic` as special function (like `@main`) [done — LLVM/AOT only]
  - [ ] **Evaluator**: Not implemented in interpreter
  - [x] **LLVM Support**: `entry_point.rs` recognizes `@panic` and generates handler infrastructure
  - [x] **AOT Tests**: 9 passing AOT tests in `ori_llvm/tests/aot/` cover @panic recognition, invocation, re-panic, default handler, exit codes

- [x] **Implement**: Validate signature `(PanicInfo) -> void` [done — LLVM/AOT only]
  - [x] **LLVM Support**: Signature validated during entry point codegen

- [x] **Implement**: Implicit stderr for print() inside @panic [done — LLVM/AOT only]
  - [ ] **Evaluator**: Not implemented in interpreter
  - [x] **LLVM Support**: print() redirects to stderr inside @panic handler

- [x] **Implement**: Runtime panic hook installation at program start [done — LLVM/AOT only]
  - [ ] **Evaluator**: Not implemented in interpreter
  - [x] **LLVM Support**: Panic hook installed at program start in entry point codegen

- [x] **Implement**: Construct PanicInfo (message, location, stack_trace, thread_id) on panic [done — LLVM/AOT only]
  - [ ] **Evaluator**: Not implemented in interpreter
  - [x] **LLVM Support**: `entry_point.rs` constructs PanicInfo fields, trampoline bridges to user handler

- [x] **Implement**: Re-panic detection — immediate termination if handler panics [done — LLVM/AOT only]
  - [ ] **Evaluator**: Not implemented in interpreter
  - [x] **LLVM Support**: Re-entrancy protection implemented
  - [x] **AOT Tests**: Covered in @panic AOT test suite

- [ ] **Implement**: First panic wins in concurrent context
  - [ ] **Evaluator**: Not implemented
  - [ ] **LLVM Support**: Not implemented for concurrent context
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: Default handler (when no @panic defined) — print to stderr [done — LLVM/AOT only]
  - [ ] **Evaluator**: Not implemented in interpreter
  - [x] **LLVM Support**: Default handler prints to stderr when no @panic defined
  - [x] **AOT Tests**: Covered in @panic AOT test suite

- [x] **Implement**: Exit with non-zero code after handler returns [done — LLVM/AOT only]
  - [ ] **Evaluator**: Not implemented in interpreter
  - [x] **LLVM Support**: Non-zero exit code after handler
  - [x] **AOT Tests**: Covered in @panic AOT test suite

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

---

## 7A.9 Char and Byte Classification Methods

**Proposal**: `proposals/approved/char-byte-classification-proposal.md`

Standard classification methods on `char` and `byte` types for character category testing. `char` methods are Unicode-aware; `byte` methods are ASCII-only with short aliases.

### Char Unicode Methods

- [x] **Implement**: `char.is_alphabetic()` — Unicode `L*` categories [done]
  - Implemented in evaluator (`methods/variants.rs`) via Rust's Unicode-aware `char::is_alphabetic()`
  - [ ] **Ori Tests**: NEEDS TESTS — no dedicated spec tests
- [x] **Implement**: `char.is_digit()` — Unicode `Nd` category [done — BUG FOUND: uses `is_ascii_digit()` (ASCII-only 0-9), spec says Unicode Nd category]
  - [ ] **Ori Tests**: NEEDS TESTS — no dedicated spec tests
- [ ] **Implement**: `char.is_alphanumeric()` — `L*` or `Nd`
- [x] **Implement**: `char.is_whitespace()` — `Zs` + control whitespace [done]
  - Implemented via Rust's Unicode-aware `char::is_whitespace()`
  - [ ] **Ori Tests**: NEEDS TESTS — no dedicated spec tests
- [x] **Implement**: `char.is_uppercase()` — `Lu` [done]
  - Implemented via Rust's Unicode-aware `char::is_uppercase()`
  - [ ] **Ori Tests**: NEEDS TESTS — no dedicated spec tests
- [x] **Implement**: `char.is_lowercase()` — `Ll` [done]
  - Implemented via Rust's Unicode-aware `char::is_lowercase()`
  - [ ] **Ori Tests**: NEEDS TESTS — no dedicated spec tests
- [x] **Implement**: `char.is_ascii()` — U+0000..U+007F [done]
  - [ ] **Ori Tests**: NEEDS TESTS — no dedicated spec tests
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

- [x] **Implement** (partial): `byte.is_ascii()`, `byte.is_ascii_alpha()` / `byte.is_alpha()`, `byte.is_ascii_digit()` / `byte.is_digit()`, `byte.is_ascii_whitespace()`
  - BUG FOUND: `byte.is_ascii()` always returns `true` (hardcoded `Ok(Value::Bool(true))` in `variants.rs`). Should check `b <= 127` — bytes 128-255 are NOT ASCII.
  - `is_ascii_alpha`/`is_alpha`, `is_ascii_digit`/`is_digit`, `is_ascii_whitespace` — implemented and working
  - [ ] **Ori Tests**: NEEDS TESTS — no dedicated spec tests
- [ ] **Implement**: Remaining byte methods — `is_ascii_alphanumeric`/`is_alnum`, `is_ascii_uppercase`/`is_upper`, `is_ascii_lowercase`/`is_lower`, `is_ascii_hex_digit`/`is_hex_digit`, `is_ascii_punctuation`, `is_ascii_control`
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

---

## 7A.10 Byte-Level String Access

**Proposal**: `proposals/approved/byte-string-access-proposal.md`

Methods for accessing raw UTF-8 bytes of a `str` value, enabling O(1) byte-level indexing for lexers and parsers.

### str Methods

- [x] **Implement**: `str.as_bytes()` — `[byte]` view [done — evaluator copies bytes; seamless slice not yet used]
  - Implemented in evaluator (`methods/collections.rs`), registered in `ori_registry/src/defs/str.rs`
  - [ ] **Ori Tests**: NEEDS TESTS — no dedicated spec tests
  - [ ] Seamless slice zero-copy behavior not yet implemented
- [x] **Implement**: `str.to_bytes()` — owned `[byte]` copy [done]
  - Same implementation as `as_bytes` in evaluator (alias)
  - [ ] **Ori Tests**: NEEDS TESTS — no dedicated spec tests
- [x] **Implement**: `str.byte_len()` — O(1) UTF-8 byte count [done]
  - Implemented in evaluator, returns UTF-8 byte count
  - [ ] **Ori Tests**: NEEDS TESTS — no dedicated spec tests
- [ ] **Implement**: Flatten behavior — `as_bytes()` on substring seamless slice produces single-level `[byte]` view

### str Associated Functions

- [x] **Implement**: `str.from_utf8(bytes:)` — validate UTF-8, return `Result<str, Error>` [done]
  - Implemented in evaluator, validates UTF-8
  - [ ] **Ori Tests**: NEEDS TESTS — no dedicated spec tests
- [x] **Implement**: `str.from_utf8_unchecked(bytes:)` — skip validation [done]
  - Implemented in evaluator (validates anyway for safety in interpreter)
  - [ ] **Ori Tests**: NEEDS TESTS — no dedicated spec tests

### LLVM Support

- [ ] **LLVM Support**: Codegen for `as_bytes`, `to_bytes`, `byte_len`, `from_utf8`, `from_utf8_unchecked`
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/aot/strings.rs` — byte access codegen
  - [ ] **AOT Tests**: No AOT coverage yet

---

## 7A.11 Section Completion Checklist

- [ ] All items above have all checkboxes marked `[ ]`
- [ ] Re-evaluate against docs/compiler-design/v2/02-design-principles.md
- [ ] 80+% test coverage, tests against spec/design
- [ ] Run full test suite: `./test-all.sh`
- [ ] **LLVM Support**: All LLVM codegen tests pass

**Exit Criteria**: Core built-in functions working correctly
