---
section: "03"
title: "Take-Project Hardening"
status: not-started
reviewed: false
goal: "Enforce the in_class ⊆ var_to_lineage invariant via lineage reconciliation and generalize is_take_project from iterator-only tag whitelist to MachineRepr-driven ownership classification"
inspired_by:
  - "Swift SIL OperandOwnership.cpp — ownership classification by type, not tag"
  - "Lean 4 LCNF ExpandResetReuse.lean — destructive extraction tracking"
  - "Koka CheckFBIP.hs — borrow-vs-own match boundary analysis"
depends_on: []
success_criteria:
  - "debug_assert!(in_class.iter().all(|v| var_to_lineage.contains_key(v))) passes on full test suite"
  - "is_take_project driven by MachineRepr::UnmanagedPtr ownership, not Tag::Iterator whitelist"
  - "Regression tests pin: predecessor arg in_class gets lineage, existing singleton lineages preserved"
  - "All 17,000+ tests pass in debug and release"
  - "ORI_CHECK_LEAKS=1 clean on all iterator_drop AOT tests"
sections:
  - id: "03.1"
    title: "Lineage Reconciliation Pass"
    status: not-started
  - id: "03.2"
    title: "is_take_project Generalization"
    status: not-started
  - id: "03.3"
    title: "Completion Checklist"
    status: not-started
third_party_review:
  status: none
  updated: null
---

# Section 03: Take-Project Hardening

**Context:** The AIMS take-project ownership model has two gaps discovered during the dual-source TPR review:

1. **Lineage invariant gap (TPR-07-001-gemini):** The `in_class` membership set (computed via bidirectional union-find including Jump edges at `take_project/mod.rs:645`) is a superset of `var_to_lineage` (computed via forward-only BFS at `take_project/mod.rs:389`). Predecessor block arguments enter `in_class` via the bidirectional Jump union but don't get lineage entries because the alias graph's Jump edges are forward-only. These orphaned vars are skipped by `dead_cleanup.rs:152` (in_class check) and `edge_cleanup.rs:212` (in_class check), and `is_bypass_safe_entry_for_var` returns false without lineage (line 232) → no RC decrement ever emitted → memory leak.

2. **Iterator-only scope (TPR-07-005-codex, TPR-07-004-gemini):** `is_take_project()` in `borrowed_defs.rs:61-88` checks `matches!(dst_tag, Tag::Iterator | Tag::DoubleEndedIterator)` — a tag-based whitelist. Future unique-owned types (Box<T>, channels) will silently stay on the borrow path, reintroducing double-free or leak bugs.

**Approach:** Fix the lineage gap via a **separate reconciliation pass** in `compute_lineage()` (NOT backward edges in `build_alias_graph()` — that would contaminate singleton lineages via phi→arg backward paths). Generalize `is_take_project` to check `MachineRepr` for unique ownership instead of tag whitelist.

**This section is independent of §01/§02** — take-project hardening doesn't touch enum layout. Can be developed in parallel with §02.

---

## 03.1 Lineage Reconciliation Pass

**File(s):** `compiler/ori_arc/src/aims/emit_rc/take_project/mod.rs` (modify `compute_lineage()` and `analyze()`)

**Critical design constraint:** Do NOT add backward edges to `build_alias_graph()`. The alias graph's Jump edges are intentionally forward-only to prevent phi contamination (established in TPR-07-019 fix, iteration 2). Adding backward edges would allow BFS from source A to reach source B via `A→phi→B` backward paths, merging what should be separate lineages. Instead, add a post-BFS reconciliation pass.

- [ ] **Write failing TDD matrix FIRST** — unit tests in `compiler/ori_arc/src/aims/emit_rc/take_project/tests.rs`:
  - `lineage_reconciliation_predecessor_arg_gets_lineage`: Topology with predecessor arg in_class but not forward-reachable → after fix, var has lineage entry. Without fix, var has no lineage (the gap).
  - `lineage_reconciliation_preserves_singleton_lineages`: Two sources merge at phi → each source retains singleton lineage {0} and {1} respectively, NOT merged. The phi param gets mixed lineage {0,1}.
  - `lineage_reconciliation_unrelated_predecessor_gets_default`: A predecessor arg that is NOT a take-project source but enters in_class via the union-find gets a default lineage matching its component's sources.
  - Negative pin: `lineage_reconciliation_does_not_create_spurious_lineage` — vars NOT in in_class do NOT get lineage entries from the reconciliation pass.

- [ ] **Add reconciliation pass in `compute_lineage()`** — after the forward-only BFS completes (around line 420), before sorting/dedup:
  ```rust
  // Post-BFS reconciliation: assign lineage to in_class vars that
  // the forward BFS didn't reach. These are predecessor args that
  // entered in_class via the bidirectional union-find (Jump edges)
  // but aren't forward-reachable from any source.
  //
  // Strategy: for each in_class var missing from var_to_sources,
  // find its union-find component, collect the source indices of
  // that component, and assign those as the var's lineage.
  // This preserves phi-safety: the var inherits the lineage of
  // the COMPONENT it belongs to, not of any individual source.
  for &var in &in_class {
      if !var_to_sources.contains_key(&var) {
          // Find component root, collect all source indices in that component
          let component_sources = /* collect source indices for vars
              in the same union-find component as `var` */;
          if !component_sources.is_empty() {
              var_to_sources.insert(var, component_sources);
          }
      }
  }
  ```
  The exact implementation will need to reference the union-find `parent` map (from `compute_membership`) and the existing `tp_sources` list to map component → source indices.

- [ ] **Thread `in_class` and `parent` into `compute_lineage()`** — currently `compute_lineage` takes `func`, `tp_sources`, and the alias graph. It needs `in_class` and the union-find `parent` map to perform reconciliation. Either pass them as parameters or restructure to access them from `analyze()`.

- [ ] **Add `debug_assert!` in `analyze()`** — after `compute_lineage()` returns, verify the invariant:
  ```rust
  debug_assert!(
      in_class.iter().all(|v| var_to_lineage.contains_key(v)),
      "in_class variable without lineage entry — reconciliation incomplete"
  );
  ```
  This assertion fires in debug builds on every test run, catching any future regression.

- [ ] **AOT regression test** in `compiler/ori_llvm/tests/aot/iterator_drop.rs`:
  - `take_project_predecessor_arg_no_leak`: Fixture with a branch where one arm has a take-project and the other doesn't, both jumping to a merge block. The non-take-project arm's arg must still get cleaned up correctly. Verify with `ORI_CHECK_LEAKS=1`.

- [ ] **Verify all 16+ existing `iterator_drop` AOT tests pass** in both debug and release. Zero regressions from the reconciliation pass.

- [ ] **Subsection close-out (03.1)** — Run `/improve-tooling` retrospectively. Commit separately.
- [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check` and clean any detected temp files.

---

## 03.2 is_take_project Generalization

**File(s):** `compiler/ori_arc/src/aims/emit_rc/borrowed_defs.rs` (modify `is_take_project()`)

- [ ] **Write failing TDD test FIRST** — currently hard to test since no non-iterator unique-owned types exist. Write a unit test that exercises the predicate directly:
  - `is_take_project_iterator_positive`: existing behavior preserved
  - `is_take_project_non_iterator_unmanaged_ptr`: future-proofing test — if a new UnmanagedPtr type is added, the predicate accepts it
  - `is_take_project_rc_pointer_negative`: RcPointer payloads are NOT take-projects (they have RC)
  - `is_take_project_opaque_ptr_negative`: OpaquePtr payloads are NOT take-projects (they may have external ownership)

- [ ] **Replace tag whitelist with MachineRepr check** in `is_take_project()`:
  ```rust
  // Current (iterator-only):
  // matches!(dst_tag, Tag::Iterator | Tag::DoubleEndedIterator)
  
  // New (MachineRepr-driven):
  // Check if the projected field's MachineRepr represents unique ownership
  // (UnmanagedPtr that is Box-allocated, no RC header).
  // This automatically covers Iterator, DoubleEndedIterator, and any
  // future unique-owned type without predicate changes.
  ```
  The exact check depends on how `MachineRepr` classifies unique ownership. Iterator/DoubleEndedIterator map to `MachineRepr::UnmanagedPtr` in `canonical/mod.rs:287`. The check becomes: "is the destination's MachineRepr an UnmanagedPtr?" with an exclusion for `OpaquePtr` (C FFI pointers that have external ownership).

- [ ] **Also replace `*field == 0` check** (line 70 of `borrowed_defs.rs`):
  - Current: `if *field == 0 { return false; }` — hardcodes tag at field 0
  - New: `if enum_layout_info.field_is_tag(*field) { return false; }` — uses §01's `EnumLayoutInfo`
  - This makes the predicate correct for ALL tag encodings (niche, tagged-ptr, none), not just explicit-tag

- [ ] **Subsection close-out (03.2)** — Run `/improve-tooling` retrospectively. Commit separately.
- [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check` and clean any detected temp files.

---

## 03.3 Completion Checklist

- [ ] Lineage reconciliation pass assigns lineage to ALL in_class vars
- [ ] `debug_assert!` enforces `in_class ⊆ var_to_lineage.keys()` — fires on full test suite in debug
- [ ] Singleton lineages preserved (no phi contamination from reconciliation)
- [ ] `is_take_project` uses MachineRepr ownership classification, not tag whitelist
- [ ] `field_is_tag()` from §01 replaces hardcoded `*field == 0` check
- [ ] All existing `iterator_drop` AOT tests pass (16+ tests, debug and release)
- [ ] New AOT test for predecessor arg leak scenario
- [ ] `ORI_CHECK_LEAKS=1` clean on all iterator-related test programs
- [ ] `./test-all.sh` green
- [ ] `./clippy-all.sh` green
- [ ] `/tpr-review` passed — independent review clean
- [ ] `/impl-hygiene-review` passed — MUST run AFTER `/tpr-review` is clean
- [ ] `/improve-tooling` section-close sweep completed

---

## 03.R Third Party Review Findings

- None.
