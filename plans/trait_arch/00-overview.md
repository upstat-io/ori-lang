# Trait Architecture Refactor: Cross-Cutting Design Improvements

> **Description:** Restructure the compiler's trait infrastructure to eliminate the "trait tax" — the 12+ file, 5-crate scavenger hunt required to add a new trait. Replace manual sync with data-driven registries, compile-time enforcement, and architectural splits that align the extension axis (traits) with the coupling axis (crates).
>
> **Primary Goal:** Adding a new derived trait requires editing metadata in one place, implementing backend logic in two places (eval + LLVM), and writing tests — with every missed sync point caught at `cargo t` time, not at runtime.

## Evidence Base

Analysis of 40 consecutive commits (3866e8e9..659e8e20, Feb 17-18 2026):

| Finding | Evidence |
|---------|----------|
| **35% reactive work** | 14 of 40 commits are fix/hygiene/drift/align — not features |
| **12+ touch points per trait** | Comparable addition touched 28 files across 5 crates |
| **N×M primitive satisfaction** | 10 types × 20 traits = 200 inline `\|\|` comparisons |
| **4 god files over limit** | `registration/mod.rs` (1761), `lower_builtin_methods.rs` (1497), `check_error/mod.rs` (2064), `calls.rs` (1011) |
| **Dual backend duplication** | Every derive implemented twice (eval + LLVM) with same logical structure |
| **No sync enforcement** | `DerivedTrait::Debug` silently skipped in LLVM codegen — no test caught it |
| **Mechanical error code boilerplate** | Every trait validation adds 3 files of repetitive code |

## Prior Art Consulted

| Compiler | Pattern Adopted | Relevance |
|----------|----------------|-----------|
| **Rust** (`rustc_hir/lang_items.rs`) | `language_item_table!` declarative macro — single source generates enum + all accessors | Model for Section 01 trait metadata macro |
| **Gleam** (`compiler-core/type_.rs`) | Exhaustive enum matching as enforcement | Confirms current pattern is sound; extend with `ALL` iteration |
| **Zig** (`Compilation.zig`, `InternPool.zig`) | Type info as packed data with bitfield queries | Model for Section 02 trait satisfaction bitsets |
| **TypeScript** (`types.ts`) | `TypeFlags` bitflag composition for O(1) type queries | Confirms bitset approach for trait satisfaction |
| **Rust** (`rustc_middle/ty/context.rs`) | `TyCtxt` with pre-computed method tables | Model for data-driven dispatch tables |
| **Swift** (`lib/Sema/DerivedConformances.cpp`) | `DerivedConformance::canDerive*()` + per-trait strategy files | Model for Section 07 derive strategy split |

## Relationship to Existing Plans

- **`dpr_registration-sync_02172026.md`**: This plan subsumes and implements Phase 1 and Phase 2 of that DPR. The DPR analyzed the pattern; this plan executes it plus six additional structural improvements.
- **`plans/roadmap/section-03-traits.md`**: This plan improves the *infrastructure* for adding traits. It does not add new traits itself, but makes every future trait addition cheaper.
- **`dpr_type-checker-perf_02182026.md`**: Section 02 (trait satisfaction bitsets) directly improves the hot path identified in that perf DPR.

## Architecture Overview

```
Before: Extension along traits × Coupling along crates = N×M touch points

  ori_ir     ori_types     ori_eval     ori_llvm     library/std
  ──────     ─────────     ────────     ────────     ───────────
  DerivedTrait  registration   derived_methods  derive_codegen  prelude.ori
  (7 variants)  (7 match arms)  (7 match arms)   (7 match arms)  (7 trait defs)
  from_name()   well_known.rs   methods/mod.rs   lower_builtin
  method_name() signatures      builtin names    field_ops
                prim_satisfies  interned names
                check_error     format

  Touch points per new trait: 12+


After: Metadata drives everything from one source

  ori_ir (SINGLE SOURCE OF TRUTH)
  ────────────────────────────────
  define_derived_traits! {
      (Eq,    "Eq",    "eq",    BinaryPredicate,  None,  true),
      (Clone, "Clone", "clone", UnaryIdentity,    None,  true),
      ...
  }
  ↓ generates: enum, from_name, method_name, trait_name, ALL, COUNT, shape
  ↓ exports: metadata consumed by all downstream crates

  ori_types          ori_eval           ori_llvm
  ─────────          ────────           ────────
  match shape {      match shape {      match shape {
    use metadata       use metadata       use metadata
  }                  }                  }
  + split modules    + unchanged        + factory fn
  + bitset traits    + unchanged        + split by type

  Touch points per new trait: 3-5 (metadata + eval handler + LLVM handler + tests)
```

## Implementation Tiers

### Tier 1: Foundation (High Impact, Low Risk)
- **Section 01:** Trait metadata registry — `define_derived_traits!` macro with `ALL`, `COUNT`, `trait_name`, metadata
- **Section 02:** Data-driven trait satisfaction — replace N×M `||` chains with bitsets
- **Section 03:** Registration module split — break 1761-line god file into submodules
- **Section 05:** Cross-crate sync enforcement — completeness tests using `DerivedTrait::ALL`

### Tier 2: Consolidation (Medium Impact, Medium Risk)
- **Section 04:** LLVM codegen consolidation — split `lower_builtin_methods.rs`, derive scaffolding factory

### Tier 3: Architecture (High Impact, Higher Risk)
- **Section 06:** Error code generation — `define_error_codes!` macro
- **Section 07:** Shared derive strategy — eliminate eval/LLVM derive duplication

## Dependencies

```
Section 01 (trait metadata)     ← standalone, no dependencies
Section 02 (trait satisfaction) ← depends on Section 01 (uses ALL for initialization)
Section 03 (registration split) ← standalone, no dependencies
Section 04 (LLVM refactor)      ← standalone, no dependencies
Section 05 (sync enforcement)   ← depends on Section 01 (uses ALL for iteration)
Section 06 (error codegen)      ← standalone, no dependencies
Section 07 (derive strategy)    ← depends on Section 01 (metadata) + Section 04 (factory)
```

**Recommended execution order:** 01 → 05 → 02 → 03 → 04 → 06 → 07

Sections 01, 03, 04, and 06 can be developed in parallel (no interdependencies).
Sections 02, 05, and 07 depend on Section 01.

## Progress

| Section | Title | Status | Commit |
|---------|-------|--------|--------|
| 01 | Trait Metadata Registry | **Complete** | `671ca6c7` |
| 02 | Data-Driven Trait Satisfaction | **Complete** | `f0786fd0` |
| 03 | Registration Module Split | **Complete** | — |
| 04 | LLVM Codegen Consolidation | **Complete** | — |
| 05 | Cross-Crate Sync Enforcement | **Complete** | `a718da5d` |
| 06 | Error Code Generation | **Complete** | — |
| 07 | Shared Derive Strategy | **Complete** | — |

**All sections complete.** The trait architecture refactor is finished.

## Exit Criteria

- [x] Adding a new derived trait requires editing `define_derived_traits!` invocation (1 file), defining strategy (same file), implementing eval handler (1 file), implementing LLVM handler (1 file), adding prelude definition (1 file), and writing tests (§01 + §07)
- [x] `cargo t` catches any missed sync point across all 4 consuming crates (§05)
- [x] No source file in the trait infrastructure exceeds 500 lines (excluding tests) (§03 + §04)
- [x] `primitive_satisfies_trait` is data-driven (bitset or table), not N×M inline code (§02)
- [x] `./test-all.sh` passes with zero regressions
- [x] All existing spec tests, compile-fail tests, and AOT tests continue to pass unchanged
