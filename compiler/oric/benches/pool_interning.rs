//! Pool interning and cross-module type identity benchmarks.
//!
//! Measures:
//! - Interning throughput (primitives, containers, functions, structs)
//! - Re-interning throughput (cross-pool Merkle hash fast path)
//! - Cross-module type comparison (Merkle hash O(1) vs structural O(depth))

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, Criterion};
use ori_types::{re_intern_type, Idx, Pool};
use rustc_hash::FxHashMap;

/// Benchmark: create a fresh pool (interns 12 primitives).
fn bench_intern_primitives(c: &mut Criterion) {
    c.bench_function("pool/intern_primitives", |b| {
        b.iter(|| {
            let pool = Pool::new();
            black_box(pool);
        });
    });
}

/// Benchmark: intern 100 container types (List, Option, Set, Iterator + nested).
fn bench_intern_containers(c: &mut Criterion) {
    c.bench_function("pool/intern_100_containers", |b| {
        b.iter(|| {
            let mut pool = Pool::new();
            for &p in &[Idx::INT, Idx::FLOAT, Idx::BOOL, Idx::STR, Idx::CHAR] {
                let _ = pool.list(p);
                let _ = pool.option(p);
                let _ = pool.set(p);
                let _ = pool.iterator(p);
            }
            // Nested containers
            for &p in &[Idx::INT, Idx::STR, Idx::BOOL, Idx::FLOAT, Idx::CHAR] {
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

/// Benchmark: intern 50 function types with varying parameter counts.
fn bench_intern_functions(c: &mut Criterion) {
    c.bench_function("pool/intern_50_functions", |b| {
        b.iter(|| {
            let mut pool = Pool::new();
            let prims = [
                Idx::INT,
                Idx::FLOAT,
                Idx::BOOL,
                Idx::STR,
                Idx::CHAR,
                Idx::UNIT,
            ];
            for i in 0..50 {
                let param_count = (i % 5) + 1;
                let params: Vec<Idx> = (0..param_count)
                    .map(|j| prims[(i + j) % prims.len()])
                    .collect();
                let ret = prims[i % prims.len()];
                let _ = pool.function(&params, ret);
            }
            black_box(pool);
        });
    });
}

/// Create a diverse type set in a pool for cross-pool benchmarks.
///
/// Returns the pool and a Vec of Idx handles for 100 types of varying depth.
fn create_type_set() -> (Pool, Vec<Idx>) {
    let mut pool = Pool::new();
    let mut types = Vec::with_capacity(100);

    // Primitives (12)
    for i in 0..12 {
        types.push(Idx::from_raw(i));
    }

    // Simple containers (20)
    for &p in &[Idx::INT, Idx::FLOAT, Idx::BOOL, Idx::STR, Idx::CHAR] {
        types.push(pool.list(p));
        types.push(pool.option(p));
        types.push(pool.set(p));
        types.push(pool.iterator(p));
    }

    // Two-child containers (10)
    for &p in &[Idx::INT, Idx::FLOAT, Idx::BOOL, Idx::STR, Idx::CHAR] {
        types.push(pool.map(p, Idx::INT));
        types.push(pool.result(p, Idx::STR));
    }

    // Nested containers (10)
    for &p in &[Idx::INT, Idx::STR] {
        let list = pool.list(p);
        let nested = pool.list(list);
        types.push(nested);
        let opt_list = pool.option(list);
        types.push(opt_list);
        let map = pool.map(p, list);
        types.push(map);
        let inner_map = pool.map(p, Idx::BOOL);
        let inner_opt = pool.option(inner_map);
        let deep = pool.list(inner_opt);
        types.push(deep);
        let result_nested = pool.result(list, Idx::STR);
        types.push(result_nested);
    }

    // Functions (20)
    for i in 0u32..20 {
        let param_count = (i % 4) + 1;
        let params: Vec<Idx> = (0..param_count)
            .map(|j| Idx::from_raw((i + j) % 12))
            .collect();
        let ret = Idx::from_raw(i % 12);
        types.push(pool.function(&params, ret));
    }

    // Tuples (10)
    for i in 0u32..10 {
        let elem_count = (i % 3) + 2;
        let elems: Vec<Idx> = (0..elem_count)
            .map(|j| Idx::from_raw((i + j) % 12))
            .collect();
        types.push(pool.tuple(&elems));
    }

    // Fill to ~100
    while types.len() < 100 {
        let base = types[types.len() % 32 + 12]; // pick a non-primitive
        types.push(pool.list(base));
    }

    (pool, types)
}

/// Benchmark: re-intern types from source pool into target pool.
///
/// Measures the Merkle hash fast path (warm cache — target already has
/// the same types) and the structural walk (cold — target is fresh).
fn bench_re_intern(c: &mut Criterion) {
    let (source_pool, source_types) = create_type_set();

    // Warm target: clone of source, so all hashes already exist → O(1) fast path
    c.bench_function("pool/re_intern_warm_100_types", |b| {
        b.iter(|| {
            let mut target = source_pool.clone();
            let mut cache = FxHashMap::default();
            for &idx in &source_types {
                let result = re_intern_type(&source_pool, idx, &mut target, &mut cache);
                black_box(result);
            }
        });
    });

    // Cold target: fresh pool with only primitives → structural walk fallback
    c.bench_function("pool/re_intern_cold_100_types", |b| {
        b.iter(|| {
            let mut target = Pool::new();
            let mut cache = FxHashMap::default();
            for &idx in &source_types {
                let result = re_intern_type(&source_pool, idx, &mut target, &mut cache);
                black_box(result);
            }
        });
    });
}

/// Benchmark: cross-module type comparison using Merkle hashes.
///
/// Compares the cost of Merkle hash equality (O(1) u64 comparison) against
/// structural equality (recursive traversal). Both pools contain the same
/// types at different Idx positions.
fn bench_cross_module_compare(c: &mut Criterion) {
    let (pool1, types1) = create_type_set();

    // Create pool2 with offset indices: intern dummy types first to shift positions
    let mut pool2 = Pool::new();
    // Add 50 dummy types to shift all Idx values
    for i in 0u32..50 {
        let _ = pool2.list(Idx::from_raw(i % 12));
    }
    // Now re-create the same type set — same structure, different Idx values
    let mut cache = FxHashMap::default();
    let types2: Vec<Idx> = types1
        .iter()
        .map(|&idx| re_intern_type(&pool1, idx, &mut pool2, &mut cache))
        .collect();

    // Verify setup: same hashes, different Idx values (for non-primitives)
    for (i, (&idx1, &idx2)) in types1.iter().zip(&types2).enumerate() {
        assert_eq!(
            pool1.hash(idx1),
            pool2.hash(idx2),
            "Hash mismatch at index {i}"
        );
    }

    // Benchmark: Merkle hash comparison (O(1))
    c.bench_function("pool/cross_module_merkle_hash_100", |b| {
        b.iter(|| {
            let mut matches = 0u32;
            for (&idx1, &idx2) in types1.iter().zip(&types2) {
                if pool1.hash(idx1) == pool2.hash(idx2) {
                    matches += 1;
                }
            }
            black_box(matches);
        });
    });

    // Benchmark: structural comparison via re-interning (O(depth))
    // This simulates what you'd have to do without Merkle hashes:
    // re-intern each type and check if the result matches
    c.bench_function("pool/cross_module_structural_100", |b| {
        b.iter(|| {
            let mut matches = 0u32;
            let mut target = pool1.clone();
            let mut re_cache = FxHashMap::default();
            for (&idx1, &idx2) in types1.iter().zip(&types2) {
                let re_interned = re_intern_type(&pool2, idx2, &mut target, &mut re_cache);
                if re_interned == idx1 {
                    matches += 1;
                }
            }
            black_box(matches);
        });
    });
}

/// Benchmark: deduplication (interning the same type twice returns same Idx).
fn bench_dedup(c: &mut Criterion) {
    c.bench_function("pool/dedup_100_types", |b| {
        b.iter(|| {
            let mut pool = Pool::new();
            // First pass: intern 100 container types
            let mut first_pass = Vec::with_capacity(100);
            for i in 0u32..100 {
                let prim = Idx::from_raw(i % 12);
                first_pass.push(pool.list(prim));
            }
            // Second pass: intern same types again — should all be deduped
            let mut second_pass = Vec::with_capacity(100);
            for i in 0u32..100 {
                let prim = Idx::from_raw(i % 12);
                second_pass.push(pool.list(prim));
            }
            // Verify dedup works
            for (a, b) in first_pass.iter().zip(&second_pass) {
                debug_assert_eq!(a, b);
            }
            black_box((first_pass, second_pass));
        });
    });
}

criterion_group!(
    benches,
    bench_intern_primitives,
    bench_intern_containers,
    bench_intern_functions,
    bench_re_intern,
    bench_cross_module_compare,
    bench_dedup,
);
criterion_main!(benches);
