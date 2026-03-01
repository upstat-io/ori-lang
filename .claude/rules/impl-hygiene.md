---
paths:
  - "**compiler**"
---

# Implementation Hygiene Rules

Not architecture (design decisions are made) and not code hygiene (surface style). About whether implementation faithfully realizes the architecture — tight joints, correct flow, no leaks.

## Phase Boundary Discipline

- **One-way data flow**: later phases never call back into earlier phases
- **No circular imports**: `ori_lexer` never imports `ori_parse`
- **Minimal boundary types**: pass only what next phase needs — `(tag, span)`, not `(tag, span, source_slice, metadata)`
- **Clean ownership transfer**: move at boundaries, borrow within phases; no unnecessary `.clone()` at transitions
- **No phase bleeding**: lexer doesn't parse, parser doesn't type-check, type checker doesn't codegen
- **Phase purity**: output depends only on input; no global mutable state, no side channels

## Data Flow

- **Zero-copy**: spans reference source by position, not owned slice; tokens carry `(tag, len)`, not string copies
- **Arena per phase**: temporaries freed when phase completes, no leakage to next phase
- **Interned values via opaque indices**: cross boundaries with `Name`, `ExprId`, `TypeId` — never raw `u32`
- **No allocation in hot token paths**: no `String::from()`, `Vec::new()`, `Box::new()` per token at lexer->parser boundary
- **Source text borrowed**: parser borrows `&str`; only final AST or error messages may own copies

## Error Handling at Boundaries

- **Accumulate, don't bail**: each phase collects all errors in one pass
- **Phase-scoped error types**: lexer errors != parse errors != type errors
- **Upstream errors propagated**: parser handles/propagates lexer errors, not swallows them; earlier errors take priority
- **Errors carry spans**: every error includes source position; spanless errors are bugs
- **Recovery is explicit**: enum state (`Recovery::Allowed | Forbidden`), not implicit booleans

## Type Discipline at Boundaries

- **Separate raw vs cooked types**: `RawTag` != `TokenKind`; each boundary has own type vocabulary
- **Newtypes for all IDs**: `ExprId`, `TypeId`, `TokenIndex` — not raw `u32`
- **Generic phase parameters**: `Module<Info, Defs>` pattern for untyped vs typed phases
- **Metadata separated from data**: comments/formatting/whitespace in sidecar (`ModuleExtra`), not interleaved with AST
- **No phase state in output types**: AST nodes carry structure + spans, not parser cursor or inference state

## Pass Composition

- **Each pass is IR -> IR**: no hidden inputs from global state
- **Explicit pass ordering**: dependencies documented and enforced
- **No shared mutable state between passes**: inter-pass communication via IR only
- **Boundary validation**: assert invariants before crossing to next phase
- **`#[cold]` on error paths**: error handling doesn't pollute hot-path instruction cache

## Registration Sync Points

- **Single source of truth**: when same fact (enum variant, error code, trait name) appears in multiple locations, one is source, others derived/validated
- **No manual mirroring**: centralize via shared method (`from_str()`, `all()`, iterator) rather than parallel lists
- **Compile-time or test-time enforcement**: when centralization isn't possible, add test iterating source-of-truth list
- **Flag drift as finding**: new variant added in one location but missing from parallel location = **DRIFT** finding

## Gap Detection

- **Cross-phase capability mismatch = GAP**: one phase supports a feature, another blocks it (e.g., typeck handles `.0` but parser rejects it)
- **Never silently work around a gap**: flag immediately; workarounds hide gaps from roadmap
- **Audit across phases**: when adding capability to any phase, verify full pipeline: lexer -> parser -> typeck -> evaluator -> codegen
- **Track with specificity**: which phase blocks, which already support, what user sees

## File Organization

- **500-line limit**: source files (excluding tests); exceeding = **BLOAT** finding
- **Single responsibility per file**: closures + operators + construction + dispatch in one file = 6 jobs; split
- **Submodule extraction over monolithic growth**: logical group exceeding ~200 lines -> sibling submodule; parent `mod.rs` = dispatch hub
- **File names reflect content**: `closures.rs` not closure logic in `mod.rs`
- **Hierarchy matches phase structure**: 3 passes = 3 submodules, not one file with comment-separated sections
- **Split when touching**: touching a file over 500 lines without splitting = finding

## Cascading Fix Detection

- **Whack-a-mole = architectural issue**: fix at one callsite moves failure to next layer -> STOP; shared assumption is wrong across pipeline
- **Three-strike rule**: same logical fix at 3+ independent callsites = missing abstraction or violated boundary contract; fix belongs at boundary, not at every consumer
- **Present options**: on cascading fix, present (1) architectural issue, (2) why per-site patches won't scale, (3) 2-3 options with trade-offs

## Phase-Specific Purity

- **Lexer**: stateless scanning; produces `(tag, len)`; no keyword judgment, name resolution, or nesting context beyond tokenization needs
- **Parser**: syntax only; builds AST from tokens; no name resolution, type checking, or semantic validation
- **Type Checker**: consumes AST, produces typed IR; no re-parsing, no codegen; errors via diagnostic infrastructure
- **Optimization Passes**: reads IR, produces transformed IR; no reaching into other passes' state; analysis is pass-local
