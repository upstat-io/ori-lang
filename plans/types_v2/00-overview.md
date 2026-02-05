# Types 2.0 Implementation Plan

> **ROADMAP**: Replaces `plans/roadmap/section-02-types.md`
> **Best-of-Breed Type System Architecture** — Combining innovations from Rust, Go, Zig, TypeScript, Gleam, Elm, and Roc

## Design Philosophy

Based on deep analysis of 7 production-grade compiler type systems (~100,000+ lines of type checker code), this plan synthesizes the best patterns into a novel architecture for Ori's type system:

1. **Unified Pool** — Zig-style single pool where types, values, and schemes are all 32-bit indices
2. **Pre-Computed Flags** — Rust-style TypeFlags cached at interning for O(1) queries
3. **Link-Based Unification** — Gleam/Elm-style union-find with path compression
4. **Rank-Based Generalization** — Elm/Roc-style correct let-polymorphism
5. **Context-Aware Errors** — Elm-quality error messages with rich context tracking
6. **Structure-of-Arrays** — Roc-style cache-friendly layout for bulk operations

The goal is to create a type system that is:
- **Dramatically more memory efficient** (~5 bytes per type vs ~40 bytes)
- **Faster type equality** (O(1) index comparison vs O(n) structural)
- **Faster unification** (O(α(n)) link-based vs O(n) substitution maps)
- **Industry-leading error messages** (Elm quality)
- **Salsa-compatible by design** (incremental compilation)

---

## CRITICAL: Complete Replacement

**This is NOT an incremental improvement. This is a complete ground-up rewrite.**

- **No remnants** of the old system
- **No backwards compatibility**
- **No migration shims**
- **Full sweeping replacement**

### Files to DELETE (ori_types - 3,105 lines)

```
compiler/ori_types/src/
├── lib.rs           # DELETE
├── core.rs          # DELETE
├── data.rs          # DELETE
├── context.rs       # DELETE
├── type_interner.rs # DELETE
├── env.rs           # DELETE
├── traverse.rs      # DELETE
└── error.rs         # DELETE
```

### Files to DELETE (ori_typeck - 14,186 lines)

```
compiler/ori_typeck/src/
├── lib.rs           # DELETE
├── shared.rs        # DELETE
├── suggest.rs       # DELETE
├── checker/         # DELETE (22 files)
├── infer/           # DELETE (48 files)
├── registry/        # DELETE (11 files)
├── derives/         # DELETE (1 file)
└── operators/       # DELETE (1 file)
```

### Crates to UPDATE

- `ori_eval` — Update Type imports
- `ori_patterns` — Update Type imports
- `ori_llvm` — Update Type imports
- `oric` — Update Salsa queries and context

---

## Section Overview

### Section 1: Unified Pool Architecture

Replace dual Type/TypeData representation with single unified pool.

| Subsection | Focus | Source |
|------------|-------|--------|
| 1.1 | `Idx(u32)` universal handle | Zig |
| 1.2 | `Tag` enum (256 variants) | Zig |
| 1.3 | `Item { tag, data }` compact storage | Zig |
| 1.4 | `Pool` with interning and deduplication | Zig, Roc |
| 1.5 | Pre-interned primitives | Zig |
| 1.6 | Extra arrays for complex types (SoA) | Roc |

### Section 2: Pre-Computed Metadata

Cache type properties at interning time for O(1) queries.

| Subsection | Focus | Source |
|------------|-------|--------|
| 2.1 | `TypeFlags` bitflags (32 bits) | Rust |
| 2.2 | Presence flags (HAS_VAR, HAS_ERROR) | Rust |
| 2.3 | Category flags (IS_PRIMITIVE, IS_FUNCTION) | TypeScript |
| 2.4 | Optimization flags (NEEDS_SUBST, IS_RESOLVED) | Rust |
| 2.5 | Capability flags (HAS_CAPABILITY, IS_PURE) | Ori-specific |
| 2.6 | Stable hash caching | Rust |

### Section 3: Unification Engine

Link-based unification with path compression.

| Subsection | Focus | Source |
|------------|-------|--------|
| 3.1 | `VarState` enum (Unbound, Link, Rigid) | Gleam |
| 3.2 | `UnifyEngine` with resolve() | Gleam, Elm |
| 3.3 | Path compression in resolution | Union-Find theory |
| 3.4 | Flag-gated occurs check | Rust optimization |
| 3.5 | Structural unification for concrete types | All |

### Section 4: Rank-Based Generalization

Correct let-polymorphism with rank tracking.

| Subsection | Focus | Source |
|------------|-------|--------|
| 4.1 | `Rank` type and constants | Elm, Roc |
| 4.2 | Scope entry/exit tracking | Elm |
| 4.3 | Variable rank updates during unification | Elm |
| 4.4 | Generalization at scope exit | Elm, Roc |
| 4.5 | Instantiation with fresh variables | HM theory |
| 4.6 | Type scheme storage | Elm |

### Section 5: Context-Aware Errors

Elm-quality error messages with rich context.

| Subsection | Focus | Source |
|------------|-------|--------|
| 5.1 | `Expected` with origin tracking | Elm |
| 5.2 | `ContextKind` enum (30+ variants) | Elm |
| 5.3 | `TypeProblem` identification | TypeScript, Elm |
| 5.4 | Type diffing for specific problems | TypeScript |
| 5.5 | Suggestion generation | Elm, Gleam |
| 5.6 | `TypeCheckError` with full context | Gleam |

### Section 6: Type Inference Engine

Expression-level type inference.

| Subsection | Focus | Source |
|------------|-------|--------|
| 6.1 | `InferEngine` structure | All |
| 6.2 | Literal inference | All |
| 6.3 | Identifier lookup with instantiation | HM theory |
| 6.4 | Function call inference | All |
| 6.5 | Operator inference | All |
| 6.6 | Control flow (if/match/loops) | All |
| 6.7 | Lambda and closure inference | All |
| 6.8 | Pattern expression inference | Ori-specific |
| 6.9 | Collection literal inference | All |
| 6.10 | Struct construction inference | All |

### Section 7: Registries

Type, trait, and method registries.

| Subsection | Focus | Source |
|------------|-------|--------|
| 7.1 | `TypeRegistry` for user types | Current Ori |
| 7.2 | `TraitRegistry` for traits/impls | Current Ori |
| 7.3 | Unified `MethodRegistry` | New design |
| 7.4 | Built-in method definitions | Current Ori |
| 7.5 | Method lookup algorithm | All |

### Section 8: Salsa Integration

Incremental compilation support.

| Subsection | Focus | Source |
|------------|-------|--------|
| 8.1 | Derive requirements verification | Salsa |
| 8.2 | `type_check` query design | Current Ori |
| 8.3 | `TypedModule` output structure | Current Ori |
| 8.4 | Pool sharing across queries | New design |
| 8.5 | Determinism guarantees | Salsa |

### Section 9: Migration

Dependent crate updates.

| Subsection | Focus | Source |
|------------|-------|--------|
| 9.1 | `ori_eval` updates | - |
| 9.2 | `ori_patterns` updates | - |
| 9.3 | `ori_llvm` updates | - |
| 9.4 | `oric` Salsa query updates | - |
| 9.5 | Test migration | - |

---

## Performance Targets

| Metric | Current | Target | Improvement |
|--------|---------|--------|-------------|
| Memory per type | ~40 bytes | ~5 bytes | 87% reduction |
| Type equality | O(n) or hash | O(1) | Constant time |
| Unification | O(n) subst | O(α(n)) | Near-constant |
| Traversal skip | None | O(1) flag | Instant check |
| Error context | Minimal | Rich (30+) | Much better UX |

---

## Implementation Order

### Phase 1: Foundation (P0)
1. Section 01: Unified Pool Architecture
2. Section 02: Pre-Computed Metadata
3. Section 03: Unification Engine

### Phase 2: Inference (P1)
4. Section 04: Rank-Based Generalization
5. Section 05: Context-Aware Errors
6. Section 06: Type Inference Engine

### Phase 3: Integration (P2)
7. Section 07: Registries
8. Section 08: Salsa Integration

### Phase 4: Migration (P3)
9. Section 09: Migration

---

## Success Criteria

- [ ] All existing ori_typeck tests pass
- [ ] All spec tests pass
- [ ] O(1) type equality via index comparison
- [ ] O(α(n)) unification via link-based union-find
- [ ] O(1) flag checks for traversal optimization
- [ ] Memory usage reduced by 50%+ (measure with benchmarks)
- [ ] Error messages improved (subjective review)
- [ ] Full Salsa compatibility maintained
- [ ] No remnants of old system anywhere in codebase
- [ ] Clean clippy, no warnings

---

## Risk Mitigation

1. **Tests first** — Capture current test behavior before any deletion
2. **Incremental validation** — Test each section independently
3. **Reference repos** — Consult Zig/Roc/Gleam for edge cases
4. **No shortcuts** — Proper implementation only, no hacks
5. **Ask when unclear** — Don't guess architectural decisions

---

## Relationship with Parser V2

**Types V2 is independent of Parser V2.** The two systems are cleanly decoupled:

| Aspect | Parser V2 | Types V2 |
|--------|-----------|----------|
| **Phase** | Syntax → AST | AST → Typed AST |
| **Crates** | `ori_lexer`, `ori_parse`, `ori_ir` | `ori_types`, `ori_typeck` |
| **Status** | ✅ Complete | Not started |

### Terminology Clarification

Both plans use the word "metadata" but for **completely different concepts**:

| Term | Parser V2 | Types V2 |
|------|-----------|----------|
| **"Metadata"** | `ModuleExtra` — formatting trivia (comments, blank lines) | `TypeFlags` — type properties (HAS_VAR, IS_PRIMITIVE) |
| **Location** | `ori_ir/src/metadata.rs` | `ori_types/src/flags.rs` (new) |
| **Purpose** | Formatter/IDE support | Type checker optimization |

### No Parser Changes Required

Types V2 does **not** modify:
- `ori_lexer` — unchanged
- `ori_parse` — unchanged
- `ori_ir` — unchanged (may add new files, but no conflicts with `metadata.rs`)

The parser produces AST; the type checker consumes it. One-way dependency only.
