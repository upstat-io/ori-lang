# Ori Compiler Roadmap — Execution Plan

## How to Use This Plan

### Prerequisites

Before starting:
1. Familiarize yourself with `CLAUDE.md` (language quick reference)
2. Familiarize yourself with `docs/ori_lang/0.1-alpha/spec/` (authoritative spec)
3. Ensure `./test-all.sh` passes (runs Rust tests, Ori interpreter tests, and LLVM tests)

### Execution Rules

1. **Follow section order strictly** — Dependencies are encoded in the numbering
2. **Within each section**, complete sections in order (X.1 → X.2 → ...)
3. **Within each section**, complete items top to bottom
4. **Each item requires**: Implementation → Tests → Documentation
5. **Do not skip sections** unless marked complete or explicitly skipped

### Item Structure

```markdown
- [ ] **Implement**: [description] — [spec reference]
  - [ ] **Write test**: `tests/spec/category/file.ori`
  - [ ] **Run test**: `ori test tests/spec/category/file.ori`
```

### Updating Progress

- Check boxes as items complete: `[ ]` → `[x]`
- Update `priority-and-tracking.md` section status
- Save after each update

### Style Rules

**No emojis or dates in roadmap files.** Use text instead:
- Instead of emoji status indicators, use text or mark checkboxes `[x]`
- Do not add date annotations like `(2026-02-04)` to task items
- Status values: `complete`, `in-progress`, `not-started` (text only)

If you encounter emojis or date annotations in roadmap files, remove them. The website parser and tooling expect plain text status values.

### Section File Frontmatter Format

Each section file uses YAML frontmatter for machine-parseable metadata. This enables the website to dynamically read roadmap data instead of hard-coding it.

```yaml
---
section: 1                           # int or string ("7A", "15B", "21A")
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

**Section status derivation:**
- All sections `complete` → section `complete`
- Any section `in-progress` or mix of statuses → section `in-progress`
- All sections `not-started` → section `not-started`

### Adding New Items

When adding new implementation items to the roadmap, consider creating a new section file if:

1. **Scope warrants separation** — The new work represents a distinct, cohesive unit (e.g., a new stdlib module, a major language feature)
2. **Section file is getting large** — If a section file exceeds ~150-200 items or ~400 lines, consider splitting
3. **Clear boundaries exist** — The new items have minimal dependencies on other items in the same section
4. **Different timeline** — The new work could reasonably be done independently of existing section items

**How to create a new section:**
1. Use letter suffixes for sub-sections within a tier (e.g., 7E, 15E)
2. Use the next number for entirely new sections (coordinate with tier structure)
3. Follow naming convention: `section-NN-descriptive-name.md` or `section-NNA-descriptive-name.md`
4. Update three files: new section file, `00-overview.md` tier table, `priority-and-tracking.md` status table
5. Keep sections focused — each section should have a clear goal and exit criteria

**When NOT to create a new section:**
- Small additions (1-5 items) that fit naturally into an existing section
- Items that are tightly coupled to existing section work
- Approved proposals that specify which section they belong to (follow the proposal)

---

## Bugs Found During Formatting Review (2026-02-02)

Bugs discovered during comprehensive formatting scenario review.

### Fixed in This Session

1. **For/Loop receiver parentheses** — `(for x in items yield x).fold(...)` was losing parentheses
   - Fixed in `compiler/ori_fmt/src/formatter/mod.rs` — added `For` and `Loop` to `needs_receiver_parens()`

2. **Lambda call target parentheses** — `(y -> y * 2)(x)` was losing parentheses
   - Fixed in `compiler/ori_fmt/src/formatter/helpers.rs` — added `emit_call_target_inline()` and `format_call_target()`

3. **Iterator source parentheses** — `for x in (for y in items yield y)` was losing parentheses
   - Fixed in `compiler/ori_fmt/src/formatter/helpers.rs` — added `emit_iter_inline()` and `format_iter()`

### Pre-existing Issues Discovered (Not Formatter Bugs)

These are parser/lexer issues discovered during formatting testing, not formatter bugs:

1. **`??` coalesce operator not implemented** — Token `DoubleQuestion` is lexed but parser never creates `BinaryOp::Coalesce`
   - Location: `compiler/ori_parse/src/grammar/expr.rs`
   - Status: Parser infrastructure exists but operator not wired up

2. **`!=` in for guards** — Parser error with `!=` in guard expressions
   - Example: `for x in items if x != 0 yield x` fails to parse
   - Location: `compiler/ori_parse/src/grammar/expr.rs`

3. **`by` in ranges for for loops** — `for x in 0..100 by 5` doesn't parse
   - Location: `compiler/ori_parse/src/grammar/expr.rs`
   - Status: `by` recognized but not integrated with for loop iteration

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

- [x] **`as` Conversion Syntax** — APPROVED → See Section 15.7
  - Proposal: `proposals/approved/as-conversion-proposal.md`
  - Removes special-case positional argument exception
  - Adds `As<T>`, `TryAs<T>` traits to prelude
  - Strict: `as` only for infallible, `as?` for fallible, explicit methods for lossy
  - **Affects**: Section 3 (Traits), Section 15 (Syntax)

### New Prelude Traits

- [ ] **Iterator Traits** — Formalize iteration with `Iterator`, `Iterable`, `Collect`
  - Proposal: `proposals/drafts/iterator-traits-proposal.md`
  - Enables user types in `for` loops
  - Formalizes `.map()`, `.filter()`, `.fold()` as trait extensions
  - **Affects**: Section 3 (Traits), Section 7 (Stdlib)

- [ ] **Debug Trait** — Separate from `Printable` for developer-facing output
  - Proposal: `proposals/drafts/debug-trait-proposal.md`
  - Derivable structural representation
  - Enables `dbg` function
  - **Affects**: Section 3 (Traits), Section 7 (Stdlib)

### New Prelude Functions

- [ ] **Developer Functions** — `todo`, `unreachable`, `dbg`
  - Proposal: `proposals/drafts/developer-functions-proposal.md`
  - `todo(reason:)` → `Never` — marks unfinished code
  - `unreachable(reason:)` → `Never` — marks impossible branches
  - `dbg(value:, label:)` → `T` — debug print that returns value
  - **Affects**: Section 7 (Stdlib)

### Behavioral Decisions (No Proposal Needed)

- **NaN comparisons panic** — Comparing NaN values panics (fits "bugs should be caught" philosophy)
- **Skip `AsRef`/`AsMut`** — Ori's value semantics don't need reference conversion traits
- **Skip `debug_assert*`** — Same behavior in all builds (consistency over conditional checks)

---

## Section Execution Order

### Tier 0: Syntax Foundation (BLOCKS ALL OTHER WORK)

| Order | Section | Document | Focus |
|-------|-------|----------|-------|
| 0 | Block Syntax | [section-00-parser.md](./section-00-parser.md) § 0.10 | `{ }` blocks replace `run()`/`match()`/`try()`, function-level `pre()`/`post()` contracts |

> **PRIORITY**: Block expression syntax (approved 2026-02-19) must be implemented before continuing any other roadmap work. Every feature built on the old `run()`/`match()`/`try()` syntax creates migration debt. Proposal: `proposals/approved/block-expression-syntax.md`. Migration script: `scripts/migrate_block_syntax.py`.

### Tier 1: Foundation (REQUIRED FIRST)

| Order | Section | Document | Focus |
|-------|-------|----------|-------|
| 1 | Section 1 | [section-01-type-system.md](./section-01-type-system.md) | Type annotations |
| 2 | Section 2 | [section-02-type-inference.md](./section-02-type-inference.md) | Type inference |
| 2.5 | Section 5.1 | [section-05-type-declarations.md](./section-05-type-declarations.md) § 5.1 | Struct types (needed for Section 3 tests) |
| 3 | Section 3 | [section-03-traits.md](./section-03-traits.md) | Trait system |
| 4 | Section 4 | [section-04-modules.md](./section-04-modules.md) | Module system |
| 5 | Section 5.2+ | [section-05-type-declarations.md](./section-05-type-declarations.md) § 5.2-5.8 | Sum types, newtypes, generics |

> **Note**: Section 5.1 (struct types) was moved earlier because Section 3 trait tests require user-defined types.

### Tier 2: Capabilities & Stdlib

| Order | Section | Document | Focus |
|-------|-------|----------|-------|
| 6 | Section 6 | [section-06-capabilities.md](./section-06-capabilities.md) | Effect tracking |
| 7A | Section 7A | [section-07A-core-builtins.md](./section-07A-core-builtins.md) | Core built-ins |
| 7B | Section 7B | [section-07B-option-result.md](./section-07B-option-result.md) | Option & Result |
| 7C | Section 7C | [section-07C-collections.md](./section-07C-collections.md) | Collections & iteration |
| 7D | Section 7D | [section-07D-stdlib-modules.md](./section-07D-stdlib-modules.md) | Stdlib modules |

### Tier 3: Core Patterns

| Order | Section | Document | Focus |
|-------|-------|----------|-------|
| 8 | Section 8 | [section-08-patterns.md](./section-08-patterns.md) | Pattern evaluation |
| 9 | Section 9 | [section-09-match.md](./section-09-match.md) | Match expressions |
| 10 | Section 10 | [section-10-control-flow.md](./section-10-control-flow.md) | Control flow |

### Tier 4: FFI & Interop

| Order | Section | Document | Focus |
|-------|-------|----------|-------|
| 11 | Section 11 | [section-11-ffi.md](./section-11-ffi.md) | Foreign functions |
| 12 | Section 12 | [section-12-variadic-functions.md](./section-12-variadic-functions.md) | Variable arguments |

### Tier 5: Language Completion

| Order | Section | Document | Focus |
|-------|-------|----------|-------|
| 13 | Section 13 | [section-13-conditional-compilation.md](./section-13-conditional-compilation.md) | Platform/feature support |
| 14 | Section 14 | [section-14-testing.md](./section-14-testing.md) | Testing framework |
| 15A | Section 15A | [section-15A-attributes-comments.md](./section-15A-attributes-comments.md) | Attributes & comments |
| 15B | Section 15B | [section-15B-function-syntax.md](./section-15B-function-syntax.md) | Function syntax |
| 15C | Section 15C | [section-15C-literals-operators.md](./section-15C-literals-operators.md) | Literals & operators |
| 15D | Section 15D | [section-15D-bindings-types.md](./section-15D-bindings-types.md) | Bindings & types |

### Tier 6: Async & Concurrency

| Order | Section | Document | Focus |
|-------|-------|----------|-------|
| 16 | Section 16 | [section-16-async.md](./section-16-async.md) | Async support |
| 17 | Section 17 | [section-17-concurrency.md](./section-17-concurrency.md) | Select, cancel, channels |

### Tier 7: Advanced Type System

| Order | Section | Document | Focus |
|-------|-------|----------|-------|
| 18 | Section 18 | [section-18-const-generics.md](./section-18-const-generics.md) | Const type params |
| 19 | Section 19 | [section-19-existential-types.md](./section-19-existential-types.md) | impl Trait |

### Tier 8: Advanced Features

| Order | Section | Document | Focus |
|-------|-------|----------|-------|
| 20 | Section 20 | [section-20-reflection.md](./section-20-reflection.md) | Runtime introspection |
| 21A | Section 21A | [section-21A-llvm.md](./section-21A-llvm.md) | LLVM backend |
| 21B | Section 21B | [section-21B-aot.md](./section-21B-aot.md) | AOT compilation |
| 22 | Section 22 | [section-22-tooling.md](./section-22-tooling.md) | Formatter, LSP, REPL |

---

## Running Tests

```bash
# Run ALL tests (Rust + Ori interpreter + LLVM backend)
./test-all.sh

# Individual test suites:
cargo t                          # Rust unit tests only
cargo st                         # Ori language tests (interpreter)
./llvm-test.sh                   # LLVM Rust unit tests
./docker/llvm/run.sh ori test tests/  # Ori language tests (LLVM)

# Specific category
cargo st tests/spec/types/
cargo st tests/spec/traits/

# Single file
cargo st tests/spec/types/primitives.ori
```

---

## Section Dependencies Quick Reference

> **NOTE**: Dependencies show minimum requirements to START a section. Tiers represent the
> recommended execution order for practical reasons (e.g., Section 18 only needs Sections 1-2
> but is in Tier 7 because const generics are an advanced feature better tackled after
> core language completion).

```
Can start immediately (no deps):
  - Section 1, 2
  - Section 5.1 (struct types) — no dependencies on other sections

After Section 2 (type inference):
  - Section 3 (traits) — implementation can start
  - NOTE: Section 3 TESTING requires Section 5.1 (struct types for impl tests)

After Section 3 (traits):
  - Section 4 (modules)
  - Section 6 (capabilities) — placed here to unblock Section 8 cache
  - Section 19 (existential types) [deferred to Tier 7]

After Section 4 (modules):
  - Section 5.2+ (sum types, newtypes, generics) — visibility needs modules

After Section 5 (type declarations):
  - Section 6 (capabilities) — if not already done
  - Section 7 (stdlib) — also requires Section 3

After Section 6 (capabilities):
  - Section 7 (stdlib)
  - Section 8-10 (core patterns) — Section 8 cache now unblocked
  - Section 11 (FFI) - needs Unsafe capability
  - Section 14 (testing) - uses capabilities for mocking
  - Section 16 (async)

After Section 8 (patterns):
  - Section 13 (conditional compilation)

After Section 11 (FFI):
  - Section 12 (variadics) - for C variadics
  - Section 20 (reflection)

After Section 16 (async):
  - Section 17 (concurrency)

After Sections 1-2 (type system):
  - Section 18 (const generics) [deferred to Tier 7]

After core complete (Sections 1-15):
  - Section 21 (codegen)
  - Section 22 (tooling)
```

---

## Source Plan Mapping

| New Section | V3 Source | Gap Source | Notes |
|-----------|-----------|------------|-------|
| 1: Type System | section-01 | — | |
| 2: Type Inference | section-02 | — | |
| 3: Traits | section-07 | — | |
| 4: Modules | section-08 | — | |
| 5: Type Declarations | section-06 | — | |
| 6: Capabilities | section-11 | — | **Swapped with Stdlib** |
| 7A-D: Stdlib | section-09 | — | **Split into 4 sub-sections** |
| 8: Patterns | section-03 | — | |
| 9: Match | section-04 | — | |
| 10: Control Flow | section-05 | — | |
| 11: FFI | — | section-01 | |
| 12: Variadic Functions | — | section-04 | |
| 13: Conditional Compilation | — | section-03 | |
| 14: Testing | section-10 | — | |
| 15A-D: Syntax Proposals | section-15.1-15.5 | — | **Split into 4 sub-sections** |
| 16: Async | section-12 | — | |
| 17: Concurrency | section-15.7 | section-05, section-06 | |
| 18: Const Generics | — | section-07 | |
| 19: Existential Types | — | section-08 | |
| 20: Reflection | — | section-09 | |
| 21A-B: Codegen | section-13 | — | **Split into LLVM + AOT** |
| 22: Tooling | section-14 | — | |
