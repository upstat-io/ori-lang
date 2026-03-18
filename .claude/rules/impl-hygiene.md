---
paths:
  - "*.rs"
---

# Hygiene Rules

These rules cover **code quality**: structure, naming, types, errors, performance. Process rules (TDD, git workflow, CI) live in CLAUDE.md. No duplication between them.

## Finding Categories

- **LEAK** — Data or control crossing a boundary it shouldn't (phase bleeding, backward reference, swallowed error). Default: **Critical**.
- **DRIFT** — Registration data present in one location but missing from a parallel sync point. Default: **Major**.
- **GAP** — Feature supported in one phase but blocked/missing in another. Default: **Major**.
- **WASTE** — Unnecessary allocation, clone, or transformation at boundary. Default: **Minor**.
- **EXPOSURE** — Internal state leaking through boundary types. Default: **Minor**.
- **BLOAT** — File exceeds limits, mixes responsibilities, lacks submodule structure. Default: **Minor**.
- **NOTE** — Observation, acceptable tradeoff, documented exception. Default: **Informational**.

5+ findings clustered in one module = design problem; escalate to architectural review, not individual fixes.

## Phase Boundaries

- **One-way data flow**: later phases never call back into earlier phases
- **No circular imports**: `ori_lexer` never imports `ori_parse`
- **Minimal boundary types**: pass only what next phase needs
- **Clean ownership transfer**: move at boundaries, borrow within phases; no unnecessary `.clone()`
- **No phase bleeding**: lexer doesn't parse, parser doesn't type-check, type checker doesn't codegen
- **Phase purity**: output depends only on input; no global mutable state, no side channels
- **Delayed action queues**: when forward references needed in imperative passes (ARC, optimization), enqueue work — don't loop or backtrack. Salsa's demand-driven model handles forward refs via memoized queries.

### Phase-Specific Purity

- **Lexer**: scanning with minimal local state (nesting depth, mode stack); produces `(tag, len)`. No semantic state (names, types, scopes).
- **Parser**: syntax only; builds AST from tokens; no name resolution or semantic validation
- **Type Checker**: consumes AST, produces typed IR; no re-parsing, no codegen. Salsa queries must be pure.
- **Evaluator**: interprets typed IR; no re-type-checking, no codegen
- **ARC Pass**: analyzes ownership on IR; no codegen, no interpretation
- **LLVM Codegen**: emits LLVM IR from typed IR; no interpretation, no re-type-checking
- **Diagnostics**: formats and renders errors; no phase logic, no semantic analysis
- **Optimization Passes**: reads IR, produces transformed IR; analysis is pass-local

### Error Recovery Per Phase

- **Parser**: inserts error nodes, synchronizes to next statement
- **Type Checker**: uses error type (TyError) that unifies with anything; continues checking
- **Evaluator**: accumulates errors, skips dependent evaluations
- **Codegen**: aborts if any type errors remain — requires error-free input

## Data Flow

- **Zero-copy**: spans reference source by position; tokens carry `(tag, len)`, not string copies. Owned copies acceptable for diagnostic messages and error context that outlive the phase.
- **Phase-scoped allocation**: Salsa interning for cross-phase data; temporaries freed when phase completes; no leaking scratch data to next phase
- **Interned values via opaque indices**: cross boundaries with `Name`, `ExprId`, `TypeId` — never raw `u32`
- **No heap allocation in hot paths**: no `format!()`, `String::from()`, `Box::new()` per token. `Vec::new()` is fine (zero-alloc); avoid `Vec::with_capacity()`, `String::from()` per iteration.
- **Source text owned by Salsa**: all phase access via borrow from Salsa queries. No phase copies the full source text into its own data structures.
- **Hash lookups over linear scans**: for collections > ~8 items; small fixed-size lists may use linear scan
- **Pre-allocate**: `Vec::with_capacity()` with reasonable estimates

### Hot Paths

Hot: lexer scan loop, parser expression/statement parsing, type inference unification, IR traversal/transformation, codegen emission. Cold: error/diagnostic formatting, CLI, startup, test setup. When unsure, profile.

### String Hygiene

- Identifiers always interned via `Name` — never compare identifier strings directly
- Source text always `&str` borrowed from Salsa
- Error messages may own `String`
- No `format!()` in hot paths; use `write!()` to a buffer for complex string building

## Error Handling

- **Accumulate, don't bail**: each phase collects all errors in one pass; cap at configurable limit (default ~100) and emit "too many errors, stopping"
- **Phase-scoped error types**: lexer errors != parse errors != type errors. Each type is an enum with one variant per distinct error condition. Every variant carries: span, error code, context for the diagnostic formatter.
- **Upstream errors propagated**: use `?` for propagation, `impl From<UpstreamError>` for cross-phase conversion, `.map_err()` for context at boundaries. Never `Ok(default)` to swallow errors.
- **Errors carry spans**: every error includes source position; spanless errors are bugs
- **Recovery is explicit**: enum variants, not implicit booleans
- **Structured construction**: `Diagnostic::error(code).with_message(...).with_label(...)` — never `format!()` strings
- **Expected context**: every "expected X, got Y" MUST include WHY — annotation, return type, parameter
- **Deduplication**: deduplicate by (error code, primary span). Suppress follow-on: if error at span S produces TyError, suppress subsequent type errors involving TyError at child spans.
- **Edit-distance suggestions**: Damerau-Levenshtein for "did you mean?" — threshold: `distance <= min(name.len() - 1, max(2, name.len() / 3))`
- **Error codes are stable API**: once assigned, never reuse or change meaning. Ranges: E0xxx = parse, E1xxx = type check, E2xxx = semantic. Tests assert on error codes, not exact message text.
- **Anti-patterns**: `match err { Err(_) => Ok(default) }`, `if let Ok(x) = fallible` (silently drops error), `.unwrap_or_default()` on Result in production code

### Diagnostic Message Quality

- Plain language, not compiler internals; lead with what's wrong
- Suggest how to fix it; show the fix, not just the problem
- No "you" blame language; show code context when possible

## Type Discipline

- **Separate raw vs cooked types**: each phase boundary has distinct input/output types where the transformation is non-trivial
- **Newtypes for all IDs**: `ExprId`, `TypeId`, `TokenIndex` — not raw `u32`. Inner field private, construct via `new()`/`From`, `.0` access only inside defining module.
- **Metadata in sidecars**: metadata (spans, comments, debug info, source locations) travels in sidecars or indexed maps, not inline in core data structures. Core types stay lean. Exception: spans are small enough (u32 pair) to inline.
- **No phase state in output types**: AST nodes carry structure + spans, not parser cursor or inference state. Inference variables (unification, skolem) must be resolved before emitting typed IR.
- **Pre-compute derivable metadata**: at construction/interning time (e.g., TypeFlags, is_generic, size). O(1) queries, never re-walk composite structures. Salsa memoization handles invalidation for interned values; avoid manual caches.
- **Option vs Result**: `Option` for absent/not found (lookup miss). `Result` for failure with diagnostic info. Never `Result<T, ()>` — use `Option`. Never `Option` when None should carry an error.
- **Type aliases**: for long generic types (e.g., `Result<T, E>` with fixed E). Never for simple types. Alias names add semantic meaning. Don't shadow std types without purpose.

### Dispatch Choice

- Static dispatch (generics) by default
- `dyn Trait` only for user-extensible plugin points or heterogeneous collections
- Cost: `&dyn` < `Box<dyn>` < `Arc<dyn>`. Never `Arc<dyn>` in hot paths.

## Pass Composition

- **Each pass is (IR, Config) → IR**: configuration (opt level, target) is an explicit parameter. No env vars, no static mut, no thread-locals.
- **Explicit pass ordering**: dependencies documented in `//!` module doc of the pass manager and comments at each invocation site. Assert ordering with tests.
- **No shared mutable state between passes**: inter-pass communication via IR only. Exception: diagnostic accumulation is append-only — passes may append but not read/mutate each other's diagnostics.
- **Boundary validation**: `debug_assert!` invariants before crossing to next phase. Examples: all type variables resolved before codegen, no unreachable blocks in IR, RC ops balanced after ARC pass.

## Invariant Explicitness

- **Implicit invariants are invisible regressions.** If correctness depends on a property (RC balanced after loop, scope restored after block, phantom var inserted before iteration, elem_dec_fn non-NULL for heap types), it MUST be either:
  - A `debug_assert!` at the point where the invariant is relied upon, OR
  - A test that would fail if the invariant is violated
- **Semantic changes require semantic pins.** When a fix changes observable behavior (RC emission pattern, element cleanup order, scope lifetime, dec function selection), add a regression test that ONLY passes with the new semantics. This test is the permanent guard against revert.
- **Cross-section fixes require cross-section plan updates.** If implementing Section X requires changing code owned by Section Y, you MUST update Section Y's plan to reflect the change. A partial fix absorbed silently across section boundaries creates invisible dependencies that compound into cascading failures.

## Narrow the Front

- **Complete one fix fully before starting another.** RC + control-flow + lowering interactions multiply failure surfaces. Concurrent changes across these domains compound risk.
- **"Fully" means**: fix + matrix tests + semantic pin + plan update. A fix without matrix tests is incomplete. A fix with tests but without plan update (when cross-section) is incomplete.
- **Prefer depth over breadth.** Fix one element type across all patterns before fixing a second element type. Fix one loop variant completely before the other. This reduces the number of concurrent moving parts and makes failures narrow and explainable.

## Registration Sync Points

- **Single source of truth**: one location is canonical, others derived/validated
- **No manual mirroring**: centralize via `from_str()`, `all()`, iterator — not parallel lists
- **Compile-time or test-time enforcement**: add test iterating source-of-truth list
- **Flag drift as finding**: new variant in one location but missing from parallel = **DRIFT**
- **New type checklist**: new pub types need: Debug derive, Display if user-facing, From conversions for cross-phase types, documentation, tests. New types trigger sync requirements — not just new enum variants.

## Gap Detection

- **Cross-phase capability mismatch = GAP**: one phase supports a feature, another blocks it
- **Never silently work around a gap**: flag immediately
- **Audit across phases**: when adding capability, verify full pipeline: lexer → parser → typeck → eval → codegen

## Cascading Fix Detection

- **Whack-a-mole = architectural issue**: fix at one callsite moves failure to next → STOP
- **Three-strike rule**: same fix at 3+ callsites = missing abstraction; fix at boundary
- **More heuristics**: >3-4 params → config struct. Same enum matched in 3+ files → centralize dispatch. Same error string in 3+ places → error factory function.
- **Present options**: (1) architectural issue, (2) why per-site patches won't scale, (3) 2-3 options

## File Organization

- **500-line limit**: source files (excluding tests); exceeding = **BLOAT** finding
- **Proactive split**: split at ~450 lines if you know more code is coming. Don't wait until over the limit.
- **Single responsibility per file**: one logical operation or one type family. Anti-pattern: `utils.rs`, `helpers.rs`, `misc.rs`. Every file name describes its domain.
- **Submodule extraction**: logical group exceeding ~200 lines → sibling submodule; parent `mod.rs` = dispatch hub
- **Directory structure**: mirrors the logical phase/pass structure
- **Split when touching**: touching a file over 500 lines without splitting = finding
- **Tests in sibling `tests.rs`**: `#[cfg(test)] mod tests;` declaration only — body in sibling file
- **Section markers**: plain `// Section name` on its own line, preceded by blank line. No decorative characters. If sections exceed ~200 lines, extract to submodule instead.
- **Banner removal**: if you touch a file with decorative banners (`// ===`, `// ---`), remove them.

### File Layout (top to bottom)

1. `//!` module docs
2. `mod` declarations
3. Imports (3 groups, blank-line separated: external → crate → relative, alphabetical within)
4. Type aliases
5. Type definitions (structs, enums)
6. Inherent `impl` blocks (immediately after their type)
7. Trait `impl` blocks (immediately after inherent impls)
8. Free functions
9. `#[cfg(test)] mod tests;` at bottom

### Module Roles

- `lib.rs` is an **index**: `//!` doc, `mod` declarations, `pub use` re-exports — no function bodies. Strict, no exceptions.
- `mod.rs` **dispatches**: routes to submodules, holds shared private items
- Leaf files **implement**: actual logic lives here

### Crate Organization

- Each crate has a single documented purpose
- Module nesting max 4 levels (e.g., `ori_types::check::registration::traits`). Deeper = missing abstraction.
- If a crate has >50 source files, consider splitting
- Shared utilities live in dedicated crates (`ori_diagnostic`, `ori_patterns`, `ori_ir`). No `utils` modules in phase crates. If 3+ crates need the same utility, extract to a shared crate.

### Import Hygiene

- 3 groups separated by blank lines: external → crate → relative, alphabetical within
- No glob imports (`use foo::*`) except in test modules and preludes
- No unused imports
- Re-export only types that are part of your crate's public API contract. Consumers import from the crate that owns the type.

## Impl Block Method Ordering

1. **Constructors**: `new`, `with_*`, `from_*`, factory methods
2. **Accessors**: getters, `as_*` (cheap ref conversions)
3. **Predicates**: `is_*`, `has_*`, `can_*`, `contains`
4. **Public operations**: the main thing this type does
5. **Conversion/consumption**: `into_*`, `to_*`
6. **Private helpers**: in call-order grouping, not alphabetical

Within each group: pub before pub(crate) before private (loose).

## Struct/Enum Ordering

**Struct fields:**
1. Primary data (core state)
2. Secondary/derived data
3. Configuration/options
4. Flags/booleans last

Inline comments on struct fields when purpose isn't obvious.

**Enum variants:** ordered by frequency/importance (common first) or logically grouped (keywords together, operators together). Match arms follow the enum's declaration order.

## Naming

**Functions** — verb-based prefixes:
- Predicates: `is_*`, `has_*`, `can_*`
- Conversions: `into_*` (consuming), `to_*` (borrowing), `as_*` (cheap ref), `from_*` (construct)
- Processing: `cook_*` (lexer), `parse_*` (parser), `check_*` (typeck), `eval_*` (evaluator), `emit_*` (codegen)
- Consumption: `eat_*` (advance past), `skip_*` (advance+discard)
- Resolution: `resolve_*`, `lookup_*`, `fresh_*`
- Factory: `new`, `with_*`

**Variables** — scope-scaled:
- 1 char in <= 3 lines: `c`, `i`, `n`, `b`
- 2-4 chars in <= 15 lines: `ch`, `tok`, `pos`, `len`, `src`, `buf`, `err`, `kw`
- Descriptive in larger scopes: `token_span`, `base_offset`, `content_str`

**Constants**: `SCREAMING_SNAKE_CASE`, descriptive names.
**Type aliases**: `PascalCase`, suffix with purpose.
**Modules**: `snake_case`, noun-based.
**Crates**: `ori_` prefix.
**Generic parameters**: `T`/`E`/`K`/`V` for standard patterns; descriptive names when 2+ type params or domain-specific meaning. Never bare `T` with 3+ type params.

## Visibility

- Private by default; minimize pub surface
- `pub(crate)` for cross-module internal use
- `pub(super)` for parent-module access; prefer narrowest visibility that works
- No dead pub items; no dead code
- Items pub only for testing: `#[cfg(test)] pub` or `pub(crate)` with `// test-only` comment
- `#[non_exhaustive]` for public library APIs only. Internal compiler enums should be exhaustively matched — the compiler error on new variants catches missing match arms.

## Comments

- `//!` module doc on every file; `///` on all `pub` items
- All pub types and functions get `///` docs; use `` [`TypeName`] `` for cross-references; no docs that just restate the function name
- Comment WHY, not WHAT; `debug_assert!` for preconditions
- **Anti-patterns**: `// increment counter` (restates code), `// TODO` without context, `// This is a hack` without explaining the proper fix
- No decorative banners (`// ===`, `// ---`, `// ***`)
- No comments restating code, no commented-out code ever (use version control), no bare `// TODO`
- TODOs: format `// TODO(phase): description` — e.g., `// TODO(typeck): handle generic associated types`. Every TODO references a plan or roadmap item. No orphan TODOs.
- Section labels in large enums/matches: plain `// Section name`
- **Spec citations required**: code implementing grammar rules, operator semantics, type rules, or language semantics must cite the spec clause. Format: `// Spec: Clause N.M — description`

## Derives

- **Debug on all pub types** (required for tracing and error context)
- **Clone** only when semantic copying makes sense
- **PartialEq/Eq** only for types used in comparisons/hash maps
- **Hash** only when Eq is derived
- **Derive** when impl is standard (field-by-field equality, hash, debug)
- **Manual** only when behavior differs — articulate WHY

## Lint Discipline

- `#[expect(clippy::lint, reason = "...")]` — never bare `#[allow(clippy::...)]`
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
- Minimize unsafe scope — extract safe logic outside the unsafe block
- **FFI exports**: `ori_` prefix, `#[no_mangle]` + `extern "C"`, null checks on pointer params
- **C types**: use `std::ffi` (`c_char`, `c_int`, `c_void`, `CStr`, `CString`). Never `i8`/`i32` for C types. Never assume char is signed.
- **ABI contract**: codegen and runtime must agree on struct layouts. Changes to `ori_rt` function signatures must update `ori_llvm` call sites in the same commit.
- **Platform-specific code**: isolate behind `#[cfg(target_arch)]` blocks. Abstract platform differences.

## Salsa & Caching

- Queries must be pure: no side effects, no non-determinism
- No `Arc<Mutex<T>>` in query inputs or values
- Prefer fine-grained queries over coarse
- Salsa interned values automatically invalidated. For non-Salsa caches, document the invalidation strategy. Prefer Salsa queries over manual caches.

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

## Test Hygiene

- Test behavior, not implementation. Each test tests one thing.
- No test ordering dependencies; no shared mutable state between tests
- Descriptive names: `test_X_when_Y_then_Z`
- No `#[ignore]` without a tracking issue/reason
- **Test data**: fixtures in `tests/fixtures/` or `tests/spec/`. No hardcoded absolute paths — use relative paths or `CARGO_MANIFEST_DIR`. Test data committed to repo.

## Conditional Compilation

- `#[cfg(test)]` for test modules and test helpers only, not for production logic branching
- `#[cfg(debug_assertions)]` for debug-only checks
- Production code must not change behavior based on `#[cfg(test)]`

## Lifetime Annotations

- Prefer elision when possible
- Descriptive names for long-lived borrows: `'src`, `'ast`, `'ctx`
- Single-letter (`'a`) only for local/obvious cases
- Avoid >2 lifetime parameters per function

## API Stability

- Pub items in `lib.rs` are the stable API surface
- Breaking changes to pub crate APIs must update all downstream consumers in the same commit
- When replacing a code path, remove the old code in the same commit. No deprecation for internal compiler code.

## Dependencies

- Prefer `std` over external crates
- New external deps require justification
- Features are additive only (never remove functionality). Each feature documented in `Cargo.toml`.

## Concurrency

- Compiler internals are single-threaded (Salsa handles parallelism)
- No global mutable state (`static mut`, `lazy_static` with mutation). All state flows through function parameters or Salsa queries.
- Thread-safety required only at `ori_rt` FFI boundary

## CI & Build

- All code passes `./clippy-all.sh`, `./test-all.sh`, `./fmt-all.sh` before merge. No exceptions.
- No warnings in CI
- Build must succeed with default features and `--all-features`

## Commit Hygiene

- One logical change per commit; conventional commit format (`feat`/`fix`/`refactor`)
- Cross-crate changes that must be in sync go in a single commit
- Large refactors broken into phases: (1) add new API alongside old, (2) migrate consumers, (3) remove old API. Never break the build between phases.
- No WIP/temp commits on dev/master

## Technical Debt

- Fix when you find it. If it can't be fixed in the current change, add an entry to the active plan or create a roadmap item. No untracked debt.
- Experimental/prototype code lives in feature branches, never in dev/master.
