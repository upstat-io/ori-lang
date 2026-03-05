---
paths:
  - "*.rs"
---

# Hygiene Rules

## Phase Boundaries

- **One-way data flow**: later phases never call back into earlier phases
- **No circular imports**: `ori_lexer` never imports `ori_parse`
- **Minimal boundary types**: pass only what next phase needs
- **Clean ownership transfer**: move at boundaries, borrow within phases; no unnecessary `.clone()`
- **No phase bleeding**: lexer doesn't parse, parser doesn't type-check, type checker doesn't codegen
- **Phase purity**: output depends only on input; no global mutable state, no side channels
- **Delayed action queues**: when forward references needed, enqueue work — don't loop or backtrack

### Phase-Specific Purity

- **Lexer**: stateless scanning; produces `(tag, len)`
- **Parser**: syntax only; builds AST from tokens; no name resolution or semantic validation
- **Type Checker**: consumes AST, produces typed IR; no re-parsing, no codegen
- **Optimization Passes**: reads IR, produces transformed IR; analysis is pass-local

## Data Flow

- **Zero-copy**: spans reference source by position; tokens carry `(tag, len)`, not string copies
- **Arena per phase**: temporaries freed when phase completes, no leakage to next phase
- **Interned values via opaque indices**: cross boundaries with `Name`, `ExprId`, `TypeId` — never raw `u32`
- **No allocation in hot paths**: no `String::from()`, `Vec::new()`, `Box::new()` per token
- **Source text borrowed**: parser borrows `&str`; only final AST or error messages may own copies
- **Hash lookups over linear scans**: if scanning a list by key, use a hash map
- **Pre-allocate**: `Vec::with_capacity()` with reasonable estimates

## Error Handling

- **Accumulate, don't bail**: each phase collects all errors in one pass
- **Phase-scoped error types**: lexer errors != parse errors != type errors
- **Upstream errors propagated**: earlier errors take priority; parser propagates lexer errors
- **Errors carry spans**: every error includes source position; spanless errors are bugs
- **Recovery is explicit**: `Recovery::Allowed | Forbidden` enum, not implicit booleans
- **Structured construction**: `Diagnostic::error(code).with_message(...).with_label(...)` — never `format!()` strings
- **Expected context**: every "expected X, got Y" MUST include WHY — annotation, return type, parameter
- **Deduplication**: hash emitted diagnostics; suppress follow-on errors when earlier error explains
- **Edit-distance suggestions**: Damerau-Levenshtein for "did you mean?" — threshold: `distance <= max(2, name.len() / 3)`
- **Error codes are stable API**: once assigned, never reuse or change meaning

## Type Discipline

- **Separate raw vs cooked types**: `RawTag` != `TokenKind`; each boundary has own type vocabulary
- **Newtypes for all IDs**: `ExprId`, `TypeId`, `TokenIndex` — not raw `u32`
- **Generic phase parameters**: `Module<Info, Defs>` pattern for untyped vs typed phases
- **Metadata separated from data**: comments/whitespace in sidecar (`ModuleExtra`), not in AST
- **No phase state in output types**: AST nodes carry structure + spans, not parser cursor or inference state
- **Pre-compute metadata at interning**: `TypeFlags` computed once — O(1) queries, never re-walk the type tree

## Pass Composition

- **Each pass is IR -> IR**: no hidden inputs from global state
- **Explicit pass ordering**: dependencies documented and enforced
- **No shared mutable state between passes**: inter-pass communication via IR only
- **Boundary validation**: `debug_assert!` invariants before crossing to next phase

## Registration Sync Points

- **Single source of truth**: one location is canonical, others derived/validated
- **No manual mirroring**: centralize via `from_str()`, `all()`, iterator — not parallel lists
- **Compile-time or test-time enforcement**: add test iterating source-of-truth list
- **Flag drift as finding**: new variant in one location but missing from parallel = **DRIFT**

## Gap Detection

- **Cross-phase capability mismatch = GAP**: one phase supports a feature, another blocks it
- **Never silently work around a gap**: flag immediately
- **Audit across phases**: when adding capability, verify full pipeline: lexer → parser → typeck → eval → codegen

## Cascading Fix Detection

- **Whack-a-mole = architectural issue**: fix at one callsite moves failure to next → STOP
- **Three-strike rule**: same fix at 3+ callsites = missing abstraction; fix at boundary
- **Present options**: (1) architectural issue, (2) why per-site patches won't scale, (3) 2-3 options

## File Organization

- **500-line limit**: source files (excluding tests); exceeding = **BLOAT** finding
- **Single responsibility per file**: split when a file serves multiple jobs
- **Submodule extraction**: logical group exceeding ~200 lines → sibling submodule; parent `mod.rs` = dispatch hub
- **File names reflect content**: `closures.rs` not closure logic in `mod.rs`
- **Split when touching**: touching a file over 500 lines without splitting = finding
- **Tests in sibling `tests.rs`**: `#[cfg(test)] mod tests;` declaration only — body in sibling file

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

- `lib.rs` is an **index**: `//!` doc, `mod` declarations, `pub use` re-exports — no function bodies
- `mod.rs` **dispatches**: routes to submodules, holds shared private items
- Leaf files **implement**: actual logic lives here

## Impl Block Method Ordering

1. **Constructors**: `new`, `with_*`, `from_*`, factory methods
2. **Accessors**: getters, `as_*` (cheap ref conversions)
3. **Predicates**: `is_*`, `has_*`, `can_*`, `contains`
4. **Public operations**: the main thing this type does
5. **Conversion/consumption**: `into_*`, `to_*`
6. **Private helpers**: in call-order grouping, not alphabetical

Within each group: pub before pub(crate) before private (loose).

## Struct/Enum Field Ordering

1. Primary data (core state)
2. Secondary/derived data
3. Configuration/options
4. Flags/booleans last

Inline comments on struct fields when purpose isn't obvious.

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

## Visibility

- Private by default; minimize pub surface
- `pub(crate)` for cross-module internal use
- No dead pub items; no dead code

## Comments

- `//!` module doc on every file; `///` on all `pub` items
- Comment WHY, not WHAT; `debug_assert!` for preconditions
- No decorative banners (`// ===`, `// ---`, `// ***`)
- No comments restating code, no commented-out code, no bare `// TODO`
- Section labels in large enums/matches: plain `// Section name`
- Spec references: `// Spec: Clause 14.3 — operator precedence`

## Derives

- **Derive** when impl is standard (field-by-field equality, hash, debug)
- **Manual** only when behavior differs — articulate WHY

## Lint Discipline

- `#[expect(clippy::lint, reason = "...")]` — never bare `#[allow(clippy::...)]`
- `#![deny(unsafe_code)]` in pure crates: `ori_ir`, `ori_diagnostic`, `ori_types`, `ori_eval`, `ori_patterns`, `ori_parse`, `ori_lexer`
- `unsafe` allowed only in: `ori_llvm`, `ori_rt`, `oric` (FFI, LLVM bindings, runtime)

## Performance Annotations

- `#[cold]` on error factory functions
- `#[inline]` only on 1-5 line accessors and cross-crate hot functions; never on >20-line functions
- Hot-path structs get compile-time size assertions: `const _: () = assert!(size_of::<T>() == N);`

## Style

- Functions < 100 lines (target < 50; dispatch tables exempt)
- Consistent patterns across similar code within same file
- No dead/commented-out code
