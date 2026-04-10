---
paths:
  - "**/*.rs"
---

# Code Style & Safety

Extracted from `impl-hygiene.md` -- derives, lints, performance annotations, style rules, clone discipline, unsafe/FFI, Salsa, panic/assertion, tracing.

## Derives

- **Debug on all pub types** (required for tracing and error context)
- **Clone** only when semantic copying makes sense
- **PartialEq/Eq** only for types used in comparisons/hash maps
- **Hash** only when Eq is derived
- **Derive** when impl is standard (field-by-field equality, hash, debug)
- **Manual** only when behavior differs -- articulate WHY

## Lint Discipline

- `#[expect(clippy::lint, reason = "...")]` -- never bare `#[allow(clippy::...)]`
- `#![deny(unsafe_code)]` in pure crates: `ori_ir`, `ori_diagnostic`, `ori_types`, `ori_eval`, `ori_patterns`, `ori_parse`, `ori_lexer`
- `unsafe` allowed only in: `ori_llvm`, `ori_rt`, `oric` (FFI, LLVM bindings, runtime)
- **Project-wide denied lints**: `unwrap_used` (prod code), `panic`, `todo`, `dbg_macro`, `print_stdout`/`print_stderr`. Override with `#[expect(reason)]` when genuinely needed.
- **No commented-out code ever**: use version control. Not in any branch.

## Performance Annotations

- `#[cold]` on error factory functions
- `#[inline]`: 1-5 lines freely. 6-20 lines only if profiling shows benefit or cross-crate hot path. >20 lines never.
- `#[repr(C)]` only for FFI types; default layout for everything else
- **Size assertions** on types in per-token/per-expression arrays or passed by value in hot loops. Add when size exceeds 2 machine words (16 bytes on 64-bit): `const _: () = assert!(size_of::<T>() == N);`
- `#[must_use]` on all pub functions returning `Result`/`Option`, builder methods returning `Self`, and pure functions where ignoring the return is always a bug. On types like `Diagnostic`, `Error`.

## Style

- Functions < 100 lines (target < 50). Exempt: dispatch tables, exhaustive enum matches. Match arms > 3 lines should call a helper.
- **Nesting depth**: max 4 levels. Prefer early returns (guard clauses) to reduce nesting.
- **Pattern consistency**: similar operations across files must use the same structural pattern. Before writing a new file, read 2-3 siblings for established conventions.
- **Iterator style**: iterator chains for transformations (map/filter/collect), manual loops for complex control flow (break/continue/multiple mutations). If a chain needs 4+ combinators or a closure > 3 lines, switch to a loop.
- No dead/commented-out code

## Clone Discipline

- Clone acceptable on cold/error paths and test setup
- Prefer `&str`/`Cow` over `String` at boundaries
- `Arc` only for shared ownership across threads/tasks
- No `.clone()` in hot paths without a comment justifying it

## Unsafe & FFI

- Every `unsafe` block requires a `// SAFETY:` comment explaining the invariant
- Minimize unsafe scope -- extract safe logic outside the unsafe block
- **FFI exports**: `ori_` prefix, `#[no_mangle]` + `extern "C"`, null checks on pointer params
- **C types**: use `std::ffi` (`c_char`, `c_int`, `c_void`, `CStr`, `CString`). Never `i8`/`i32` for C types. Never assume char is signed.
- **ABI contract**: codegen and runtime must agree on struct layouts. Changes to `ori_rt` function signatures must update `ori_llvm` call sites in the same commit.
- **Platform-specific code**: isolate behind `#[cfg(target_arch)]` blocks. Abstract platform differences.

## Salsa & Caching

- Queries must be pure: no side effects, no non-determinism
- No `Arc<Mutex<T>>` in query inputs or values
- Prefer fine-grained queries over coarse
- Salsa interned values automatically invalidated. For non-Salsa caches, document the invalidation strategy. Prefer Salsa queries over manual caches.
- **Tracked input ownership**: every `#[salsa::input]` must have exactly one mutator. Multiple writers to the same input = data race at the query level.
- **Dependency edge completeness**: if query A reads data that query B produces, A must transitively depend on B through Salsa. Reading through side channels (global state, files, env vars) breaks incremental.
- **Query-key stability**: keys used in tracked queries must have stable identity across revisions. If a key changes identity (e.g., different `ExprId` for the same expression after re-parse), all dependent queries re-execute even if the content is unchanged -- a performance cliff.

## Panic & Assertion

- **Never panic on user input**: all user-facing errors go through the diagnostic system
- **`.unwrap()`**: only with comment proving infallibility, or in tests. Production code: `.expect("reason")` or propagate with `?`.
- **`assert!()`**: for invariants whose violation would cause unsound codegen or safety issues. Always include a message.
- **`debug_assert!()`**: for expensive invariant checks (O(n) or worse)
- **`unreachable!()`**: for impossible code paths. Include context message. Never `panic!()` for impossible states.
- **Recursion depth**: all recursive traversals (type folding, expression visiting) must have a depth limit. Default: 256. Report clear error, don't stack overflow.

## Tracing & Logging

- Use `tracing` macros only (`trace!`, `debug!`, `info!`, `warn!`, `error!`). Never `println!`/`eprintln!` in library crates.
- `tracing::error!()` is for internal compiler failures only. User-facing errors go through the Diagnostic system.
- Error construction emits `trace!()` at debug level. Error recovery emits `warn!()`.
- `#[tracing::instrument]` on public APIs
- No sensitive data in logs, no log spam in hot loops
