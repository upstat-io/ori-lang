---
section: "09"
title: "Layout Caching via Salsa Query"
status: not-started
reviewed: false
goal: "Extract type layout computation from ori_repr's imperative pass into a Salsa-tracked layout_of(Idx) -> Layout query, replacing the manual RefCell<FxHashMap> cache with demand-driven memoization"
success_criteria:
  - "A layout_of(Idx) -> Layout Salsa-tracked query exists and is the sole entry point for layout data"
  - "All codegen consumers call layout_of() — no manual layout recomputation from the type pool"
  - "The manual RefCell<FxHashMap> cache in TypeLayoutResolver is replaced by Salsa memoization"
  - "Layout computation is deterministic and cached across Salsa revisions"
  - "impl-hygiene.md §Aspirational Patterns updated to mark Layout Caching as 'implemented'"
inspired_by:
  - "Rust TyCtxt::layout_of (compiler/rustc_middle/src/ty/layout.rs)"
  - "Rust rustc_target::abi::Layout — memoized query, one computation per type"
  - "Zig InternPool-based layout (cached alongside type interning)"
depends_on: ["01"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "09.1"
    title: "Design Layout Query Interface"
    status: not-started
  - id: "09.2"
    title: "Implement Salsa-Tracked Layout Query"
    status: not-started
  - id: "09.3"
    title: "Migrate Codegen Consumers"
    status: not-started
  - id: "09.4"
    title: "Remove Manual Cache & Documentation"
    status: not-started
  - id: "09.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "09.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 09: Layout Caching via Salsa Query

**Status:** Not Started
**Goal:** Move type layout computation from an imperative pass with manual caching to a Salsa-tracked query that any phase can call. This is the aspirational pattern from `impl-hygiene.md` — "A Salsa-tracked `layout_of(TypeId) -> Layout` query that any phase can call."

**Success Criteria:**
- [ ] `layout_of()` Salsa query exists — satisfies mission criterion "all consumers call layout_of()"
- [ ] No manual `RefCell<FxHashMap>` cache remains — satisfies mission criterion "Salsa memoization"
- [ ] All codegen tests pass with Salsa-cached layout — behavior identical to imperative pass
- [ ] `impl-hygiene.md` updated — aspirational pattern marked as implemented

**Context:** Research found a two-layer layout system:
1. **`ori_repr`** (`compiler/ori_repr/src/layout/`, 285+ lines): Pure Rust layout computation (`repr_size()`, `repr_align()`, `compute_field_layout()`, `reorder_fields()`) operating on `MachineRepr` enum. Produces `ReprPlan`. NOT a Salsa query — computed imperatively once per compilation.
2. **`ori_llvm` `TypeLayoutResolver`** (`compiler/ori_llvm/src/codegen/type_info/layout_resolver.rs`, 394 lines): `Idx → BasicTypeEnum` with `RefCell<FxHashMap<Idx, BasicTypeEnum>>` cache and cycle detection via `FxHashSet<Idx>` + `MAX_RESOLVE_DEPTH = 32`.

The gap: layout is computed imperatively and cached manually. Moving to Salsa gives:
- Automatic invalidation when types change (incremental correctness)
- Cross-phase availability (ARC can query layout for alignment-aware optimization)
- Determinism guarantee from Salsa's purity requirement
- Cache eviction handled by Salsa revision tracking

**Design challenge:** `ReprPlan` currently carries `Idx` references, not LLVM handles (`repr.md §RP-4`). The Salsa query should return a `ReprPlan`-like type that derives all Salsa-required traits (`Clone, Eq, PartialEq, Hash, Debug`). `ReprPlan` already derives these per `repr.md §RP-4`.

**Reference implementations:**
- **Rust** `TyCtxt::layout_of()`: memoized layout query on the type context. Lazily computed on first access, cached, never recomputed.

**Depends on:** Section 01 (policy language). Benefits from Section 07 (TypeFolder patterns for type traversal in layout computation).

---

## 09.1 Design Layout Query Interface

**File(s):** design document (not code yet)

- [ ] Design the Salsa query interface:
  - `layout_of(db: &dyn CompilerDb, idx: Idx) -> ReprPlan` — tracked query
  - Input: fully-resolved `Idx` (per `CG:TR-2` — no `Tag::Var` in codegen input)
  - Output: `ReprPlan` (already `Clone, Eq, Hash, Debug` per `repr.md §RP-4`)
  - The query lives in a shared location accessible to both `ori_arc` (for alignment-aware optimization) and `ori_llvm` (for emission)

- [ ] Decide the hosting crate:
  - Option A: `ori_types` — closest to the pool, but adds layout dependency to the type crate
  - Option B: `ori_repr` — already owns layout computation, just needs Salsa wiring
  - Option C: new `ori_layout` crate — cleanest separation but adds a crate
  - Recommendation: Option B (`ori_repr`) — it already owns the computation; adding Salsa tracking is the minimal change

- [ ] Design the migration path:
  - Phase 1: Add `layout_of()` query to `ori_repr` alongside existing imperative computation
  - Phase 2: Migrate `TypeLayoutResolver` in `ori_llvm` to call `layout_of()` instead of maintaining its own cache
  - Phase 3: Remove the `RefCell<FxHashMap>` manual cache

- [ ] Validate design against `/tp-help`

- [ ] **Subsection close-out (09.1)** — MANDATORY before starting 09.2:
  - [ ] All tasks above are `[x]`
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [ ] **Run `/sync-claude` to check if code changes affect CLAUDE.md or rules files**

---

## 09.2 Implement Salsa-Tracked Layout Query

**File(s):** `compiler/ori_repr/src/`, `compiler/oric/src/query/`

- [ ] Write failing tests:
  - Query `layout_of()` for primitive types — verify sizes match `CG:TR-1`
  - Query for struct types — verify field ordering matches `RP-20`
  - Query twice for same `Idx` — verify same result (Salsa caching)
  - Query after type pool mutation — verify invalidation (new Salsa revision)

- [ ] Add Salsa tracked query:
  - Wire `layout_of` as a `#[salsa::tracked]` function in the compiler's query group
  - Implementation calls existing `ori_repr` layout computation functions
  - Result is `ReprPlan` (already Salsa-compatible: derives Clone, Eq, Hash, Debug; no Arc<Mutex>, no fn pointers, no LLVM handles per `repr.md §RP-4`)
  - Cycle detection: if `layout_of(A)` recursively calls `layout_of(A)` (recursive type), produce a clear error — Salsa's cycle detection should handle this but verify

- [ ] Verify determinism: same input pool → same `ReprPlan` output across multiple calls and revisions

- [ ] **Subsection close-out (09.2)** — MANDATORY before starting 09.3:
  - [ ] All tasks above are `[x]`
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [ ] **Run `/sync-claude` to check if code changes affect CLAUDE.md or rules files**

---

## 09.3 Migrate Codegen Consumers

**File(s):** `compiler/ori_llvm/src/codegen/type_info/layout_resolver.rs`, `compiler/ori_llvm/src/codegen/`

- [ ] Replace `TypeLayoutResolver`'s manual `RefCell<FxHashMap<Idx, BasicTypeEnum>>` cache:
  - Instead of caching layout in a local HashMap, call `layout_of(db, idx)` to get the `ReprPlan`
  - The LLVM type construction (`Idx → BasicTypeEnum`) still happens in `ori_llvm` — but the layout DATA comes from the Salsa query
  - The `TypeLayoutResolver` becomes a thin wrapper: `layout_of()` → `ReprPlan` → construct LLVM type

- [ ] Migrate all codegen sites listed in `repr.md §RP-5 Consumers`:
  - `CG:TR-1` canonical type mapping: call `layout_of()` for layout data
  - `CG:TR-3` aggregate field ordering: use `FieldEntry` from `layout_of()`
  - `CG:AB-1` ABI threshold: use `abi_size` from `layout_of()`
  - `CG:NR-2/NR-6` narrowing: use `storage_width` from `layout_of()`

- [ ] Remove the cycle detection `FxHashSet<Idx>` from `TypeLayoutResolver` — Salsa's cycle detection handles this

- [ ] Verify: `timeout 150 ./test-all.sh` — all codegen tests, FileCheck tests, AOT tests pass

- [ ] **Subsection close-out (09.3)** — MANDATORY before starting 09.4:
  - [ ] All tasks above are `[x]`
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [ ] **Run `/sync-claude` to check if code changes affect CLAUDE.md or rules files**

---

## 09.4 Remove Manual Cache & Documentation

**File(s):** `compiler/ori_llvm/src/codegen/type_info/layout_resolver.rs`, `.claude/rules/impl-hygiene.md`, `.claude/rules/repr.md`

- [ ] Remove the `RefCell<FxHashMap>` manual cache from `TypeLayoutResolver` — all layout data now comes from Salsa

- [ ] Update `impl-hygiene.md` §Aspirational Patterns → Layout Caching: change from "aspirational" to "implemented"

- [ ] Update `repr.md` §RP-4:
  - Change "NOT a Salsa tracked struct. Computed imperatively" to "Salsa-tracked query via `layout_of()`. Computed on demand and memoized."
  - Update the consumers table to reference the Salsa query

- [ ] Update `canon.md` §6 SSOTs table — add `layout_of()` as the canonical home for layout computation

- [ ] **Subsection close-out (09.4)** — MANDATORY before completing section:
  - [ ] All tasks above are `[x]`
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [ ] **Run `/sync-claude` to check if code changes affect CLAUDE.md or rules files**

---

## 09.R Third Party Review Findings

- None.

---

## 09.N Completion Checklist

- [ ] All subsections (09.1-09.4) complete
- [ ] `layout_of()` Salsa query exists and is the sole layout entry point
- [ ] Manual `RefCell<FxHashMap>` cache removed
- [ ] All codegen tests, FileCheck tests, AOT tests pass
- [ ] Layout is deterministic across Salsa revisions (tested)
- [ ] Debug AND release builds pass
- [ ] `timeout 150 ./test-all.sh` passes
- [ ] **`/tpr-review`** — independent dual-source review clean
- [ ] **`/impl-hygiene-review`** — implementation hygiene clean
- [ ] **`/improve-tooling`** — section-close sweep
- [ ] **`/sync-claude`** — section-close doc sync
