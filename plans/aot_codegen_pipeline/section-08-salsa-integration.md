---
section: "08"
title: "Salsa-Integrated Borrow Inference"
status: in-progress
goal: "Cache borrow inference results to avoid redundant ARC pipeline runs"
inspired_by:
  - "Ori-unique — neither Swift, Lean, nor Rust has incremental borrow inference"
depends_on: ["04"]
sections:
  - id: "08.1"
    title: "Make ArcFunction Salsa-compatible"
    status: complete
  - id: "08.2"
    title: "Define borrow sig caching strategy"
    status: complete
  - id: "08.3"
    title: "Integrate with compilation pipeline"
    status: complete
  - id: "08.4"
    title: "Tests"
    status: in-progress
---

# Section 08: Salsa-Integrated Borrow Inference

**Status:** Complete
**Goal:** Cache borrow inference results so unchanged modules skip the ARC pipeline entirely. When a function body changes but its borrow signature is unchanged, callers don't need recompilation.

**Context:** This is Ori-unique — no reference compiler has incremental borrow inference. The original plan called for full Salsa query integration (`infer_borrows` as a `#[salsa::tracked]` query), but `FxHashMap<Name, AnnotatedSig>` does not satisfy `Eq + Hash` (required for Salsa return types), and `infer_borrows` operates on all functions at once for fixed-point iteration. Converting to per-function queries would require significant algorithm refactoring.

**Approach chosen:** Side-cache pattern (following `PoolCache`/`CanonCache`/`ImportsCache` precedent). A `BorrowSigCache` stores `Arc<FxHashMap<Name, AnnotatedSig>>` by file path, keyed on `CompilerDb` behind `#[cfg(feature = "llvm")]`. This avoids fighting Salsa trait requirements while providing session-scoped caching.

**Depends on:** Section 04 (Borrow Inference Hardening) — the hardened lookup ensures all functions have sigs before this makes them cached.

---

## 08.1 Make ArcFunction Salsa-Compatible

**File:** `compiler/ori_arc/src/ir/mod.rs`

Salsa query inputs must implement `Clone`, `Eq`, `PartialEq`, `Hash`, `Debug`.

- [x] Verify `ArcFunction` already derives or can derive these traits:
  - `Clone` — needed for Salsa memoization
  - `Eq`/`PartialEq` — needed for change detection
  - `Hash` — needed for Salsa's dependency tracking
  - `Debug` — needed for Salsa's logging
  - Check: `ArcBlock`, `ArcInstr`, `ArcTerminator`, `ArcValue`, `ArcVarId`, `ArcBlockId` all need these
  - **Result:** All types already derive `Clone, Debug, PartialEq, Eq, Hash`. No changes needed.

- [x] If any type is missing a derive, add it (likely `Hash` on some types)
  - **Result:** All types already have all required derives.

- [x] If `ArcFunction` is too large to clone/hash efficiently, consider using `Arc<ArcFunction>` as the query input (Salsa handles Arc natively)
  - **Result:** Using `Arc<FxHashMap<Name, AnnotatedSig>>` in the side-cache wrapping, which avoids cloning the full map. Individual `ArcFunction` types are reasonable size for cloning.

---

## 08.2 Define Borrow Sig Caching Strategy

**File:** `compiler/oric/src/db/mod.rs`

Instead of a full Salsa query (blocked by `FxHashMap` not satisfying `Eq + Hash`), we use the established side-cache pattern.

- [x] Define `BorrowSigCache` type following `PoolCache`/`CanonCache` pattern:
  - `Arc<RwLock<HashMap<PathBuf, Arc<FxHashMap<Name, AnnotatedSig>>>>>`
  - `store()`, `get()`, `invalidate()` methods
  - Behind `#[cfg(feature = "llvm")]` (since `ori_arc` is feature-gated)

- [x] Add `borrow_sig_cache` field to `CompilerDb` struct (behind `#[cfg(feature = "llvm")]`)

- [x] Add `borrow_sig_cache()` accessor on `CompilerDb` (not on `Db` trait — feature-gated types can't go on the trait)

- [x] Update `Default` and `with_interner()` constructors

**Note on invalidation:** `invalidate_file_caches()` takes `&dyn Db` and cannot access `BorrowSigCache`. For the current single-invocation CLI model, this is fine — each `ori build` creates a fresh `CompilerDb`. For future watch-mode, the watcher would invalidate directly on `&CompilerDb`.

---

## 08.3 Integrate with Compilation Pipeline

**File:** `compiler/oric/src/commands/compile_common.rs`

- [x] Wire `BorrowSigCache` into `compile_to_llvm()`:
  - Check cache before running `run_arc_pipeline_cached()`
  - On hit: skip entire ARC pipeline, use cached sigs
  - On miss: run pipeline, store result in cache
  - Tracing: `debug!` on cache hit for observability

- [x] Wire `BorrowSigCache` into `compile_to_llvm_with_imports()`:
  - Same pattern as `compile_to_llvm()`
  - Layered caching: `BorrowSigCache` (session-scoped) → `ArcIrCache` (disk-level)

- [x] Verify both LLVM and non-LLVM builds compile (cfg gating correct)

- [x] Verify `./test-all.sh` — zero regressions (9402 passed, 7 pre-existing failures)

---

## 08.4 Tests

**File:** `compiler/oric/src/db/tests.rs` (module `borrow_sig_cache`)

Unit tests for `BorrowSigCache` cache behavior (8 tests):

- [x] `store_and_retrieve`: Store sigs, retrieve same value
- [x] `cache_miss_returns_none`: Empty cache returns None
- [x] `invalidate_clears_entry`: Store then invalidate → None
- [x] `invalidate_nonexistent_is_noop`: Invalidate missing key doesn't panic
- [x] `separate_paths_independent`: Different files have independent cache entries
- [x] `overwrite_replaces_previous`: Second store replaces first
- [x] `db_accessor_returns_cache`: `CompilerDb::borrow_sig_cache()` provides working cache
- [x] `cloned_db_shares_cache`: Cloned `CompilerDb` shares the underlying `Arc<RwLock<...>>`

**End-to-end incremental tests:**

- [ ] End-to-end incremental test: compile file, modify body (same sig), recompile → cache hit
- [ ] End-to-end invalidation test: modify body (different sig), recompile → cache miss
- [ ] Benchmark: measure compile time improvement from session caching on multi-file programs

---

## 08.5 Completion Checklist

- [x] `ArcFunction` and all sub-types derive `Clone, Eq, PartialEq, Hash, Debug`
- [x] ~~`BorrowSigCache` type defined with `store()`/`get()`/`invalidate()` methods~~ — **SUPERSEDED** (2026-02-25): `BorrowSigCache` removed in Section 12.15. Replaced by per-SCC Salsa memoization which provides automatic caching, invalidation, and early cutoff.
- [x] ~~`BorrowSigCache` field added to `CompilerDb`~~ — **SUPERSEDED** (2026-02-25): Removed from `CompilerDb`.
- [x] `compile_to_llvm()` uses per-SCC Salsa borrow inference (migrated from `BorrowSigCache` in Section 12.15)
- [x] `compile_to_llvm_with_imports()` uses per-SCC Salsa borrow inference (migrated from `BorrowSigCache` in Section 12.15)
- [x] Both LLVM and non-LLVM builds compile
- [x] ~~Unit tests for cache behavior (8 tests)~~ — **SUPERSEDED** (2026-02-25): Tests removed with `BorrowSigCache`. Replaced by 7 incremental behavior tests in Section 12.12.
- [ ] End-to-end incremental tests
- [x] `./test-all.sh` passes (10,111 passed, 0 failed — 2026-02-25)
- [x] No performance regression on cold compile

**Exit Criteria:** ~~Borrow inference results are cached per-session via `BorrowSigCache`.~~ **Updated (2026-02-25):** Borrow inference is now fully Salsa-tracked via per-SCC queries (Section 12). `BorrowSigCache` was removed — Salsa memoization provides automatic caching at finer granularity (per-SCC vs per-file). End-to-end incremental verification required (Section 12.14).
