# Ori Compiler Roadmap — Execution Plan

## How to Use This Plan

### Prerequisites

Before starting:
1. Familiarize yourself with `CLAUDE.md` (language quick reference)
2. Familiarize yourself with `docs/ori_lang/0.1-alpha/spec/` (authoritative spec)
3. Ensure `./test-all` passes (runs Rust tests, Ori interpreter tests, and LLVM tests)

### Execution Rules

1. **Follow phase order strictly** — Dependencies are encoded in the numbering
2. **Within each phase**, complete sections in order (X.1 → X.2 → ...)
3. **Within each section**, complete items top to bottom
4. **Each item requires**: Implementation → Tests → Documentation
5. **Do not skip phases** unless marked complete or explicitly skipped

### Item Structure

```markdown
- [ ] **Implement**: [description] — [spec reference]
  - [ ] **Write test**: `tests/spec/category/file.ori`
  - [ ] **Run test**: `ori test tests/spec/category/file.ori`
```

### Updating Progress

- Check boxes as items complete: `[ ]` → `[x]`
- Update `priority-and-tracking.md` phase status
- Save after each update

### Phase File Frontmatter Format

Each phase file uses YAML frontmatter for machine-parseable metadata. This enables the website to dynamically read roadmap data instead of hard-coding it.

```yaml
---
phase: 1                           # int or string ("7A", "15B", "21A")
title: Type System Foundation
status: in-progress                # not-started | in-progress | complete
tier: 1                            # 1-8 (see tier descriptions below)
goal: Fix type checking to properly use type annotations
spec:                              # string or array (optional)
  - spec/06-types.md
sections:
  - id: "1.1"                      # matches ## X.Y headers in body
    title: Primitive Types
    status: complete               # not-started | in-progress | complete
  - id: "1.2"
    title: Type Annotations
    status: in-progress
---
```

**Status values:**
- `not-started` — No checkboxes completed in section
- `in-progress` — Some checkboxes completed, some pending
- `complete` — All checkboxes completed

**Determining section status from body:**
- Analyze checkbox patterns (`[x]` vs `[ ]`) under each `## X.Y` header
- All `[x]` → `complete`
- Mix of `[x]` and `[ ]` → `in-progress`
- All `[ ]` → `not-started`

**Phase status derivation:**
- All sections `complete` → phase `complete`
- Any section `in-progress` or mix of statuses → phase `in-progress`
- All sections `not-started` → phase `not-started`

### Adding New Items

When adding new implementation items to the roadmap, consider creating a new phase file if:

1. **Scope warrants separation** — The new work represents a distinct, cohesive unit (e.g., a new stdlib module, a major language feature)
2. **Phase file is getting large** — If a phase file exceeds ~150-200 items or ~400 lines, consider splitting
3. **Clear boundaries exist** — The new items have minimal dependencies on other items in the same phase
4. **Different timeline** — The new work could reasonably be done independently of existing phase items

**How to create a new phase:**
1. Use letter suffixes for sub-phases within a tier (e.g., 7E, 15E)
2. Use the next number for entirely new phases (coordinate with tier structure)
3. Follow naming convention: `phase-NN-descriptive-name.md` or `phase-NNA-descriptive-name.md`
4. Update three files: new phase file, `00-overview.md` tier table, `priority-and-tracking.md` status table
5. Keep phases focused — each phase should have a clear goal and exit criteria

**When NOT to create a new phase:**
- Small additions (1-5 items) that fit naturally into an existing phase
- Items that are tightly coupled to existing phase work
- Approved proposals that specify which phase they belong to (follow the proposal)

---

## Spec Updates Required

Approved proposals that need spec documentation:

- [ ] **Clone Trait** — Add trait definition to `spec/06-types.md` or `spec/08-declarations.md`
  - Proposal: `proposals/approved/clone-trait-proposal.md`
  - Implementation: Already in `oric/src/typeck/derives/mod.rs`
  - Missing: Spec definition of `trait Clone { @clone (self) -> Self }`

- [ ] **Zipper Data Structures** — Add to stdlib roadmap
  - Proposal: `proposals/drafts/zipper-stdlib-proposal.md`
  - Covers: `Zipper<T>`, `TreeZipper<T>` for ARC-safe bidirectional traversal

---

## Draft Proposals (Prelude Enhancements)

New proposals from Rust prelude comparison (2026-01-27). These enhance Ori's prelude with commonly-needed functionality.

### Syntax Changes

- [x] **`as` Conversion Syntax** — APPROVED → See Phase 15.7
  - Proposal: `proposals/approved/as-conversion-proposal.md`
  - Removes special-case positional argument exception
  - Adds `As<T>`, `TryAs<T>` traits to prelude
  - Strict: `as` only for infallible, `as?` for fallible, explicit methods for lossy
  - **Affects**: Phase 3 (Traits), Phase 15 (Syntax)

### New Prelude Traits

- [ ] **Iterator Traits** — Formalize iteration with `Iterator`, `Iterable`, `Collect`
  - Proposal: `proposals/drafts/iterator-traits-proposal.md`
  - Enables user types in `for` loops
  - Formalizes `.map()`, `.filter()`, `.fold()` as trait extensions
  - **Affects**: Phase 3 (Traits), Phase 7 (Stdlib)

- [ ] **Debug Trait** — Separate from `Printable` for developer-facing output
  - Proposal: `proposals/drafts/debug-trait-proposal.md`
  - Derivable structural representation
  - Enables `dbg` function
  - **Affects**: Phase 3 (Traits), Phase 7 (Stdlib)

### New Prelude Functions

- [ ] **Developer Functions** — `todo`, `unreachable`, `dbg`
  - Proposal: `proposals/drafts/developer-functions-proposal.md`
  - `todo(reason:)` → `Never` — marks unfinished code
  - `unreachable(reason:)` → `Never` — marks impossible branches
  - `dbg(value:, label:)` → `T` — debug print that returns value
  - **Affects**: Phase 7 (Stdlib)

### Behavioral Decisions (No Proposal Needed)

- **NaN comparisons panic** — Comparing NaN values panics (fits "bugs should be caught" philosophy)
- **Skip `AsRef`/`AsMut`** — Ori's value semantics don't need reference conversion traits
- **Skip `debug_assert*`** — Same behavior in all builds (consistency over conditional checks)

---

## Phase Execution Order

### Tier 1: Foundation (REQUIRED FIRST)

| Order | Phase | Document | Focus |
|-------|-------|----------|-------|
| 1 | Phase 1 | [phase-01-type-system.md](./phase-01-type-system.md) | Type annotations |
| 2 | Phase 2 | [phase-02-type-inference.md](./phase-02-type-inference.md) | Type inference |
| 2.5 | Phase 5.1 | [phase-05-type-declarations.md](./phase-05-type-declarations.md) § 5.1 | Struct types (needed for Phase 3 tests) |
| 3 | Phase 3 | [phase-03-traits.md](./phase-03-traits.md) | Trait system |
| 4 | Phase 4 | [phase-04-modules.md](./phase-04-modules.md) | Module system |
| 5 | Phase 5.2+ | [phase-05-type-declarations.md](./phase-05-type-declarations.md) § 5.2-5.8 | Sum types, newtypes, generics |

> **Note**: Phase 5.1 (struct types) was moved earlier because Phase 3 trait tests require user-defined types.

### Tier 2: Capabilities & Stdlib

| Order | Phase | Document | Focus |
|-------|-------|----------|-------|
| 6 | Phase 6 | [phase-06-capabilities.md](./phase-06-capabilities.md) | Effect tracking |
| 7A | Phase 7A | [phase-07A-core-builtins.md](./phase-07A-core-builtins.md) | Core built-ins |
| 7B | Phase 7B | [phase-07B-option-result.md](./phase-07B-option-result.md) | Option & Result |
| 7C | Phase 7C | [phase-07C-collections.md](./phase-07C-collections.md) | Collections & iteration |
| 7D | Phase 7D | [phase-07D-stdlib-modules.md](./phase-07D-stdlib-modules.md) | Stdlib modules |

### Tier 3: Core Patterns

| Order | Phase | Document | Focus |
|-------|-------|----------|-------|
| 8 | Phase 8 | [phase-08-patterns.md](./phase-08-patterns.md) | Pattern evaluation |
| 9 | Phase 9 | [phase-09-match.md](./phase-09-match.md) | Match expressions |
| 10 | Phase 10 | [phase-10-control-flow.md](./phase-10-control-flow.md) | Control flow |

### Tier 4: FFI & Interop

| Order | Phase | Document | Focus |
|-------|-------|----------|-------|
| 11 | Phase 11 | [phase-11-ffi.md](./phase-11-ffi.md) | Foreign functions |
| 12 | Phase 12 | [phase-12-variadic-functions.md](./phase-12-variadic-functions.md) | Variable arguments |

### Tier 5: Language Completion

| Order | Phase | Document | Focus |
|-------|-------|----------|-------|
| 13 | Phase 13 | [phase-13-conditional-compilation.md](./phase-13-conditional-compilation.md) | Platform/feature support |
| 14 | Phase 14 | [phase-14-testing.md](./phase-14-testing.md) | Testing framework |
| 15A | Phase 15A | [phase-15A-attributes-comments.md](./phase-15A-attributes-comments.md) | Attributes & comments |
| 15B | Phase 15B | [phase-15B-function-syntax.md](./phase-15B-function-syntax.md) | Function syntax |
| 15C | Phase 15C | [phase-15C-literals-operators.md](./phase-15C-literals-operators.md) | Literals & operators |
| 15D | Phase 15D | [phase-15D-bindings-types.md](./phase-15D-bindings-types.md) | Bindings & types |

### Tier 6: Async & Concurrency

| Order | Phase | Document | Focus |
|-------|-------|----------|-------|
| 16 | Phase 16 | [phase-16-async.md](./phase-16-async.md) | Async support |
| 17 | Phase 17 | [phase-17-concurrency.md](./phase-17-concurrency.md) | Select, cancel, channels |

### Tier 7: Advanced Type System

| Order | Phase | Document | Focus |
|-------|-------|----------|-------|
| 18 | Phase 18 | [phase-18-const-generics.md](./phase-18-const-generics.md) | Const type params |
| 19 | Phase 19 | [phase-19-existential-types.md](./phase-19-existential-types.md) | impl Trait |

### Tier 8: Advanced Features

| Order | Phase | Document | Focus |
|-------|-------|----------|-------|
| 20 | Phase 20 | [phase-20-reflection.md](./phase-20-reflection.md) | Runtime introspection |
| 21A | Phase 21A | [phase-21A-llvm.md](./phase-21A-llvm.md) | LLVM backend |
| 21B | Phase 21B | [phase-21B-aot.md](./phase-21B-aot.md) | AOT compilation |
| 22 | Phase 22 | [phase-22-tooling.md](./phase-22-tooling.md) | Formatter, LSP, REPL |

---

## Running Tests

```bash
# Run ALL tests (Rust + Ori interpreter + LLVM backend)
./test-all

# Individual test suites:
cargo t                          # Rust unit tests only
cargo st                         # Ori language tests (interpreter)
./llvm-test                      # LLVM Rust unit tests
./docker/llvm/run.sh ori test tests/  # Ori language tests (LLVM)

# Specific category
cargo st tests/spec/types/
cargo st tests/spec/traits/

# Single file
cargo st tests/spec/types/primitives.ori
```

---

## Phase Dependencies Quick Reference

> **NOTE**: Dependencies show minimum requirements to START a phase. Tiers represent the
> recommended execution order for practical reasons (e.g., Phase 18 only needs Phases 1-2
> but is in Tier 7 because const generics are an advanced feature better tackled after
> core language completion).

```
Can start immediately (no deps):
  - Phase 1, 2
  - Phase 5.1 (struct types) — no dependencies on other phases

After Phase 2 (type inference):
  - Phase 3 (traits) — implementation can start
  - NOTE: Phase 3 TESTING requires Phase 5.1 (struct types for impl tests)

After Phase 3 (traits):
  - Phase 4 (modules)
  - Phase 6 (capabilities) — placed here to unblock Phase 8 cache
  - Phase 19 (existential types) [deferred to Tier 7]

After Phase 4 (modules):
  - Phase 5.2+ (sum types, newtypes, generics) — visibility needs modules

After Phase 5 (type declarations):
  - Phase 6 (capabilities) — if not already done
  - Phase 7 (stdlib) — also requires Phase 3

After Phase 6 (capabilities):
  - Phase 7 (stdlib)
  - Phase 8-10 (core patterns) — Phase 8 cache now unblocked
  - Phase 11 (FFI) - needs Unsafe capability
  - Phase 14 (testing) - uses capabilities for mocking
  - Phase 16 (async)

After Phase 8 (patterns):
  - Phase 13 (conditional compilation)

After Phase 11 (FFI):
  - Phase 12 (variadics) - for C variadics
  - Phase 20 (reflection)

After Phase 16 (async):
  - Phase 17 (concurrency)

After Phases 1-2 (type system):
  - Phase 18 (const generics) [deferred to Tier 7]

After core complete (Phases 1-15):
  - Phase 21 (codegen)
  - Phase 22 (tooling)
```

---

## Source Plan Mapping

| New Phase | V3 Source | Gap Source | Notes |
|-----------|-----------|------------|-------|
| 1: Type System | phase-01 | — | |
| 2: Type Inference | phase-02 | — | |
| 3: Traits | phase-07 | — | |
| 4: Modules | phase-08 | — | |
| 5: Type Declarations | phase-06 | — | |
| 6: Capabilities | phase-11 | — | **Swapped with Stdlib** |
| 7A-D: Stdlib | phase-09 | — | **Split into 4 sub-phases** |
| 8: Patterns | phase-03 | — | |
| 9: Match | phase-04 | — | |
| 10: Control Flow | phase-05 | — | |
| 11: FFI | — | phase-01 | |
| 12: Variadic Functions | — | phase-04 | |
| 13: Conditional Compilation | — | phase-03 | |
| 14: Testing | phase-10 | — | |
| 15A-D: Syntax Proposals | phase-15.1-15.5 | — | **Split into 4 sub-phases** |
| 16: Async | phase-12 | — | |
| 17: Concurrency | phase-15.7 | phase-05, phase-06 | |
| 18: Const Generics | — | phase-07 | |
| 19: Existential Types | — | phase-08 | |
| 20: Reflection | — | phase-09 | |
| 21A-B: Codegen | phase-13 | — | **Split into LLVM + AOT** |
| 22: Tooling | phase-14 | — | |
