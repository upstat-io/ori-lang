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

- [ ] `[BUG-05-001][high]` **COW list operations produce wrong results in valgrind tests** — found by continue-roadmap.
  Repro: `./diagnostics/valgrind-aot.sh tests/valgrind/cow/cow_list_push.ori` (also `cow_list_set.ori`, `cow_iterator_collect.ori`)
  Error: Exit code 1 (wrong result), not valgrind memory errors. Tests exercise shared/unique push, set, and collect with `[int]`/`[str]` lists.
  Subsystem: `compiler/ori_rt/src/list/cow.rs`, `cow_structural.rs`
  Found: 2026-03-30 | Source: continue-roadmap
  Note: Pre-existing — not caused by §06 struct reordering (tests use plain lists, no struct types). May be related to known COW slice propagation issues.

---

## Resolved Bugs

- None.
