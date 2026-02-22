---
section: "11"
title: "Comprehensive Verification"
status: not-started
goal: "Verify the complete AOT pipeline against the full language surface area"
depends_on: ["01", "02", "03", "04", "05", "06", "07", "08", "09", "10"]
sections:
  - id: "11.1"
    title: "AOT test matrix"
    status: not-started
  - id: "11.2"
    title: "Dual-execution verification"
    status: not-started
  - id: "11.3"
    title: "Memory safety verification"
    status: not-started
  - id: "11.4"
    title: "Performance validation"
    status: not-started
  - id: "11.5"
    title: "Documentation"
    status: not-started
---

# Section 11: Comprehensive Verification

**Status:** Not Started
**Goal:** Every language feature compiles through AOT, matches JIT behavior, and has zero memory leaks.

**Context:** This is the capstone section. All architectural improvements are in place. Now we prove the system works as one cohesive whole by testing every language feature through the AOT pipeline and verifying behavioral equivalence with the JIT evaluator.

**Depends on:** All previous sections.

---

## 11.1 AOT Test Matrix

**File:** `compiler/ori_llvm/tests/aot/`

Build a comprehensive test matrix covering every language feature through AOT compilation.

- [ ] **Literals & primitives:**
  - int, float, bool, char, byte, str, unit
  - Arithmetic, bitwise, comparison, logical operators
  - String concatenation, interpolation, escapes

- [ ] **Control flow:**
  - if/else (expression-valued)
  - while loops with break/continue
  - loop (infinite with break)
  - for-in loops over iterators
  - for-yield (generator expressions)
  - Pattern matching (match expressions)
  - Nested control flow (match inside if inside loop)

- [ ] **Data structures:**
  - Tuple construction and destructuring
  - Struct construction, field access, update syntax
  - Enum construction and pattern matching
  - Recursive types (tree, linked list)
  - Generic types

- [ ] **Collections:**
  - List: literal, push, pop, index, length, iter, map, filter, collect
  - Map: literal, insert, get, remove, length, iter, keys, values
  - Set: literal, insert, contains, remove, length, iter, union, intersection
  - Range: `1..10`, `1..=10`, iter, collect

- [ ] **Functions & closures:**
  - Direct function calls
  - Method calls on types
  - Closures with 0, 1, N captures
  - Higher-order functions (passing closures)
  - Recursive functions
  - Mutually recursive functions
  - Functions returning closures
  - Closures capturing closures

- [ ] **Error handling:**
  - `?` propagation
  - try/catch blocks
  - panic and @panic handler
  - Result chaining

- [ ] **Traits & derived:**
  - Eq, Comparable, Hashable, Printable, Debug, Clone
  - Derived trait implementations on structs and enums
  - Custom trait implementations (if supported)
  - Operator overloading through traits

- [ ] **Iterator pipeline:**
  - map, filter, take, skip, enumerate, zip, chain
  - collect, fold, count, find, any, all, for_each
  - Chained adapters (map → filter → collect)
  - Nested iterators (flat_map, flatten)

- [ ] **ARC-specific:**
  - Shared references (multiple owners)
  - Last-reference optimization (in-place mutation when RC=1)
  - Drop ordering (nested structs, collections of RC'd values)
  - Reset/reuse (constructing same-shape value after match)

---

## 11.2 Dual-Execution Verification

Verify that AOT-compiled programs produce identical output to JIT-interpreted programs.

- [ ] Build a test harness that runs each test program twice:
  1. `ori run test.ori` → capture stdout, stderr, exit code (JIT)
  2. `ori build test.ori -o test && ./test` → capture stdout, stderr, exit code (AOT)
  3. Assert outputs are identical

- [ ] Apply to all spec tests in `tests/spec/`:
  ```bash
  for test in tests/spec/**/*.ori; do
      jit_output=$(ori run "$test" 2>&1) || true
      aot_output=$(ori build "$test" -o /tmp/test && /tmp/test 2>&1) || true
      diff <(echo "$jit_output") <(echo "$aot_output") || echo "MISMATCH: $test"
  done
  ```

- [ ] Track mismatches and investigate each one:
  - If JIT is correct and AOT differs → AOT bug
  - If AOT is correct and JIT differs → JIT bug
  - If both wrong → spec or type checker bug

- [ ] Create a CI-runnable script for this dual-execution check

---

## 11.3 Memory Safety Verification

- [ ] **Leak detection:** For every AOT test, verify `ori_rc_live_count()` returns 0 after `main` completes
  - Add a runtime hook that checks live count at exit
  - Any non-zero count indicates a leak
  - Report which types have leaked references

- [ ] **Use-after-free detection:** Compile and run tests under AddressSanitizer (ASan):
  ```bash
  CFLAGS="-fsanitize=address" cargo bl
  ./llvm-test.sh
  ```

- [ ] **Double-free detection:** Run under ASan — any double-free will be caught

- [ ] **Overflow detection:** Compile with refcount overflow checks enabled:
  - `ori_rc_inc` should panic (not wrap) if refcount exceeds `isize::MAX`

- [ ] **Stress test:** Create programs that exercise:
  - 10,000+ allocations/deallocations
  - Deep recursion (100+ levels)
  - Large collections (10,000+ elements)
  - Complex ownership patterns (diamond sharing, passing through multiple functions)

---

## 11.4 Performance Validation

- [ ] **Compile time:** Measure `ori build` time for programs of various sizes:
  - Small: 100 lines
  - Medium: 1,000 lines
  - Large: 10,000 lines (when available)
  - Track as baseline for future optimization

- [ ] **Runtime performance:** Compare AOT vs JIT execution time:
  - AOT should be significantly faster for compute-heavy programs
  - Measure with `time` or internal timing
  - Document the speedup ratio

- [ ] **RC overhead:** Measure the impact of RC operations:
  - Count total RcInc/RcDec executed at runtime (add counters)
  - Compare with and without RC elimination enabled
  - Report elimination effectiveness (% of ops removed)

- [ ] **Binary size:** Track compiled binary sizes:
  - Minimal program (hello world)
  - Medium program (data structure operations)
  - Record as baseline

---

## 11.5 Documentation

- [ ] Update `plans/arc_optimization/` to point to this plan as the superseding document
- [ ] Update `plans/arc_codegen_unification/` similarly
- [ ] Update `CLAUDE.md` if any new commands, paths, or patterns were introduced
- [ ] Update `.claude/rules/arc.md` with final pipeline description
- [ ] Add a brief architecture overview to `compiler/ori_arc/src/lib.rs` module doc
- [ ] Add a brief architecture overview to `compiler/ori_llvm/src/codegen/arc_emitter/mod.rs` module doc

---

## 11.6 Completion Checklist

- [ ] AOT test matrix covers all language features (every checkbox in 11.1 checked)
- [ ] Dual-execution script passes on all spec tests
- [ ] Zero memory leaks detected (live count = 0 at exit)
- [ ] ASan clean (no use-after-free, double-free)
- [ ] Stress tests pass
- [ ] Compile time baselined
- [ ] Runtime AOT > JIT performance verified
- [ ] RC elimination effectiveness measured and documented
- [ ] Binary sizes baselined
- [ ] All documentation updated
- [ ] Superseded plans marked as superseded
- [ ] `./test-all.sh` green
- [ ] `./llvm-test.sh` green
- [ ] `./clippy-all.sh` green

**Exit Criteria:** Every Ori language feature compiles through AOT and produces identical results to JIT interpretation, with zero memory leaks, under all test conditions. The AOT pipeline is the single, unified codegen path.
