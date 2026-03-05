# Compiler Coding Guidelines

Distilled from 10 reference compiler codebases: Rust, Go, Zig, Gleam, Roc, Swift, Lean 4, Elm, Koka, TypeScript.

Patterns included here are **proven across multiple production compilers**, not theoretical. Each rule cites which compilers demonstrate the pattern.

---

## 1. Error Handling & Diagnostics

### 1.1 Accumulate, Never Bail

Collect all errors in one pass; emit/render later. Never stop at the first error.

- **Go**: Queues errors in `[]errorMsg`, flushes sorted by position, caps at 10 errors
- **Rust**: `DiagCtxt` accumulates via `Lock<DiagCtxtInner>`; stashes errors for later reemission
- **Gleam**: Pure `compiler-core` returns error vecs; CLI decides whether to halt
- **Roc**: Problems collected per-module; reported after full solve
- **Zig**: `ErrorBundle` accumulated per compilation unit

**Rule**: Every phase returns `(Result, Vec<Diagnostic>)` — never panics on user errors.

### 1.2 Phase-Specific Error Types

Each compilation phase defines its own error ADT. Don't share a single error enum across phases.

- **Elm**: `BadSyntax | BadImports | BadNames | BadTypes | BadMains | BadPatterns | BadDocs` — one per phase
- **Go**: `codes.go` separates "bad" (syntax) from "invalid" (type) by naming convention
- **Rust**: `PResult<'a, T>` for parser, `ErrorGuaranteed` for later phases
- **Koka**: Monadic errors in `Inf` monad separate from parse errors

**Rule**: `LexError != ParseError != TypeError != CodegenError`. Each phase owns its error vocabulary.

### 1.3 Error Codes Are Stable API

Once assigned, an error code number never changes meaning. New errors get new codes.

- **Go**: "Code values should NEVER change; new codes added at end only"
- **TypeScript**: 300+ diagnostic codes, versioned in `diagnosticMessages.json`
- **Rust**: `ErrCode` via `newtype_index!`, docs via `include_str!`

**Rule**: Error codes are permanent. Document each with spec reference + example.

### 1.4 Structured Error Construction

Build errors from semantic components, not string concatenation.

- **Elm**: `toReport` decomposes errors into title + region + suggestions + doc
- **Rust**: `Diag<'_, G>` builder: `.span().label().note().emit()`
- **Gleam**: `Diagnostic { title, text, level, location, hint }` — composite struct
- **Koka**: Doc algebra combinators (`vcat`, `indent`, `hang`)
- **TypeScript**: Message templates with `{0}`, `{1}` placeholders

**Rule**: Errors are data structures first, strings second. Compose via builders or doc combinators.

### 1.5 Capture Expected Context

When reporting type mismatches, capture *why* something was expected, not just what.

- **Elm**: `Expected` type: `NoExpectation | FromContext Region Category | FromAnnotation`
- **Koka**: `matchArguments` carries source range + whether positional or named
- **Rust**: `ObligationCause` tracks where a trait bound originated

**Rule**: Every "expected X, got Y" error should also say "because of Z" — the context that created the expectation.

### 1.6 Edit-Distance Suggestions

Use Damerau-Levenshtein (not ad-hoc matching) for "did you mean?" suggestions.

- **Elm**: `Reporting.Suggest` — edit distance with case-insensitive ranking
- **Rust**: `find_best_match_for_name` uses Levenshtein
- **Go**: Type checker suggests similar identifiers on lookup failure

**Rule**: Suggestion quality scales with algorithm quality. Use a proven string distance metric.

### 1.7 Deduplication

Suppress duplicate or follow-on errors.

- **Rust**: `emitted_diagnostics: FxHashSet<Hash128>` — hash-based dedup
- **Go**: Same-line errors coalesced; "invalid operand" suppressed if prior error exists
- **Gleam**: Warning emitter tracks atomic count

**Rule**: Hash emitted diagnostics. Suppress cascading errors when earlier errors explain them.

---

## 2. Type Representation & Memory

### 2.1 Intern Everything, Reference by Index

All types stored once in a pool/arena; referenced by compact index.

- **Zig**: Single `InternPool.Index` (32-bit) represents both types and values
- **Rust**: `Ty<'tcx>` — pointer to interned type, lifetime-bound to `TyCtxt`
- **Roc**: `Subs` struct — unified type variable store, all-in-one arena
- **Lean 4**: Indices into global tables; `FVarId` / `MVarId` for variables

**Rule**: `Idx(u32)` is the universal type handle. O(1) equality via index comparison.

### 2.2 Newtype All Indices

Prevent accidental mixing of index types.

- **Zig**: `ComptimeAllocIndex`, `FileIndex`, `AnalUnit` — all wrap `u32` enum
- **Rust**: `newtype_index!` macro generates distinct index types
- **Roc**: `Variable(u32)`, `Symbol(u64)` — newtypes with custom display
- **TypeScript**: Branded string types: `type Path = string & { __pathBrand: any }`

**Rule**: `ExprId != TypeIdx != VarId`. Every index domain gets its own newtype.

### 2.3 Pre-Compute Metadata at Interning Time

Compute flags, hashes, and properties once when a type is created, not on every query.

- **Zig**: `InternPool` stores type + value + dependency info together
- **Rust**: `TypeFlags` computed on interning — `HAS_FREE_REGIONS`, `HAS_PROJECTIONS`, etc.

**Rule**: Metadata queries (has variables? needs substitution? is primitive?) should be O(1) flag checks.

### 2.4 No Arc in Hot Paths

Use arena allocation + lifetimes, not reference counting, for type data.

- **Rust**: Arenas + `'tcx` lifetime eliminate RC overhead entirely
- **Zig**: GPA + arena tiers; no reference counting in type system
- **Go**: GC handles memory; no manual RC

**Rule**: `Arc<T>` is for shared ownership across thread boundaries. Within a phase, use `&'a T` or arena indices.

### 2.5 Monitor Struct Sizes

Assert struct sizes at compile time to prevent accidental bloat.

- **Rust**: `static_assert_size!(PResult<'_, ()>, 24)`, `static_assert_size!(Parser<'_>, 288)`
- **Roc**: `assert_sizeof_all!` macros on `Descriptor`, `FlatType`

**Rule**: Hot-path structs should have compile-time size assertions. Catches regressions during review.

---

## 3. Parser Architecture

### 3.1 Recovery as Explicit State

Error recovery is an explicit mode, not implicit behavior.

- **Rust**: `Recovery::Allowed | Forbidden` enum — macros disable recovery (#103534)
- **Go**: `p.advance()` to skip to next safe token; explicit resynchronization
- **Gleam**: Recovery modes per-context

**Rule**: `Recovery` is a first-class enum. Functions know whether they may recover or must fail cleanly.

### 3.2 Restriction Bitflags for Context

Track parser context (am I in a pattern? a const expression? an if-guard?) via bitflags.

- **Rust**: `Restrictions: u8` — `STMT_EXPR`, `NO_STRUCT_LITERAL`, `CONST_EXPR`, `IS_PAT`
- **Go**: Pragma tracking, line directives

**Rule**: Context that affects parsing decisions lives in a bitflag set, not scattered booleans.

### 3.3 Token Consumption Primitives

Standardize a small set of token operations.

- **Rust**: `bump()` (advance), `check()` (lookahead), `expect()` (advance-or-error), `eat()` (try-advance)
- **Go**: `got()` (try-consume → bool), `want()` (assert-consume), `next()` (advance)

**Rule**: Define exactly 4-5 token primitives. All parsing logic composes from these.

### 3.4 Precedence via Binding Power

Use Pratt parsing with numeric binding powers, not grammar-rule recursion.

- **Rust**: `parse_expr_assoc_rest_with` uses binding power tables
- **Koka**: Operator fixity/precedence tables drive expression parsing

**Rule**: Operator precedence is a data table, not embedded in call structure.

---

## 4. Phase Boundaries & Pipeline

### 4.1 Phases Are Sacred Boundaries

Each phase consumes one IR and produces another. No reaching back.

- **Lean 4**: Explicit `Phase` enum (`base | mono | impure`) — passes declare required phase
- **Zig**: `ZIR → AIR → LLVM IR` pipeline; each transform is `IR → IR`
- **Swift**: `AST → SIL → LLVM IR` with per-function optimization
- **Koka**: Separate ops → unify → infer layers

**Rule**: Later phases never call back into earlier phases. Data flows one direction.

### 4.2 Phase-Specific Data Types

Don't reuse the same struct across phases. Each phase has its own vocabulary.

- **Rust**: `RawTag` (lexer) != `TokenKind` (parser) != `Ty<'tcx>` (typeck)
- **Lean 4**: Phase-specific IR nodes; `LCNF.Decl` != `IR.Decl`
- **Go**: `syntax.Type` (parser) != `types.Type` (typeck)

**Rule**: Boundary types are distinct. Converting between them is explicit.

### 4.3 Delayed Action Queues

Defer work that requires forward references.

- **Go**: `delayed: []action` — FIFO queue of closures for function body type-checking
- **Rust**: Stashed diagnostics reemitted by later passes
- **Koka**: Definition groups processed after all signatures collected

**Rule**: When a phase needs information that isn't yet available, enqueue work for later — don't loop or backtrack.

---

## 5. ARC & Ownership (Directly Relevant to Ori)

### 5.1 RC Identity Analysis

Track which values share reference count identity through projections.

- **Swift**: `%a ~rc %b` equivalence relation through RC-preserving instructions (`struct_extract`, `tuple_extract`)
- **Lean 4**: `DerivedValInfo` tracks parent-child relationships via projections

**Rule**: `retain(struct.field)` may equal `retain(struct)`. Track this to eliminate redundant RC ops.

### 5.2 Lattice-Based RC State Tracking

Model reference count knowledge as a state machine with meet/join operations.

- **Swift**: `None → Decremented → MightBeUsed → MightBeDecremented` with `KnownSafe` flag
- **Lean 4**: Iterative dataflow with `modified` flag for fixpoint convergence

**Rule**: RC optimization is dataflow analysis with lattice semantics, not ad-hoc pattern matching.

### 5.3 Two-Path Uniqueness Strategy

Static uniqueness check → in-place mutation; dynamic fallback → copy + mutate.

- **Lean 4**: `reset x`: fast path (unique → reuse in-place via `set`), slow path (`dec x; inc fields`)
- **Swift**: `is_unique` check gates ARC code motion

**Rule**: The COW protocol is: static unique → mutate | runtime check → branch | shared → copy. Optimize the common case.

### 5.4 Borrow Inference Per Function

Determine which parameters can be borrowed (no RC) vs must be owned.

- **Lean 4**: `BorrowInfState` with `owned: OwnedSet`; iterates to fixpoint
- **Swift**: `@owned` / `@guaranteed` parameter conventions
- **Koka**: `ParamInfo` tracks borrowed vs owned per parameter

**Rule**: Borrow inference is per-function, not global. Mark params as borrowed unless proven they need ownership.

### 5.5 Tail Call Preservation

Don't insert `dec` after a tail call — it breaks tail call optimization.

- **Lean 4**: `ownParamsUsingArgs` — if arg is owned at call site, mark callee param as owned to preserve tail call
- **Swift**: ARC operations hoisted out of tail position

**Rule**: RC operations must respect tail call boundaries. Ownership transfer replaces inc/dec at tail calls.

---

## 6. Code Organization

### 6.1 File Size Limits

Production compiler files rarely exceed 500 lines (excluding tests).

- **Rust**: Parser splits into `expr.rs`, `ty.rs`, `stmt.rs`, `pat.rs`, `path.rs`, `attr.rs`
- **Gleam**: 10+ impl blocks per concern, each ~100-200 lines
- **Lean 4**: One file per pass (`RC.lean`, `Borrow.lean`, `ExpandResetReuse.lean`)

**Rule**: 500 lines max. Split by concern into submodules before exceeding.

### 6.2 Module Structure

Consistent file layout within a module.

- **Rust**: `lib.rs` = pub re-exports → `mod.rs` = dispatch + private items → `submodule.rs`
- **Gleam**: `pub mod` for public, `pub(crate) mod` for internal, private `mod` for implementation
- **Roc**: Crate-level re-exports aggregate key types

**Rule**: `lib.rs` is an index, not implementation. `mod.rs` dispatches. Leaf files implement.

### 6.3 Import Organization

Group imports by origin, sort alphabetically within groups.

- **Rust**: `// tidy-alphabetical-start/end` comment markers enforce sort
- **Gleam**: External → crate → relative, sorted
- **TypeScript**: 100+ named imports per file, grouped by category

**Rule**: Three groups (external → crate → relative), alphabetical within each, blank line between groups.

### 6.4 Visibility Defaults

Private by default. Escalate visibility only when needed.

- **Rust**: `pub(crate)` common; `pub(super)` for parent; `pub` for crate API
- **Gleam**: `pub(crate)` for internal cross-module; `pub` only for CLI/WASM consumers
- **Roc**: `pub` re-exports at crate level; internals private

**Rule**: Default private → `pub(crate)` → `pub(super)` → `pub`. Justify each escalation.

---

## 7. Naming Conventions

### 7.1 Verb-Based Function Prefixes

Function names start with a verb describing the action.

| Prefix | Meaning | Examples (across compilers) |
|--------|---------|-----------------------------|
| `parse_*` | Build AST from tokens | Rust, Go, Gleam, Elm |
| `check_*` / `infer_*` | Type checking / inference | Rust, Go, Koka, Roc |
| `eval_*` | Runtime evaluation | Lean, Koka |
| `emit_*` | Generate output (IR/diagnostics) | Rust, Swift, Zig |
| `resolve_*` / `lookup_*` | Name/type resolution | Rust, Go, Gleam |
| `fresh_*` | Create fresh variable/ID | Rust, Koka, Roc |
| `is_*` / `has_*` / `can_*` | Predicates | Universal |
| `into_*` / `to_*` / `as_*` / `from_*` | Conversions | Rust convention, adopted by Gleam/Roc |

### 7.2 Lifetime & Type Parameter Names

- **Semantic lifetimes**: `'tcx` (type context), `'a` (arena), `'b` (allocator) — not arbitrary letters
- **Type parameters**: `T`, `G: EmissionGuarantee`, `E: Error` — constrained by meaning
- **Koka**: `Flavour` for variable kinds (Meta, Skolem, Bound)

**Rule**: Lifetimes and type params should be meaningful. `'tcx` > `'a` when context matters.

### 7.3 Variable Name Scaling

Variable name length should scale with scope size.

- **1-3 lines**: `c`, `i`, `n`, `t`, `v`
- **4-15 lines**: `ch`, `tok`, `pos`, `len`, `ctx`, `env`, `val`
- **15+ lines**: `receiver`, `method_name`, `arg_count`, `error_code`

**Rule**: Short scopes get short names. Long scopes get descriptive names.

---

## 8. Testing

### 8.1 Test File Organization

Tests live in sibling files, not inline.

- **Rust**: `#[cfg(test)] mod tests;` declaration → `tests.rs` sibling file
- **Gleam**: `error/tests.rs` alongside `error.rs`
- **Roc**: `solve_expr.rs` test file for `solve/` module
- **Go**: `*_test.go` in same package

**Rule**: `foo.rs` → `foo/tests.rs`. Never inline `#[cfg(test)]` module bodies in source files.

### 8.2 Test Quality

- **Lean 4**: Module docstring summarizing purpose; section docstrings for categories; "why is this desirable" not just "what is tested"
- **Gleam**: Snapshot testing via `insta::assert_snapshot!`
- **Roc**: `infer_eq(code, expected_type)` — parametric combinatorial tests
- **Go**: Table-driven tests

**Rule**: Tests explain intent. Each test says what behavior it verifies and why that behavior matters.

### 8.3 Test Matrix Coverage

Cover the combinatorial space, not just the happy path.

- **Gleam**: Loop over OS/program combinations, assert per case
- **Roc**: Parametric helpers: `infer_eq("5", "Num *")` across many inputs
- **Go**: Struct slices for table-driven tests

**Rule**: When behavior varies by input category, test all categories systematically.

---

## 9. Performance

### 9.1 Strategic Inlining

Inline hot-path accessors and small predicates. Don't over-inline.

- **Rust**: `#[inline]` on parser hot paths; `#[inline(always)]` rare and justified
- **Zig**: Implicit inlining via function size; `inline` keyword for explicit control
- **Swift**: LLVM handles most inlining; manual only for cross-module

**Rule**: `#[inline]` on 1-5 line accessors and cross-crate hot functions. Never on >20-line functions.

### 9.2 Cold Error Paths

Mark error construction as cold to keep hot paths in instruction cache.

- **Rust**: `#[cold]` on error factories, `#[track_caller]` on diagnostic creation
- **Zig**: Error paths branched out of hot loops

**Rule**: `#[cold]` on functions that construct diagnostics or handle error recovery.

### 9.3 Allocation Discipline

No allocation in hot loops. Pre-allocate, reuse, or arena-allocate.

- **Zig**: `ensureSpaceForInstructions()` pre-allocates before batch fills
- **Rust**: Arena pre-alloc with estimated capacity
- **Lean 4**: `free_dep_entries` — reuse freed slots instead of growing

**Rule**: `Vec::with_capacity()` with reasonable estimates. No `String::new()` per token.

### 9.4 Hash-Based Lookups Over Linear Scans

- **Zig**: `FxHashMap` for interning; sharded for concurrency
- **Rust**: `FxHashSet`, `FxIndexMap` — fast non-crypto hashing
- **Roc**: `NameMap` for error set resolution

**Rule**: If you're scanning a list to find something by key, use a hash map.

---

## 10. Documentation

### 10.1 Module-Level Purpose

Every file starts with `//!` explaining what this module does and why it exists.

- **Rust**: Comprehensive module docs with context and examples
- **Gleam**: Algorithm citations, ASCII diagrams in module docs
- **Lean 4**: Copyright header + module purpose
- **Zig**: `//!` with brief purpose statement

**Rule**: A developer reading only the module doc should understand the module's role in the compiler.

### 10.2 Comments Explain Why, Not What

- **Universal pattern**: All 10 compilers comment intent, not mechanics
- **Elm**: Design rationale for error message choices
- **Rust**: Multi-line comments for complex state machines (token ungluing, recovery)
- **Swift**: ARC optimization rationale documented in `ARCOptimization.md`

**Rule**: If the "what" isn't obvious from the code, refactor the code. Comments are for "why".

### 10.3 Spec References in Source

Link implementation to specification.

- **Go**: Error codes reference Go spec sections
- **Rust**: `rustc_lint_defs` links to RFCs

**Rule**: When implementing a spec clause, cite it: `// Spec: Clause 14.3 — operator precedence`.

---

## 11. Unsafe & Invariants

### 11.1 Unsafe Only at Boundaries

- **Rust**: FFI calls, lock management, pointer casting — never in business logic
- **Swift**: ARC operations wrapped in safe abstractions
- **Zig**: `@ptrCast`, `@intFromPtr` localized to specific functions

**Rule**: `unsafe` blocks exist at FFI boundaries and allocation internals. Never in type checking, parsing, or diagnostics.

### 11.2 Assert Invariants Early

- **Zig**: `assert(!map.contains(key))` before `putAssumeCapacity()`
- **Rust**: `debug_assert!` guards unsafe assumptions
- **Go**: `debug = false` const for compile-time elimination of debug checks
- **Lean 4**: Pattern match with `unreachable` for impossible branches

**Rule**: `debug_assert!` at function entry for preconditions. `unreachable!` with message for impossible branches.

---

## 12. Lint & Clippy Discipline

### 12.1 Never Bare `#[allow]`

- **Rust**: `tidy` tool enforces lint justification
- **Roc**: Extensive `#![warn(...)]` and `#![deny(...)]` lists (40+ rules)

**Rule**: Use `#[expect(clippy::lint, reason = "...")]` — never bare `#[allow(clippy::...)]`.

### 12.2 Deny Unsafe in Library Crates

- **Roc**: `#![deny(unsafe_code)]` in library crates
- **Gleam**: Clippy blocks all IO in `compiler-core` (pure functional core)

**Rule**: `#![deny(unsafe_code)]` in crates that don't need it. Separate "pure" crates from IO crates.

---

## Summary: The 15 Cardinal Rules

| # | Rule | Compilers |
|---|------|-----------|
| 1 | Accumulate errors; never bail on first | All 10 |
| 2 | Phase-specific error types | Elm, Rust, Go, Koka |
| 3 | Error codes are stable, permanent API | Go, TypeScript, Rust |
| 4 | Types interned by index; O(1) equality | Zig, Rust, Roc, Lean |
| 5 | Newtype all indices | Zig, Rust, Roc, TypeScript |
| 6 | Phases are one-way; no reaching back | Lean, Zig, Swift, Koka |
| 7 | File size < 500 lines; split by concern | Rust, Gleam, Lean |
| 8 | Private by default; escalate visibility | Rust, Gleam, Roc |
| 9 | Tests in sibling files, not inline | Rust, Gleam, Roc, Go |
| 10 | `#[inline]` on hot accessors only | Rust, Zig |
| 11 | `#[cold]` on error factories | Rust, Zig |
| 12 | No allocation in hot loops | Zig, Rust, Lean |
| 13 | Module docs explain purpose and why | All 10 |
| 14 | `debug_assert!` for preconditions | Rust, Zig, Go |
| 15 | `#[expect]` with reason, never bare `#[allow]` | Rust, Roc |
