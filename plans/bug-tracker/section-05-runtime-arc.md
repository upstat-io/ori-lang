---
section: "05"
title: "Runtime & ARC"
status: not-started
goal: "Track and resolve all known runtime and memory management bugs"
sections: []
---

# Section 05: Runtime & ARC

**Subsystem:** `compiler/ori_rt/`

Bugs in the runtime library: reference counting, COW operations, slice handling, buffer management, memory leaks, double-frees, and AIMS coherence.

---

## Open Bugs

- [x] `[BUG-05-001][critical]` **Integer narrowing Phase C applied to `push` destinations, corrupting elements ≥ narrow range** — found by continue-roadmap, root-caused and fixed 2026-04-02.
  Resolved: Fixed on 2026-04-02. Root cause: `update_element_summaries` only tracked `Construct(ListLiteral)` and `CollectionReuse`, missing `Apply` and `Invoke` calls returning `[int]`. Fix: (1) `update_element_summaries` now widens element range to `Top` for any `Apply`/`ApplyIndirect` returning a collection with int elements, (2) new `update_element_summaries_from_terminator` handles `Invoke` terminators (the actual path for `push` — it can panic, so it's an Invoke). Both the fixpoint loop and the post-narrowing recompute pass now check terminators. Tests: 6 Rust unit tests (Apply/ApplyIndirect/Invoke widening, literal+Apply combined, negative pin for non-collection Apply) + 3 AOT regression tests (push corruption guard, push large values, literal-only still narrows). All 3 originally-failing COW valgrind tests now exit 0.
  Subsystem: `compiler/ori_repr/src/range/field_summary.rs`, `compiler/ori_repr/src/range/fixpoint/mod.rs`, `compiler/ori_repr/src/range/fixpoint/narrowing.rs`
  Found: 2026-03-30 | Root-caused: 2026-04-02 | Fixed: 2026-04-02 | Source: continue-roadmap + OBE investigation

---

## Resolved Bugs

- None.
