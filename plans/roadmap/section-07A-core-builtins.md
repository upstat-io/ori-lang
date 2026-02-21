---
section: 7A
title: Core Built-ins
status: in-progress
tier: 2
goal: Type conversions, assertions, I/O, and core built-in functions
spec:
  - spec/11-built-in-functions.md
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
    status: not-started
  - id: "7A.6"
    title: Additional Built-in Functions
    status: not-started
  - id: "7A.7"
    title: Resource Management
    status: not-started
  - id: "7A.8"
    title: Compile-Time File Embedding
    status: not-started
  - id: "7A.9"
    title: Section Completion Checklist
    status: in-progress
---

# Section 7A: Core Built-ins

**Goal**: Type conversions, assertions, I/O, and core built-in functions

> **SPEC**: `spec/11-built-in-functions.md`
> **PROPOSALS**:
> - `proposals/approved/as-conversion-proposal.md` — Type conversion syntax
> - `proposals/approved/developer-functions-proposal.md` — Developer convenience functions
> - `proposals/approved/embed-expression-proposal.md` — Compile-time file embedding

---

## 7A.1 Type Conversions

> **PROPOSAL**: `proposals/approved/as-conversion-proposal.md`
>
> Type conversions use `as`/`as?` syntax instead of `int()`, `float()`, etc.
> This removes the special-case exception for positional arguments.

- [ ] **Implement**: `As<T>` trait — infallible conversions
  - [ ] **Rust Tests**: `oric/src/typeck/traits/as_trait.rs` — As trait tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/conversions.ori`
  - [ ] **LLVM Support**: LLVM codegen for As trait
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/conversion_tests.rs` — As trait codegen

- [ ] **Implement**: `TryAs<T>` trait — fallible conversions returning `Option<T>`
  - [ ] **Rust Tests**: `oric/src/typeck/traits/try_as_trait.rs` — TryAs trait tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/conversions.ori`
  - [ ] **LLVM Support**: LLVM codegen for TryAs trait
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/conversion_tests.rs` — TryAs trait codegen

- [ ] **Implement**: `x as T` syntax — desugars to `As<T>.as(self: x)`
  - [ ] **Rust Tests**: `oric/src/eval/as_conversion.rs` — as syntax tests
  - [ ] **Ori Tests**: `tests/spec/expressions/as_conversion.ori`
  - [ ] **LLVM Support**: LLVM codegen for as syntax
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/conversion_tests.rs` — as syntax codegen

- [ ] **Implement**: `x as? T` syntax — desugars to `TryAs<T>.try_as(self: x)`
  - [ ] **Rust Tests**: `oric/src/eval/as_conversion.rs` — as? syntax tests
  - [ ] **Ori Tests**: `tests/spec/expressions/as_conversion.ori`
  - [ ] **LLVM Support**: LLVM codegen for as? syntax
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/conversion_tests.rs` — as? syntax codegen

- [ ] **Implement**: Standard `As` implementations
  - `impl As<float> for int` — widening (infallible)
  - `impl As<str> for int` — formatting (infallible)
  - `impl As<str> for float` — formatting (infallible)
  - `impl As<str> for bool` — "true"/"false" (infallible)
  - `impl As<int> for char` — codepoint (infallible)
  - [ ] **Ori Tests**: `tests/spec/stdlib/as_impls.ori`
  - [ ] **LLVM Support**: LLVM codegen for standard As implementations
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/conversion_tests.rs` — As implementations codegen

- [ ] **Implement**: Standard `TryAs` implementations
  - `impl TryAs<int> for str` — parsing (fallible)
  - `impl TryAs<float> for str` — parsing (fallible)
  - `impl TryAs<byte> for int` — range check (fallible)
  - `impl TryAs<char> for int` — valid codepoint check (fallible)
  - [ ] **Ori Tests**: `tests/spec/stdlib/try_as_impls.ori`
  - [ ] **LLVM Support**: LLVM codegen for standard TryAs implementations
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/conversion_tests.rs` — TryAs implementations codegen

- [ ] **Implement**: Compile-time enforcement — `as` only for infallible conversions
  - [ ] **Rust Tests**: `oric/src/typeck/checker/as_conversion.rs` — enforcement tests
  - [ ] **Ori Tests**: `tests/compile-fail/as_fallible.ori`
  - [ ] **LLVM Support**: LLVM codegen for as conversion enforcement
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/conversion_tests.rs` — as enforcement codegen

- [ ] **Implement**: Float truncation methods (not `as`)
  - `float.truncate() -> int` — toward zero
  - `float.round() -> int` — nearest
  - `float.floor() -> int` — toward negative infinity
  - `float.ceil() -> int` — toward positive infinity
  - [ ] **Ori Tests**: `tests/spec/stdlib/float_methods.ori`
  - [ ] **LLVM Support**: LLVM codegen for float truncation methods
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/conversion_tests.rs` — float truncation codegen

- [ ] **Remove**: `int()`, `float()`, `str()`, `byte()` function syntax
  - These are replaced by `as`/`as?` syntax
  - No migration period needed if implementing fresh
  - [ ] **LLVM Support**: LLVM codegen removal of legacy conversion functions
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/conversion_tests.rs` — verify legacy functions removed

---

## 7A.2 Assertions

- [x] **Implement**: `assert(cond:)` [done] (2026-02-10)
  - [x] **Ori Tests**: Used in hundreds of tests across test suite (`assert(cond: ...)`)
  - [ ] **LLVM Support**: LLVM codegen for assert
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/assertion_tests.rs` (file does not exist)

- [x] **Implement**: `assert_eq(actual:, expected:)` [done] (2026-02-10)
  - [x] **Ori Tests**: Used in hundreds of tests across test suite
  - [ ] **LLVM Support**: LLVM codegen for assert_eq
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/assertion_tests.rs` (file does not exist)

- [x] **Implement**: `assert_ne(actual:, expected:)` [done] (2026-02-10)
  - [x] **Ori Tests**: Used in module tests (`tests/spec/modules/`)
  - [ ] **LLVM Support**: LLVM codegen for assert_ne
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/assertion_tests.rs` (file does not exist)

- [ ] **Implement**: `assert_some(x)` — spec/11-built-in-functions.md § assert_some
  - [ ] **Ori Tests**: Not verified — not found in test suite
  - [ ] **LLVM Support**: LLVM codegen for assert_some

- [ ] **Implement**: `assert_none(x)` — spec/11-built-in-functions.md § assert_none
  - [ ] **Ori Tests**: Not verified — not found in test suite
  - [ ] **LLVM Support**: LLVM codegen for assert_none

- [ ] **Implement**: `assert_ok(x)` — spec/11-built-in-functions.md § assert_ok
  - [ ] **Ori Tests**: Not verified — not found in test suite
  - [ ] **LLVM Support**: LLVM codegen for assert_ok

- [ ] **Implement**: `assert_err(x)` — spec/11-built-in-functions.md § assert_err
  - [ ] **Ori Tests**: Not verified — not found in test suite
  - [ ] **LLVM Support**: LLVM codegen for assert_err

---

## 7A.3 I/O and Other

- [x] **Implement**: `print(x)` [done] (2026-02-10)
  - [x] **Ori Tests**: Used in test suite; LLVM has `_ori_print` runtime function
  - [x] **LLVM Support**: LLVM codegen for print — `_ori_print` in runtime
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/io_tests.rs` (file does not exist)

- [x] **Implement**: `compare(a, b)` [done] (2026-02-10)
  - [x] **Ori Tests**: `tests/spec/traits/core/comparable.ori` — 58 tests for `.compare(other:)`
  - [x] **LLVM Support**: LLVM codegen for compare — inline IR in lower_calls.rs
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/comparison_tests.rs` (file does not exist)

- [x] **Implement**: `min(a, b)`, `max(a, b)` [done] (2026-02-10)
  - [x] **Ori Tests**: Prelude functions available, verified in Section 4.6
  - [ ] **LLVM Support**: LLVM codegen for min/max
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/comparison_tests.rs` (file does not exist)

- [x] **Implement**: `panic(msg)` [done] (2026-02-10)
  - [x] **Ori Tests**: Used in `#fail` test attributes (division by zero, index out of bounds)
  - [x] **LLVM Support**: LLVM codegen for panic — `_ori_panic` in runtime
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/panic_tests.rs` (file does not exist)

---

## 7A.4 Float NaN Behavior

> **Decision**: NaN comparisons panic (no proposal needed — behavioral decision)
>
> Fits Ori's "bugs should be caught" philosophy (same as integer overflow).

- [ ] **Implement**: NaN comparison panics
  - `NaN == NaN` → PANIC
  - `NaN < x` → PANIC
  - `NaN > x` → PANIC
  - [ ] **Rust Tests**: `oric/src/eval/exec/binary.rs` — NaN comparison tests
  - [ ] **Ori Tests**: `tests/spec/types/float_nan.ori`
  - [ ] **LLVM Support**: LLVM codegen for NaN comparison panic
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/float_tests.rs` — NaN comparison panic codegen

- [ ] **Implement**: NaN-producing operations don't panic (only comparisons)
  - `0.0 / 0.0` → NaN (allowed)
  - Using NaN in arithmetic → NaN (allowed)
  - Comparing NaN → PANIC
  - [ ] **Ori Tests**: `tests/spec/types/float_nan_ops.ori`
  - [ ] **LLVM Support**: LLVM codegen for NaN-producing operations
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/float_tests.rs` — NaN operations codegen

---

## 7A.5 Developer Functions

> **PROPOSAL**: `proposals/approved/developer-functions-proposal.md`
>
> `todo`, `unreachable`, and `dbg` for developer convenience. These provide
> semantic meaning (unfinished vs. impossible code) and inline debugging.

- [ ] **Implement**: `todo()` and `todo(reason:)` — Mark unfinished code
  - Returns `Never`, panics with "not yet implemented at file:line"
  - Location information captured at compile time
  - [ ] **Rust Tests**: `oric/src/eval/builtins.rs` — todo tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/developer_functions.ori`
  - [ ] **LLVM Support**: LLVM codegen for todo
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/developer_tests.rs` — todo codegen

- [ ] **Implement**: `unreachable()` and `unreachable(reason:)` — Mark impossible code
  - Returns `Never`, panics with "unreachable code reached at file:line"
  - Semantically distinct from `todo` (impossible vs. not done)
  - [ ] **Rust Tests**: `oric/src/eval/builtins.rs` — unreachable tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/developer_functions.ori`
  - [ ] **LLVM Support**: LLVM codegen for unreachable
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/developer_tests.rs` — unreachable codegen

- [ ] **Implement**: `dbg(value:)` and `dbg(value:, label:)` — Debug printing
  - Generic: `dbg<T: Debug>(value: T) -> T`
  - Writes to stderr via Print capability
  - Output format: `[file:line] = value` or `[file:line] label = value`
  - Returns value unchanged for inline use
  - [ ] **Rust Tests**: `oric/src/eval/builtins.rs` — dbg tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/developer_functions.ori`
  - [ ] **LLVM Support**: LLVM codegen for dbg
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/developer_tests.rs` — dbg codegen

- [ ] **Implement**: Compile-time location capture for all three functions
  - Location passed implicitly by compiler, not visible in user signature
  - [ ] **Rust Tests**: `oric/src/typeck/builtins.rs` — location capture tests
  - [ ] **Ori Tests**: Verify location appears in panic messages/dbg output

---

## 7A.6 Additional Built-in Functions

**Proposal**: `proposals/approved/additional-builtins-proposal.md`

Formalizes `repeat`, `compile_error`, `PanicInfo`, and clarifies `??` operator semantics.

### repeat Function

- [ ] **Implement**: `repeat<T: Clone>(value: T) -> impl Iterator` — infinite iterator of cloned values
  - [ ] **Rust Tests**: `oric/src/eval/builtins.rs` — repeat tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/repeat.ori`
  - [ ] **LLVM Support**: LLVM codegen for repeat
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/iterator_tests.rs` — repeat codegen

- [ ] **Implement**: Clone requirement enforcement — T must implement Clone
  - [ ] **Rust Tests**: `oric/src/typeck/builtins.rs` — repeat type checking
  - [ ] **Ori Tests**: `tests/compile-fail/repeat_not_clone.ori`

- [ ] **Implement**: Integration with Iterator trait — .take(), .collect(), etc.
  - [ ] **Ori Tests**: `tests/spec/stdlib/repeat_iterator.ori`

### PanicInfo Type

**Proposal**: `proposals/approved/panic-handler-proposal.md` (extends basic definition)

**Spec**: `spec/20-errors-and-panics.md` § PanicInfo Type (updated with full structure)

- [ ] **Spec**: PanicInfo type definition — `{ message, location, stack_trace, thread_id }` DONE

- [ ] **Implement**: `PanicInfo` struct type — `{ message: str, location: TraceEntry, stack_trace: [TraceEntry], thread_id: Option<int> }`
  - [ ] **Rust Tests**: `oric/src/typeck/types.rs` — PanicInfo type tests
  - [ ] **Ori Tests**: `tests/spec/types/panic_info.ori`
  - [ ] **LLVM Support**: LLVM codegen for PanicInfo
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/type_tests.rs` — PanicInfo codegen

- [ ] **Implement**: `Printable` impl for PanicInfo
  - [ ] **Ori Tests**: `tests/spec/types/panic_info_printable.ori`

- [ ] **Implement**: `Debug` impl for PanicInfo
  - [ ] **Ori Tests**: `tests/spec/types/panic_info_debug.ori`

- [ ] **Add to prelude**: PanicInfo available without import
  - [ ] **Ori Tests**: `tests/spec/prelude/panic_info.ori`

### @panic Handler

**Proposal**: `proposals/approved/panic-handler-proposal.md`

App-wide panic handler function that executes before program termination.

- [ ] **Implement**: Recognize `@panic` as special function (like `@main`)
  - [ ] **Rust Tests**: `oric/src/resolver/special_functions.rs` — @panic recognition
  - [ ] **Ori Tests**: `tests/spec/declarations/panic_handler.ori`
  - [ ] **LLVM Support**: LLVM codegen for @panic function recognition
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/panic_tests.rs` — @panic recognition codegen

- [ ] **Implement**: Validate signature `(PanicInfo) -> void`
  - [ ] **Rust Tests**: `oric/src/typeck/checker/special_fns.rs` — @panic signature validation
  - [ ] **Ori Tests**: `tests/compile-fail/panic_handler_wrong_sig.ori`

- [ ] **Implement**: Error if multiple `@panic` definitions
  - [ ] **Rust Tests**: `oric/src/resolver/special_functions.rs` — multiple @panic error
  - [ ] **Ori Tests**: `tests/compile-fail/multiple_panic_handlers.ori`

- [ ] **Implement**: Implicit stderr for print() inside @panic
  - [ ] **Rust Tests**: `oric/src/eval/panic_handler.rs` — stderr redirection
  - [ ] **Ori Tests**: `tests/spec/declarations/panic_print_stderr.ori`
  - [ ] **LLVM Support**: LLVM codegen for stderr redirection in @panic
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/panic_tests.rs` — stderr redirection codegen

- [ ] **Implement**: Runtime panic hook installation at program start
  - [ ] **Rust Tests**: `oric/src/runtime/panic.rs` — hook installation
  - [ ] **LLVM Support**: LLVM codegen for panic hook installation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/panic_tests.rs` — hook installation codegen

- [ ] **Implement**: Construct PanicInfo (message, location, stack_trace, thread_id) on panic
  - [ ] **Rust Tests**: `oric/src/runtime/panic.rs` — PanicInfo construction
  - [ ] **Ori Tests**: `tests/spec/runtime/panic_info_construction.ori`
  - [ ] **LLVM Support**: LLVM codegen for PanicInfo construction
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/panic_tests.rs` — PanicInfo construction codegen

- [ ] **Implement**: Re-panic detection — immediate termination if handler panics
  - [ ] **Rust Tests**: `oric/src/runtime/panic.rs` — re-panic detection
  - [ ] **Ori Tests**: `tests/spec/runtime/panic_in_handler.ori`
  - [ ] **LLVM Support**: LLVM codegen for re-panic detection
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/panic_tests.rs` — re-panic detection codegen

- [ ] **Implement**: First panic wins in concurrent context
  - [ ] **Rust Tests**: `oric/src/runtime/panic.rs` — concurrent panic handling
  - [ ] **Ori Tests**: `tests/spec/runtime/concurrent_panic.ori`
  - [ ] **LLVM Support**: LLVM codegen for concurrent panic handling
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/panic_tests.rs` — concurrent panic codegen

- [ ] **Implement**: Default handler (when no @panic defined) — print to stderr
  - [ ] **Rust Tests**: `oric/src/runtime/panic.rs` — default handler
  - [ ] **Ori Tests**: `tests/spec/runtime/default_panic_handler.ori`
  - [ ] **LLVM Support**: LLVM codegen for default handler
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/panic_tests.rs` — default handler codegen

- [ ] **Implement**: Exit with non-zero code after handler returns
  - [ ] **Rust Tests**: `oric/src/runtime/panic.rs` — exit code
  - [ ] **LLVM Support**: LLVM codegen for exit code handling
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/panic_tests.rs` — exit code codegen

---

## 7A.7 Resource Management

**Proposal**: `proposals/approved/drop-trait-proposal.md`

Adds `drop_early` function for explicit early resource release.

### drop_early Function

- [ ] **Implement**: `drop_early<T>(value: T) -> void` — Force drop before scope exit
  - [ ] **Rust Tests**: `oric/src/eval/builtins.rs` — drop_early tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/drop_early.ori`
  - [ ] **LLVM Support**: LLVM codegen for drop_early
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/drop_tests.rs` — drop_early codegen

- [ ] **Implement**: drop_early works for any type (not restricted to T: Drop)
  - Types with Drop: drop method called, then memory reclaimed
  - Types without Drop: memory reclaimed immediately
  - [ ] **Ori Tests**: `tests/spec/stdlib/drop_early_any_type.ori`

- [ ] **Add to prelude**: drop_early available without import
  - [ ] **Ori Tests**: `tests/spec/prelude/drop_early.ori`

- [ ] **Update Spec**: `spec/11-built-in-functions.md` — add drop_early documentation
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

- [ ] **Implement**: `has_embed(path)` — Compile-time file existence check
  - Returns compile-time `bool`
  - Same path resolution rules as `embed`
  - [ ] **Rust Tests**: `ori_types/src/infer/expr/embed.rs` — has_embed type checking
  - [ ] **Ori Tests**: `tests/spec/embed/has_embed.ori`
  - [ ] **LLVM Support**: LLVM codegen for has_embed
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/embed_tests.rs` — has_embed codegen

- [ ] **Implement**: File size limit enforcement (10 MB default)
  - `#embed_limit(size:)` attribute for per-expression override
  - `ori.toml` `[embed] max_file_size` for project-wide override
  - [ ] **Ori Tests**: `tests/compile-fail/embed_size_limit.ori`

- [ ] **Implement**: File dependency tracking in Salsa
  - Hash-based invalidation (content hash, not mtime)
  - Embedded file changes trigger recompilation
  - `has_embed` file existence changes trigger recompilation
  - [ ] **Rust Tests**: `oric/src/queries/embed.rs` — dependency tracking

- [ ] **Implement**: Error diagnostics
  - File not found (with "did you mean?" suggestions)
  - Absolute path error
  - Path escapes project root error
  - Invalid UTF-8 error (when `str` expected)
  - Cannot infer embed type error
  - File exceeds size limit error
  - [ ] **Ori Tests**: `tests/compile-fail/embed_errors.ori`

- [ ] **Implement**: Binary deduplication — multiple references to same file share one copy
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/embed_tests.rs` — deduplication

---

## 7A.9 Section Completion Checklist

- [ ] All items above have all checkboxes marked `[ ]`
- [ ] Re-evaluate against docs/compiler-design/v2/02-design-principles.md
- [ ] 80+% test coverage, tests against spec/design
- [ ] Run full test suite: `./test-all.sh`
- [ ] **LLVM Support**: All LLVM codegen tests pass

**Exit Criteria**: Core built-in functions working correctly
