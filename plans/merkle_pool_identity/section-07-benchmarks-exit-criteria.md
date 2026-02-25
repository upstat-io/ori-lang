---
section: "07"
title: "Benchmarks & Exit Criteria"
status: not_started
goal: "Measure and verify that Merkle pool identity delivers measurable performance improvements and zero regressions"
inspired_by:
  - "Zig benchmark suite — compile-time measurement for interning changes"
  - "rustc perf — wall-clock + instruction-count regression testing"
sections:
  - id: "07.1"
    title: "Interning Throughput Benchmark"
    status: not_started
  - id: "07.2"
    title: "Import Boundary Benchmark"
    status: not_started
  - id: "07.3"
    title: "Cross-Module Comparison Benchmark"
    status: not_started
  - id: "07.4"
    title: "Memory Usage Analysis"
    status: not_started
  - id: "07.5"
    title: "Regression Testing"
    status: not_started
  - id: "07.6"
    title: "Exit Criteria"
    status: not_started
---

# Section 07: Benchmarks & Exit Criteria

**Status:** Not Started
**Goal:** Quantitatively verify that Merkle pool identity delivers its promised performance
characteristics (O(1) cross-module identity, faster imports) without regressing existing
performance (interning throughput, type checking speed, compile time).

**Why this matters:** The Merkle hash change touches the hottest path in the type system —
every type interned goes through the new hash function. A regression here affects ALL
compilation. Conversely, the import boundary optimization could be a significant win, but
only if the hash hit rate is high enough. Benchmarks prove both claims.

**This section** runs after all implementation sections (01-06) are complete.

---

## 07.1 Interning Throughput Benchmark — NOT STARTED

**Goal:** Measure whether Merkle hash computation is the same speed as the current
`compute_hash` (it should be — same FxHash, similar data volume).

**Benchmark:**
```rust
// compiler/oric/benches/pool_interning.rs

fn bench_intern_primitives(c: &mut Criterion) {
    c.bench_function("intern_primitives", |b| {
        b.iter(|| {
            let pool = Pool::new();  // interns 12 primitives
            black_box(pool);
        });
    });
}

fn bench_intern_containers(c: &mut Criterion) {
    c.bench_function("intern_100_containers", |b| {
        b.iter(|| {
            let mut pool = Pool::new();
            for &p in &[Idx::INT, Idx::FLOAT, Idx::BOOL, Idx::STR, Idx::CHAR] {
                let _ = pool.list(p);
                let _ = pool.option(p);
                let _ = pool.set(p);
                let _ = pool.iterator(p);
            }
            // Nested
            for &p in &[Idx::INT, Idx::STR] {
                let list = pool.list(p);
                let _ = pool.option(list);
                let _ = pool.list(list);
                let map = pool.map(p, Idx::INT);
                let _ = pool.list(map);
            }
            black_box(pool);
        });
    });
}

fn bench_intern_functions(c: &mut Criterion) {
    c.bench_function("intern_50_functions", |b| {
        b.iter(|| {
            let mut pool = Pool::new();
            for i in 0..50 {
                let params: Vec<Idx> = (0..((i % 5) + 1))
                    .map(|j| Idx::from_raw(j % 12))
                    .collect();
                let ret = Idx::from_raw(i % 12);
                let _ = pool.function(&params, ret);
            }
            black_box(pool);
        });
    });
}
```

**Expected result:** ≤ 5% difference from current implementation. Merkle hashing does one
extra array lookup per child (`self.hashes[child_idx]`) compared to using `child_idx` directly,
but this is a single L1-cache-hit memory access — negligible.

**Acceptable threshold:** ≤ 10% regression in interning throughput. If exceeded, investigate
cache miss patterns or consider pre-computing child hashes in a temporary array.

**File:** `compiler/oric/benches/pool_interning.rs` (new benchmark file)

**Exit Criteria:**
- [ ] Interning benchmark implemented
- [ ] ≤ 10% throughput regression vs baseline (current compute_hash)
- [ ] Results documented with numbers

---

## 07.2 Import Boundary Benchmark — NOT STARTED

**Goal:** Measure the wall-clock improvement from hash-first import resolution (Section 04).

**Benchmark design:**

```rust
// compiler/oric/benches/import_resolution.rs

fn bench_import_resolution(c: &mut Criterion) {
    let mut group = c.benchmark_group("import_resolution");

    // Setup: create a "source module" with 50 functions
    let (functions, arena, typed_result, source_pool) = setup_source_module();

    // Baseline: AST-walking import
    group.bench_function("ast_walk", |b| {
        b.iter(|| {
            let mut checker = ModuleChecker::new(/* ... */);
            for func in &functions {
                checker.register_imported_function(func, &arena, None);
            }
            black_box(&checker);
        });
    });

    // Optimized: Hash-first import (warm cache — prelude already loaded)
    group.bench_function("hash_first_warm", |b| {
        b.iter(|| {
            let mut checker = ModuleChecker::new(/* ... */);
            // Pre-warm with prelude (simulates real module)
            warm_prelude(&mut checker);
            for (func, sig) in functions.iter().zip(&typed_result.functions) {
                checker.register_imported_function(func, &arena, Some(sig));
            }
            black_box(&checker);
        });
    });

    // Optimized: Hash-first import (cold cache — no prelude)
    group.bench_function("hash_first_cold", |b| {
        b.iter(|| {
            let mut checker = ModuleChecker::new(/* ... */);
            for (func, sig) in functions.iter().zip(&typed_result.functions) {
                checker.register_imported_function(func, &arena, Some(sig));
            }
            black_box(&checker);
        });
    });

    group.finish();
}
```

**Metrics to capture:**
- Time per import (ns) for each path
- Hash hit rate (% of types resolved by hash vs fallback)
- Speedup factor: `ast_walk / hash_first_warm`

**Expected results:**
- Warm cache (typical case): 2-5x speedup
- Cold cache (first import of novel types): ~1x (no improvement, hash miss → AST fallback)
- Overall for real-world modules: ~2-3x import speedup

**Function signature complexity breakdown:**
- Simple (0-1 params, primitive types): measure separately
- Medium (2-3 params, container types): measure separately
- Complex (3+ params, nested generics): measure separately

**Exit Criteria:**
- [ ] Import benchmark implemented with warm/cold/baseline paths
- [ ] ≥ 2x speedup for warm-cache imports
- [ ] Hash hit rate ≥ 80% for warm-cache scenario
- [ ] Results documented with numbers

---

## 07.3 Cross-Module Comparison Benchmark — NOT STARTED

**Goal:** Measure the cost of cross-module type comparison: Merkle hash comparison (O(1))
vs structural comparison (O(depth)).

**Benchmark:**
```rust
fn bench_cross_module_type_eq(c: &mut Criterion) {
    let mut group = c.benchmark_group("cross_module_type_eq");

    let mut p1 = Pool::new();
    let mut p2 = Pool::new();

    // Create complex types in both pools (different Idx values)
    // Shift p2 to ensure different indices
    for _ in 0..50 { let _ = p2.list(Idx::FLOAT); }

    let types_p1 = create_type_set(&mut p1);   // 100 types of varying depth
    let types_p2 = create_type_set(&mut p2);   // Same 100 types, different Idx

    // Merkle hash comparison: O(1)
    group.bench_function("merkle_hash_compare", |b| {
        b.iter(|| {
            for (&idx1, &idx2) in types_p1.iter().zip(&types_p2) {
                black_box(p1.hash(idx1) == p2.hash(idx2));
            }
        });
    });

    // Structural comparison: O(depth) per pair
    group.bench_function("structural_compare", |b| {
        b.iter(|| {
            for (&idx1, &idx2) in types_p1.iter().zip(&types_p2) {
                black_box(structural_eq(&p1, idx1, &p2, idx2));
            }
        });
    });

    group.finish();
}
```

**Expected results:**
- Merkle hash: ~1-2ns per comparison (single u64 comparison)
- Structural: ~50-500ns per comparison (recursive traversal, cache misses)
- Speedup: 25-250x

**Exit Criteria:**
- [ ] Cross-module comparison benchmark implemented
- [ ] Merkle hash comparison ≥ 10x faster than structural
- [ ] Results documented

---

## 07.4 Memory Usage Analysis — NOT STARTED

**Goal:** Measure memory impact of Merkle hashing (should be zero — same data structures,
different hash values).

**What to measure:**
- `Pool` memory before/after: `size_of::<Pool>()` + heap allocations
- `FunctionSig` memory before/after: two new `Vec<u64>` + one `u64`
- `TypedModule` memory before/after: `type_descriptors` field (Section 05 only)

**Expected results:**
- Pool memory: unchanged (hashes already stored in `Vec<u64>`, just different values)
- FunctionSig memory: +8 bytes per param + 8 bytes for return hash
  - Typical function with 3 params: +32 bytes
  - 50 functions per module: +1.6KB per module
- TypedModule memory (with descriptors): +~1KB per module
- Total per module: +~3KB — negligible

**Measurement:**
```rust
#[test]
fn pool_memory_unchanged() {
    let mut pool = Pool::new();
    // Intern 100 types
    let baseline_size = pool.memory_usage();  // need to add this method

    // Verify size is within expected range
    // (item: 5 bytes + flags: 1 byte + hash: 8 bytes) × 100 = ~1.4KB
    // + extra array + intern_map overhead
    assert!(baseline_size < 10_000, "Pool too large: {baseline_size} bytes");
}
```

**Exit Criteria:**
- [ ] Pool memory unchanged from baseline
- [ ] FunctionSig memory increase documented and acceptable
- [ ] Total per-module memory increase < 5KB
- [ ] No unexpected allocation patterns

---

## 07.5 Regression Testing — NOT STARTED

**Goal:** Verify zero regressions across the entire test suite.

**Test commands (all must pass):**
```bash
cargo t                          # All Rust unit tests
cargo st                         # All Ori spec tests
./test-all.sh                    # Full suite (Rust + spec + clippy + fmt)
./llvm-test.sh                   # LLVM backend tests
./scripts/valgrind-aot.sh        # Memory safety (ARC correctness)
./scripts/dual-exec-verify.sh    # JIT vs AOT behavioral equivalence
cargo bench -p oric --bench parser   # Parser throughput (no regression)
cargo bench -p oric --bench lexer    # Lexer throughput (no regression)
```

**Critical regression scenarios:**
1. **Type deduplication still works:** Same type interned twice returns same Idx
2. **Type equality still works:** `idx1 == idx2` for same type (pool-local)
3. **Import resolution still correct:** Imported function signatures resolve correctly
4. **Codegen still correct:** LLVM IR generation produces correct code
5. **ARC still correct:** Reference counting operations correct (Valgrind clean)
6. **Spec tests still pass:** User-visible behavior unchanged

**Exit Criteria:**
- [ ] All test commands pass
- [ ] No parser/lexer benchmark regressions (≤ 5% noise margin)
- [ ] Valgrind clean
- [ ] Dual-execution verification clean

---

## 07.6 Exit Criteria — NOT STARTED

**The Merkle Pool Identity project is COMPLETE when ALL of the following are true:**

### Correctness
- [ ] Same type structure → same Merkle hash (cross-pool stability proven by 20+ tests)
- [ ] No hash collisions in test suite (500+ distinct types, zero collisions)
- [ ] Structural equality ↔ hash equality (cross-checked for 100+ types)
- [ ] All existing tests pass unchanged (`./test-all.sh`, `./llvm-test.sh`)
- [ ] Valgrind clean (`./scripts/valgrind-aot.sh`)
- [ ] Dual-execution clean (`./scripts/dual-exec-verify.sh`)

### Performance
- [ ] Interning throughput: ≤ 10% regression from baseline
- [ ] Import resolution: ≥ 2x speedup for warm-cache imports
- [ ] Cross-module type comparison: ≥ 10x speedup vs structural
- [ ] Memory increase: < 5KB per module

### Architecture
- [ ] No dual-pool code paths in LLVM backend
- [ ] No dual-pool code paths in ARC lowering
- [ ] No dual-pool code paths in evaluator
- [ ] `ImportedFunctionForCodegen` has no `pool` field
- [ ] FunctionSig carries Merkle hashes for cross-module transport
- [ ] `Pool::lookup_by_hash()` available for O(1) type resolution
- [ ] All 44 Tag variants correctly classified (child-in-data vs children-in-extra vs leaf)

### Documentation
- [ ] Plan sections updated with completion status
- [ ] Benchmark results recorded with numbers
- [ ] MEMORY.md updated with Merkle hashing design notes

### Optional (Section 05)
- [ ] Portable TypeDescriptors implemented
- [ ] Zero-AST import path working
- [ ] Round-trip test: describe → reconstruct → verify

---

## Section 07 Completion Checklist

- [ ] Interning throughput benchmark implemented and passing (07.1)
- [ ] Import boundary benchmark implemented and showing ≥ 2x speedup (07.2)
- [ ] Cross-module comparison benchmark showing ≥ 10x speedup (07.3)
- [ ] Memory analysis complete and acceptable (07.4)
- [ ] Full regression suite passing (07.5)
- [ ] All exit criteria met (07.6)
- [ ] Results documented in this file with actual numbers
