---
paths:
  - "**/compiler/**"
---

# Implementation Hygiene Rules

**Implementation hygiene is NOT architecture** (design decisions are made) **and NOT code hygiene** (surface style). It's about whether the implementation faithfully and cleanly realizes the architecture — tight joints, correct flow, no leaks, clear structure.

## Phase Boundary Discipline

- **One-way data flow**: Data flows downstream only. Later phases never call back into earlier phases.
- **No circular imports**: Phase crates/modules must not import from downstream. `ori_lexer` never imports `ori_parse`.
- **Minimal boundary types**: Only pass what the next phase needs. Tokens: `(tag, span)`, not `(tag, span, source_slice, metadata)`.
- **Clean ownership transfer**: Move at boundaries, borrow within phases. No unnecessary `.clone()` at phase transitions.
- **No phase bleeding**: Lexer doesn't parse, parser doesn't type-check, type checker doesn't codegen. Each phase does exactly its job.
- **Phase purity enforcement**: A phase's output depends only on its input. No global mutable state, no side channels.

## Data Flow

- **Zero-copy where possible**: Spans reference source by position, not by owned slice. Tokens carry `(tag, len)` or `(tag, start, end)`, not string copies.
- **Arena per phase**: Temporary allocations freed when phase completes. No leakage to next phase.
- **Interned values via opaque indices**: Cross boundaries with `Name`, `ExprId`, `TypeId` — never raw `u32` or direct pointers to interned data.
- **No allocation in hot token paths**: Lexer→parser boundary is the hottest path. No `String::from()`, no `Vec::new()`, no `Box::new()` per token.
- **Source text borrowed**: Parser borrows source `&str`; only the final AST or error messages may need owned copies.

## Error Handling at Boundaries

- **Accumulate, don't bail**: Each phase collects all errors in one pass. User sees every problem, not just the first.
- **Phase-scoped error types**: Lexer errors ≠ parse errors ≠ type errors. Each phase defines errors relevant to its work.
- **Upstream errors propagated**: Parser must handle/propagate lexer errors, not swallow them. Earlier phase errors take priority.
- **Errors carry spans**: Every error includes source position. Errors without location are bugs.
- **Recovery is explicit**: Use enum state (`Recovery::Allowed | Forbidden`), not implicit boolean flags.

## Type Discipline at Boundaries

- **Separate raw vs cooked types**: Raw lexer output (`RawTag`) ≠ parser-ready tokens (`TokenKind`). Each boundary has its own type vocabulary.
- **Newtypes for all IDs**: `ExprId`, `TypeId`, `TokenIndex` — not raw `u32`. Prevents cross-boundary ID confusion.
- **Generic phase parameters**: Use `Module<Info, Defs>` pattern — same structure, different type parameters for untyped vs typed phases.
- **Metadata separated from data**: Comments, formatting info, whitespace live in a sidecar (`ModuleExtra`), not interleaved with AST nodes.
- **No phase state in output types**: AST nodes carry syntactic structure + spans. No parser cursor, no lookahead buffer, no inference state.

## Pass Composition

- **Each pass is IR → IR**: Takes input IR, produces output IR. No hidden inputs from global state.
- **Explicit pass ordering**: Dependencies between passes are documented and enforced, not implicit.
- **No shared mutable state between passes**: Each pass owns its working data. Inter-pass communication is via the IR itself.
- **Boundary validation**: Assert invariants before crossing to next phase (e.g., all tokens consumed, all types resolved).
- **`#[cold]` on error paths**: Error handling code doesn't pollute hot-path instruction cache.

## Registration Sync Points

- **Single source of truth**: When the same logical fact (enum variant, error code, operator mapping, trait name) must appear in multiple locations, one location is the source and others are derived or validated against it.
- **No manual mirroring**: If two match arms, arrays, or maps must list the same set of variants, centralize via a shared method (`from_str()`, `all()`, iterator) rather than maintaining parallel lists. Failing: `ErrorCode` enum + `parse_error_code()` + `DOCS` array all listing codes independently.
- **Compile-time or test-time enforcement**: When centralization isn't possible (e.g., docs files that may not exist yet), add a test that iterates the source-of-truth list and checks each derived location for completeness.
- **Flag drift as a finding**: When a new variant, mapping, or registration is added in one location but missing from a parallel location, that's a **DRIFT** finding — the plumbing equivalent of a phase leak, but for registration data instead of control flow.

## Gap Detection

- **Cross-phase capability mismatch**: When one phase supports a feature but another blocks it, that's a **GAP** finding. Example: type checker and evaluator handle numeric field access `.0` but the parser rejects it. Gaps are invisible to users and to the roadmap — they look like "not implemented" when really it's "partially implemented with a bottleneck."
- **Never silently work around a gap**: If a feature doesn't work end-to-end, don't restructure code to avoid the broken path. Flag it immediately. A workaround hides the gap from the roadmap and from future implementers.
- **Audit across phases**: When adding a new capability to any phase, verify the full pipeline: lexer → parser → type checker → evaluator → codegen. A feature that works in isolation but fails end-to-end is a gap.
- **Track with specificity**: A gap finding must name: (1) which phase blocks, (2) which phases already support, (3) what the user-visible symptom is. Vague "doesn't work" is not a finding.

## File Organization

- **500-line production file limit**: Source files (excluding tests) should stay under 500 lines. Files over this threshold obscure internal boundaries and make hygiene auditing harder. When a file exceeds 500 lines, it's a **BLOAT** finding.
- **Single responsibility per file**: Each file should have one clear responsibility. A file containing closures, operators, construction, deconstruction, runtime calls, and method dispatch is doing 6 jobs — split it into focused submodules.
- **Submodule extraction over monolithic growth**: When a logical group of functions (closures, operators, construction) exceeds ~200 lines within a file, extract it into a sibling submodule. The parent `mod.rs` should be a dispatch hub, not a megafile.
- **File names reflect content**: Module names should describe what the file does, not just where it sits in the hierarchy. `closures.rs` is better than putting closure logic in `mod.rs`.
- **Hierarchy matches phase structure**: Directory structure should mirror the logical phase/pass structure. If a crate has 3 passes, there should be 3 submodules — not one file with 3 sections separated by comments.
- **Split when touching**: Per CLAUDE.md, when touching a file already over 500 lines, take the opportunity to split it. Not splitting when touching is itself a finding.

## Cascading Fix Detection (Architectural Smell)

- **Whack-a-mole = architectural issue**: When fixing a bug at one callsite moves the failure to the next layer, STOP. Do not patch the next callsite. The pattern of "fix here → break there → fix there → break elsewhere" means a shared assumption is wrong across the pipeline. Diagnose the assumption, not the symptoms.
- **Three-strike rule**: If the same logical fix must be applied at 3+ independent callsites, it's not a series of bugs — it's a missing abstraction or a violated boundary contract. The fix belongs at the boundary where the invariant should be established, not at every consumer.
- **Present options to the user**: When you detect a cascading fix, stop and present: (1) what the architectural issue is, (2) why per-site patches won't scale, (3) 2-3 options at different levels (boundary fix, abstraction, workaround) with trade-offs. Let the user choose the approach.

## Phase-Specific Purity

**Lexer**: Stateless scanning. Produces structural facts (`tag`, `len`). Does NOT judge keywords, resolve names, or track nesting context beyond what's needed for tokenization.

**Parser**: Syntax only. Builds AST from tokens. Does NOT resolve names, check types, or validate semantics. Grammar-driven: each parse function corresponds to a grammar rule.

**Type Checker**: Consumes AST, produces typed IR. Does NOT re-parse, does not codegen. Errors accumulated via diagnostic infrastructure.

**Optimization Passes**: Each pass reads IR, produces transformed IR. No pass reaches into another pass's internal state. Loop/branch analysis is pass-local.
