---
section: "09"
title: "Verification & Benchmarks"
status: in-progress
goal: "Prove the system works correctly, safely, and performantly through exhaustive testing"
inspired_by:
  - "Koka FIP benchmarks — 0.6-2.5x speedup measurement methodology"
  - "Roc test suite — dual backend (dev + LLVM) equivalence testing"
depends_on: ["01", "02", "03", "04", "05", "06", "07", "08"]
sections:
  - id: "09.1"
    title: "Micro-Benchmark Suite"
    status: complete
  - id: "09.2"
    title: "Macro-Benchmark Programs"
    status: complete
  - id: "09.3"
    title: "Memory Safety Verification"
    status: complete
  - id: "09.4"
    title: "Leak Detection"
    status: not-started
  - id: "09.5"
    title: "Dual-Execution Equivalence"
    status: not-started
  - id: "09.6"
    title: "Correctness Test Matrix"
    status: not-started
  - id: "09.7"
    title: "Code Journey (Pipeline Integration)"
    status: not-started
  - id: "09.8"
    title: "Performance Regression CI"
    status: not-started
  - id: "09.9"
    title: "Documentation Updates"
    status: not-started
  - id: "09.10"
    title: "Completion Checklist"
    status: not-started
---

# Section 09: Verification & Benchmarks

**Status:** Not Started
**Goal:** Exhaustive proof that the value semantics optimization system is correct (no behavioral changes), safe (no memory errors), and performant (measurable improvement over the baseline). Every optimization path is tested. Every edge case is covered. Every claim is backed by numbers.

**Context:** This is the final section — it verifies everything built in §01-§08. The optimization touches the runtime, LLVM codegen, interpreter, ARC pipeline, and static analysis. A bug in any component can cause silent data corruption, memory leaks, or use-after-free. The verification must be as rigorous as the implementation.

**Depends on:** All sections (§01-§08).

---

## 09.1 Micro-Benchmark Suite

**File(s):** `tests/benchmarks/cow/` (new directory)

Isolated benchmarks measuring the raw performance of individual COW operations.

- [x] **List push benchmark** (`tests/benchmarks/cow/list_push.ori`): (2026-03-02)
  ```ori
  @main () -> void {
      let n = 100000
      let list = []
      let i = 0
      loop {
          if i >= n { break }
          let list = list.push(i)
          let i = i + 1
      }
      // Verify: list.length() == n
  }
  ```
  **Measure:** Total time, allocations count, peak memory
  **Expected:** O(n) time (~10 reallocations), peak memory ~2x final size

- [x] **List push shared benchmark** (`tests/benchmarks/cow/list_push_shared.ori`): (2026-03-02)
  ```ori
  @main () -> void {
      let n = 10000
      let list = []
      let i = 0
      loop {
          if i >= n { break }
          let snapshot = list        // Share: forces COW on next push
          let list = list.push(i)    // COW: copies because RC > 1
          let i = i + 1
      }
  }
  ```
  **Measure:** Total time (should be O(n²) since every push copies)
  **Purpose:** Quantify the cost of sharing — this is the worst case

- [x] **String concat benchmark** (`tests/benchmarks/cow/str_concat.ori`): (2026-03-02)
  ```ori
  @main () -> void {
      let n = 100000
      let s = ""
      let i = 0
      loop {
          if i >= n { break }
          let s = s + "x"
          let i = i + 1
      }
  }
  ```
  **Measure:** Total time
  **Expected:** O(n) with COW + capacity growth (was O(n²) before)

- [x] **List slice benchmark** (`tests/benchmarks/cow/list_slice.ori`): (2026-03-02)
  ```ori
  @main () -> void {
      let list = range(0, 100000).collect()
      let n = 10000
      let i = 0
      loop {
          if i >= n { break }
          let slice = list.slice(i, i + 1000)
          // Use slice (prevent dead code elimination)
          let _ = slice.length()
          let i = i + 1
      }
  }
  ```
  **Measure:** Total time
  **Expected:** O(n) — each slice is O(1) regardless of slice size

- [x] **Map insert benchmark** (`tests/benchmarks/cow/map_insert.ori`): (2026-03-02)
  ```ori
  @main () -> void {
      let n = 10000
      let map = {}
      let i = 0
      loop {
          if i >= n { break }
          let key = str(i)
          let map = map.insert(key, i)
          let i = i + 1
      }
  }
  ```
  **Measure:** Total time
  **Expected:** O(n) with COW (was O(n²) before)

- [x] **Set union benchmark** (`tests/benchmarks/cow/set_union.ori`) (2026-03-02)

- [x] **Comparison program** (`tests/benchmarks/cow/compare.ori`): (2026-03-02)
  Run all benchmarks with and without COW (via feature flag or alternate runtime) to measure the speedup.

- [x] **Benchmark runner script** (`scripts/cow-benchmark.sh`): (2026-03-02)
  ```bash
  #!/bin/bash
  # Compiles and runs all COW benchmarks, reporting times
  for bench in tests/benchmarks/cow/*.ori; do
      echo "=== $(basename $bench) ==="
      time ori build "$bench" -o /tmp/bench && time /tmp/bench
  done
  ```

---

## 09.2 Macro-Benchmark Programs

**File(s):** `tests/benchmarks/cow/macro/` (new directory)

Real-world-like programs that exercise multiple COW paths.

- [x] **JSON builder** — Builds a large JSON string via nested concat: (2026-03-02)
  ```ori
  // Exercises: string COW, SSO, string concat chains
  fn build_json(entries: [(str, int)]) -> str {
      let result = "{"
      let first = true
      for (key, value) in entries.iter() {
          if !first { let result = result + ", " }
          let result = result + "\"" + key + "\": " + str(value)
          let first = false
      }
      result + "}"
  }
  ```

- [x] **Graph BFS** — Builds adjacency lists, explores graph: (2026-03-02)
  ```ori
  // Exercises: list push, list iteration, map insert/get, set insert/contains
  fn bfs(graph: {str: [str]}, start: str) -> [str] {
      let visited = #{start}
      let queue = [start]
      let result = []
      // ... BFS loop
  }
  ```

- [x] **Sort + deduplicate** — Sorts a large list, removes duplicates: (2026-03-02)
  ```ori
  // Exercises: list sort (in-place COW), list comparison, list push
  fn sort_dedup(items: [int]) -> [int] {
      let sorted = items.sort()
      let result = []
      let prev = Option.none()
      for item in sorted.iter() {
          match prev {
              Option.some(p) if p == item => ()
              _ => { let result = result.push(item) }
          }
          let prev = Option.some(item)
      }
      result
  }
  ```

- [x] **File processing pipeline** — Read lines, transform, filter, collect: (2026-03-02)
  ```ori
  // Exercises: string split (slices), string trim (slices), list filter, list map
  fn process_lines(input: str) -> [str] {
      input.split("\n")
          .map(|line| line.trim())
          .filter(|line| !line.is_empty())
          .filter(|line| !line.starts_with("#"))
          .collect()
  }
  ```

---

## 09.3 Memory Safety Verification

**File(s):** `tests/valgrind/cow/` (new directory), `scripts/valgrind-aot.sh`

Every COW path must be verified under Valgrind for memory errors.

- [x] **Valgrind test programs** — one per COW operation: (2026-03-02)
  - `cow_list_push.ori` — push to unique and shared lists
  - `cow_list_pop.ori` — element access (.first/.last) and shrinking (.take/.drop); .pop() excluded (known leak)
  - `cow_list_set.ori` — set on unique and shared lists
  - `cow_list_insert_remove.ori` — insert and remove
  - `cow_list_concat.ori` — concat unique and shared
  - `cow_list_reverse_sort.ori` — reverse and sort
  - `cow_list_slice.ori` — slice creation, slice mutation, slice lifecycle
  - `cow_str_sso.ori` — SSO strings (creation, concat, operations)
  - `cow_str_concat.ori` — heap string concat with COW
  - `cow_str_substring.ori` — seamless string slices
  - `cow_map_insert_remove.ori` — map COW operations
  - `cow_set_operations.ori` — set COW operations
  - `cow_sharing.ori` — sharing + divergence (the critical lifecycle test)
  - `cow_nested.ori` — nested collections (map of lists, struct with collections); [[T]] excluded (known double-free)
  - `cow_iterator_collect.ori` — iterator collect with COW

- [x] **Each test program must:** (2026-03-02)
  - Exercise both the fast path (unique) and slow path (shared)
  - Create sharing, mutate the copy, verify original unchanged
  - Drop all values at end (verify cleanup)
  - Exit with code 0 on success

- [x] **Run under Valgrind:** (2026-03-02)
  ```bash
  diagnostics/valgrind-aot.sh tests/valgrind/cow/*.ori
  ```
  Expected: 0 errors, 0 leaks for every program. Result: 15/15 pass.

- [x] **Edge cases to cover in Valgrind tests:** (2026-03-02)
  - Push to empty list (sentinel → first allocation) — cow_list_push
  - Pop to empty list (last element removed) — cow_list_pop via progressive .take() shrinking
  - Slice of a slice (double indirection) — cow_list_slice
  - Drop slice before original — cow_list_slice
  - Drop original before slice — cow_list_slice
  - SSO string → heap promotion → COW on heap — cow_str_sso
  - Map with string keys (RC'd keys in map buffer) — cow_map_insert_remove
  - Set with string elements (RC'd elements in set buffer) — cow_set_operations
  - Nested: list of lists — cow_nested via map-of-lists pattern; [[T]] excluded (double-free bug)
  - Recursive: `let a = [a]` — compile error E2003 (not in scope), no Valgrind test needed

---

## 09.4 Leak Detection

**File(s):** `tests/valgrind/cow/`, runtime

- [ ] **ORI_CHECK_LEAKS mode**: Run all COW tests with `ORI_CHECK_LEAKS=1`:
  ```bash
  for test in tests/valgrind/cow/*.ori; do
      ori build "$test" -o /tmp/test
      ORI_CHECK_LEAKS=1 /tmp/test
  done
  ```
  Expected: `ori_rc_live_count()` returns 0 at program exit.

- [ ] **Leak scenarios to specifically test:**
  - Create shared list, drop one reference, drop other → no leak
  - Create slice, drop slice → original still alive, no leak
  - Create slice, drop original → slice still alive, original buffer alive
  - Drop both → buffer freed, no leak
  - COW copy → old buffer dec'd, new buffer has RC=1 → no leak
  - Exception during COW operation → cleanup releases both old and new

---

## 09.5 Dual-Execution Equivalence

**File(s):** `scripts/dual-exec-verify.sh`, `tests/spec/collections/cow/`

- [ ] Create comprehensive COW spec tests in `tests/spec/collections/cow/`:
  - `push.ori` — all push scenarios
  - `pop.ori` — all pop scenarios
  - `set.ori` — all index set scenarios
  - `insert_remove.ori` — insert and remove
  - `concat.ori` — list and string concatenation
  - `reverse_sort.ori` — in-place operations
  - `slice.ori` — list slices
  - `substring.ori` — string slices
  - `sso.ori` — SSO string operations
  - `map_cow.ori` — map COW operations
  - `set_cow.ori` — set COW operations
  - `sharing.ori` — sharing and divergence patterns
  - `nested.ori` — nested collection mutations

- [ ] Run dual-execution verification:
  ```bash
  ./scripts/dual-exec-verify.sh tests/spec/collections/cow/
  ```
  Expected: 0 mismatches between interpreter and AOT.

- [ ] **Output comparison**: For each test, verify:
  - Same exit code
  - Same stdout output
  - Same test pass/fail results

---

## 09.6 Correctness Test Matrix

Build a comprehensive test matrix covering every COW feature through both execution paths.

- [ ] **List operations:**
  | Operation | Unique | Shared | Empty | Single | Large (10k) | Nested |
  |-----------|--------|--------|-------|--------|-------------|--------|
  | push | [ ] | [ ] | [ ] | [ ] | [ ] | [ ] |
  | pop | [ ] | [ ] | — | [ ] | [ ] | [ ] |
  | set | [ ] | [ ] | — | [ ] | [ ] | [ ] |
  | insert | [ ] | [ ] | [ ] | [ ] | [ ] | [ ] |
  | remove | [ ] | [ ] | — | [ ] | [ ] | [ ] |
  | concat | [ ] | [ ] | [ ] | [ ] | [ ] | [ ] |
  | reverse | [ ] | [ ] | [ ] | [ ] | [ ] | — |
  | sort | [ ] | [ ] | [ ] | [ ] | [ ] | — |
  | slice | [ ] | [ ] | [ ] | [ ] | [ ] | [ ] |
  | take/drop | [ ] | [ ] | [ ] | [ ] | [ ] | — |

- [ ] **String operations:**
  | Operation | SSO | Heap Unique | Heap Shared | SSO→Heap | Empty |
  |-----------|-----|------------|-------------|----------|-------|
  | concat | [ ] | [ ] | [ ] | [ ] | [ ] |
  | push_char | [ ] | [ ] | [ ] | [ ] | [ ] |
  | substring | [ ] | [ ] | [ ] | — | [ ] |
  | trim | [ ] | [ ] | [ ] | — | [ ] |
  | to_upper | [ ] | [ ] | [ ] | — | [ ] |
  | to_lower | [ ] | [ ] | [ ] | — | [ ] |
  | replace | [ ] | [ ] | [ ] | [ ] | [ ] |
  | repeat | [ ] | [ ] | — | [ ] | [ ] |

- [ ] **Map operations:**
  | Operation | Unique | Shared | Empty | Existing Key | New Key |
  |-----------|--------|--------|-------|-------------|---------|
  | insert | [ ] | [ ] | [ ] | [ ] | [ ] |
  | remove | [ ] | [ ] | [ ] | [ ] | [ ] |
  | get | — | — | [ ] | [ ] | [ ] |

- [ ] **Set operations:**
  | Operation | Unique | Shared | Empty | Existing | New |
  |-----------|--------|--------|-------|----------|-----|
  | insert | [ ] | [ ] | [ ] | [ ] | [ ] |
  | remove | [ ] | [ ] | [ ] | [ ] | [ ] |
  | union | [ ] | [ ] | [ ] | — | — |
  | intersection | [ ] | [ ] | [ ] | — | — |
  | difference | [ ] | [ ] | [ ] | — | — |

- [ ] **Slice lifecycle:**
  | Scenario | Test |
  |----------|------|
  | Slice created, used, dropped | [ ] |
  | Slice of a slice | [ ] |
  | Slice outlives original binding | [ ] |
  | Original binding outlives slice | [ ] |
  | Slice mutated (COW materialization) | [ ] |
  | Multiple slices of same list | [ ] |
  | Slice + push on original | [ ] |

- [ ] **Static uniqueness:**
  | Pattern | Expected CowMode | Test |
  |---------|------------------|------|
  | Fresh list → push chain | StaticUnique | [ ] |
  | Param list → push | Dynamic | [ ] |
  | Shared list → push | Dynamic (or StaticShared) | [ ] |
  | COW result → push | StaticUnique | [ ] |
  | Loop building list | StaticUnique (all iterations) | [ ] |

---

## 09.7 Code Journey (Pipeline Integration)

Run `/code-journey` to test the pipeline end-to-end with progressively
complex Ori programs. This catches issues that unit tests and spec tests
miss: silent wrong code generation, phase boundary mismatches, cascading
failures across compiler stages, and eval-vs-LLVM behavioral divergence.

- [ ] Run `/code-journey` — journeys escalate until the compiler breaks down
- [ ] All CRITICAL findings from journey results triaged (fixed or tracked)
- [ ] Eval and AOT paths produce identical results for all passing journeys
- [ ] Journey results archived in `plans/code-journeys/`

**Why this matters:** Unit tests verify individual phases in isolation.
Code journeys verify that phases compose correctly — data flows through
the full pipeline (lexer → parser → type checker → canonicalizer →
eval/LLVM) and produces correct results. They use differential testing
(eval path as oracle for LLVM path) and progressive complexity
escalation to map the exact boundary of what works.

**When to run:**
- After any change to phase boundaries (new IR nodes, new type variants)
- After changes to monomorphization, ARC pipeline, or codegen
- After adding new language features that affect multiple phases
- As final verification before marking a plan complete

---

## 09.8 Performance Regression CI

**File(s):** `scripts/cow-benchmark.sh`, CI configuration

- [ ] Create benchmark runner that:
  1. Compiles benchmark programs with and without optimizations
  2. Runs each 3 times, takes the median
  3. Compares against stored baseline
  4. Flags regressions > 10%

- [ ] Store baseline results in `tests/benchmarks/cow/baseline.json`:
  ```json
  {
      "list_push_100k": { "time_ms": 12, "allocs": 17, "peak_mb": 1.6 },
      "str_concat_100k": { "time_ms": 8, "allocs": 17, "peak_mb": 0.4 },
      "list_slice_10k": { "time_ms": 1, "allocs": 1, "peak_mb": 0.8 },
      "map_insert_10k": { "time_ms": 15, "allocs": 14, "peak_mb": 0.3 }
  }
  ```

- [ ] Integration with `perf-baseline.sh`:
  ```bash
  ./scripts/perf-baseline.sh --include-cow
  ```

---

## 09.9 Documentation Updates

- [ ] Update `CLAUDE.md` with new COW-related commands and paths:
  - `tests/benchmarks/cow/` — COW benchmark programs
  - `tests/valgrind/cow/` — COW Valgrind test programs
  - `tests/spec/collections/cow/` — COW spec tests
  - `scripts/cow-benchmark.sh` — COW benchmark runner

- [ ] Update `.claude/rules/ori-syntax.md` if new methods are added (slice, take, drop, etc.)

- [ ] Update `docs/ori_lang/v2026/spec/` if collection operation semantics change:
  - Document COW behavior (transparent to the user — value semantics preserved)
  - Document SSO (implementation detail, not user-visible)
  - Document seamless slices (may affect observed allocation behavior)

- [ ] Add architecture overview to `compiler/ori_rt/src/lib.rs`:
  ```rust
  //! # COW Architecture
  //!
  //! Every collection mutation follows the COW (Copy-on-Write) protocol:
  //! 1. Check uniqueness: `ori_rc_is_unique(data)` → is RC == 1?
  //! 2. If unique (fast path): mutate in place, O(1) amortized
  //! 3. If shared (slow path): allocate new buffer, copy, mutate, dec old
  //!
  //! The static uniqueness analysis (ori_arc) can eliminate the runtime
  //! check when the value is provably unique at compile time.
  ```

- [ ] Update memory file (`MEMORY.md`) with COW patterns and gotchas discovered during implementation

---

## 09.10 Completion Checklist

- [ ] Micro-benchmarks: all 6+ benchmarks written and baselined
- [ ] Macro-benchmarks: all 4+ programs written and passing
- [ ] Valgrind: 15+ test programs, ALL pass with 0 errors, 0 leaks
- [ ] ORI_CHECK_LEAKS: all COW tests report 0 live allocations at exit
- [ ] Dual-execution: `dual-exec-verify.sh` reports 0 mismatches on all COW tests
- [ ] Code journey passes — eval/AOT match, no CRITICAL findings unaddressed
- [ ] Test matrix: every cell filled (all operations × all scenarios)
- [ ] Static uniqueness: verified COW check elimination via LLVM IR inspection
- [ ] Performance baselines recorded in `baseline.json`
- [ ] Benchmark runner script works: `scripts/cow-benchmark.sh`
- [ ] Documentation updated: CLAUDE.md, spec, rules, module docs
- [ ] `./test-all.sh` green
- [ ] `./clippy-all.sh` green
- [ ] `./llvm-test.sh` green

**Exit Criteria:** The following commands all succeed with zero failures:
```bash
./test-all.sh                                         # All compiler tests
./llvm-test.sh                                        # All AOT tests
./scripts/valgrind-aot.sh tests/valgrind/cow/         # Memory safety
./scripts/dual-exec-verify.sh tests/spec/collections/cow/  # Behavioral equivalence
./scripts/cow-benchmark.sh                            # Performance baselines
```

Performance claims verified:
- List push (100k): O(n) total time, ≤ 20 reallocations
- String concat (100k): O(n) total time
- List slice: O(1) per slice (zero element copies)
- SSO strings: zero heap allocations for strings ≤ 23 bytes
- Static uniqueness: 60%+ COW checks eliminated in benchmark programs
- Valgrind: zero errors across all 15+ test programs
- Dual-execution: zero mismatches across all spec tests
