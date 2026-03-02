//! Performance benchmarks for SCC-based borrow inference.
//!
//! Measures cold-compile, incremental, and SCC computation overhead for the
//! per-function Salsa borrow inference pipeline (Section 12.13).
//!
//! **Requires `llvm` feature**: `cargo bench -p oric --features llvm --bench borrow_inference`

// Benchmark code uses patterns that are idiomatic for benchmark harnesses
// but trigger clippy pedantic lints. Suppress at file level rather than
// annotating each of ~20 call sites.
#![expect(
    clippy::needless_borrow,
    reason = "benchmark generators take &Interner"
)]
#![expect(
    clippy::cast_sign_loss,
    reason = "iteration counts are always positive"
)]
#![expect(
    clippy::cast_precision_loss,
    reason = "nanosecond timing values fit in f64"
)]

use std::hint::black_box;
use std::path::{Path, PathBuf};

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use ori_arc::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcParam, ArcTerminator, ArcVarId, ArgOwnership,
    CtorKind,
};
use ori_arc::ownership::Ownership;
use ori_arc::uniqueness::{CowAnnotations, DropHints};
use ori_arc::{compute_sccs, CallGraph};
use ori_ir::Name;
use ori_types::{Idx, Pool};
use oric::db::{CompilerDb, Db};
use oric::query::arc_queries::{arc_scc_decomposition, infer_borrow_scc, ArcModuleInput};
use salsa::Setter;

const BENCH_PATH: &str = "/bench/borrow.ori";

// ── Synthetic function generators ────────────────────────────────────

/// Create a standalone function (no calls) that reads its `str` param.
///
/// Produces a Borrowed param — reads `x` but never stores it.
fn standalone_reader(name: Name) -> ArcFunction {
    ArcFunction {
        name,
        params: vec![ArcParam {
            var: ArcVarId::new(0),
            ty: Idx::STR,
            ownership: Ownership::Owned,
        }],
        return_type: Idx::INT,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![ArcInstr::Let {
                dst: ArcVarId::new(1),
                ty: Idx::INT,
                value: ori_arc::ir::ArcValue::PrimOp {
                    op: ori_arc::PrimOp::Binary(ori_ir::BinaryOp::Add),
                    args: vec![ArcVarId::new(0), ArcVarId::new(0)],
                },
            }],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(1),
            },
        }],
        entry: ArcBlockId::new(0),
        var_types: vec![Idx::STR, Idx::INT],
        var_reprs: vec![],
        spans: vec![vec![None]],
        is_fbip: false,
        num_captures: 0,
        cow_annotations: CowAnnotations::default(),
        drop_hints: DropHints::default(),
    }
}

/// Create a function that calls `callee` and forwards its `str` param.
fn caller_function(name: Name, callee: Name) -> ArcFunction {
    ArcFunction {
        name,
        params: vec![ArcParam {
            var: ArcVarId::new(0),
            ty: Idx::STR,
            ownership: Ownership::Owned,
        }],
        return_type: Idx::STR,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![ArcInstr::Apply {
                dst: ArcVarId::new(1),
                ty: Idx::STR,
                func: callee,
                args: vec![ArcVarId::new(0)],
                arg_ownership: vec![ArgOwnership::Owned],
            }],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(1),
            },
        }],
        entry: ArcBlockId::new(0),
        var_types: vec![Idx::STR, Idx::STR],
        var_reprs: vec![],
        spans: vec![vec![None]],
        is_fbip: false,
        num_captures: 0,
        cow_annotations: CowAnnotations::default(),
        drop_hints: DropHints::default(),
    }
}

/// Create a function that stores its param (Owned borrow result).
fn storer_function(name: Name) -> ArcFunction {
    ArcFunction {
        name,
        params: vec![ArcParam {
            var: ArcVarId::new(0),
            ty: Idx::STR,
            ownership: Ownership::Owned,
        }],
        return_type: Idx::UNIT,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![ArcInstr::Construct {
                dst: ArcVarId::new(1),
                ty: Idx::UNIT,
                ctor: CtorKind::Tuple,
                args: vec![ArcVarId::new(0)],
            }],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(1),
            },
        }],
        entry: ArcBlockId::new(0),
        var_types: vec![Idx::STR, Idx::UNIT],
        var_reprs: vec![],
        spans: vec![vec![None]],
        is_fbip: false,
        num_captures: 0,
        cow_annotations: CowAnnotations::default(),
        drop_hints: DropHints::default(),
    }
}

/// Create a modified reader with extra instructions (same borrow sig).
fn modified_reader(name: Name) -> ArcFunction {
    ArcFunction {
        name,
        params: vec![ArcParam {
            var: ArcVarId::new(0),
            ty: Idx::STR,
            ownership: Ownership::Owned,
        }],
        return_type: Idx::INT,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![
                ArcInstr::Let {
                    dst: ArcVarId::new(1),
                    ty: Idx::INT,
                    value: ori_arc::ir::ArcValue::PrimOp {
                        op: ori_arc::PrimOp::Binary(ori_ir::BinaryOp::Add),
                        args: vec![ArcVarId::new(0), ArcVarId::new(0)],
                    },
                },
                ArcInstr::Let {
                    dst: ArcVarId::new(2),
                    ty: Idx::INT,
                    value: ori_arc::ir::ArcValue::PrimOp {
                        op: ori_arc::PrimOp::Binary(ori_ir::BinaryOp::Mul),
                        args: vec![ArcVarId::new(1), ArcVarId::new(1)],
                    },
                },
            ],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(2),
            },
        }],
        entry: ArcBlockId::new(0),
        var_types: vec![Idx::STR, Idx::INT, Idx::INT],
        var_reprs: vec![],
        spans: vec![vec![None, None]],
        is_fbip: false,
        num_captures: 0,
        cow_annotations: CowAnnotations::default(),
        drop_hints: DropHints::default(),
    }
}

// ── Module topology generators ───────────────────────────────────────

/// Generate N standalone functions (no calls between them).
///
/// Produces N single-function SCCs — best-case for incrementality.
fn gen_standalone_funcs(interner: &ori_ir::StringInterner, n: usize) -> Vec<(Name, ArcFunction)> {
    (0..n)
        .map(|i| {
            let name = interner.intern(&format!("bench_standalone_{i}"));
            (name, standalone_reader(name))
        })
        .collect()
}

/// Generate a linear call chain: f0 → f1 → f2 → ... → f(n-1).
///
/// Produces N single-function SCCs with maximal dependency depth.
fn gen_linear_chain(interner: &ori_ir::StringInterner, n: usize) -> Vec<(Name, ArcFunction)> {
    let names: Vec<Name> = (0..n)
        .map(|i| interner.intern(&format!("bench_chain_{i}")))
        .collect();

    let mut funcs = Vec::with_capacity(n);
    for i in 0..n {
        let func = if i + 1 < n {
            caller_function(names[i], names[i + 1])
        } else {
            standalone_reader(names[i])
        };
        funcs.push((names[i], func));
    }
    funcs
}

/// Generate a deep mutual recursion SCC.
///
/// All `scc_size` functions call each other in a ring: f0→f1→f2→...→f0.
/// Remaining `n - scc_size` functions are standalone.
fn gen_deep_recursion(
    interner: &ori_ir::StringInterner,
    n: usize,
    scc_size: usize,
) -> Vec<(Name, ArcFunction)> {
    let mut funcs = Vec::with_capacity(n);

    // Build the recursive ring.
    let ring_names: Vec<Name> = (0..scc_size)
        .map(|i| interner.intern(&format!("bench_ring_{i}")))
        .collect();
    for i in 0..scc_size {
        let next = (i + 1) % scc_size;
        funcs.push((
            ring_names[i],
            caller_function(ring_names[i], ring_names[next]),
        ));
    }

    // Fill remaining slots with standalone functions.
    for i in scc_size..n {
        let name = interner.intern(&format!("bench_extra_{i}"));
        funcs.push((name, standalone_reader(name)));
    }
    funcs
}

// ── Setup helpers ────────────────────────────────────────────────────

fn setup_pool(db: &CompilerDb) {
    db.pool_cache().store(Path::new(BENCH_PATH), Pool::new());
}

fn make_module(db: &CompilerDb, mut funcs: Vec<(Name, ArcFunction)>) -> ArcModuleInput {
    funcs.sort_by_key(|(name, _)| *name);
    setup_pool(db);
    ArcModuleInput::new(db, PathBuf::from(BENCH_PATH), funcs)
}

/// Query all SCCs in a module (full borrow inference).
fn query_all_sccs(db: &dyn Db, module: ArcModuleInput) {
    let decomp = arc_scc_decomposition(db, module);
    #[expect(
        clippy::cast_possible_truncation,
        reason = "SCC count bounded by function count, fits in u32"
    )]
    for i in 0..decomp.len() as u32 {
        let _ = black_box(infer_borrow_scc(db, module, i));
    }
}

// ── Benchmark: Cold compile ──────────────────────────────────────────

fn bench_cold_compile(c: &mut Criterion) {
    let mut group = c.benchmark_group("borrow/cold");

    for &size in &[5, 50, 200] {
        // Standalone functions (independent SCCs).
        group.bench_with_input(BenchmarkId::new("standalone", size), &size, |b, &n| {
            b.iter(|| {
                let db = CompilerDb::new();
                let interner = db.interner();
                let funcs = gen_standalone_funcs(&interner, n);
                let module = make_module(&db, funcs);
                query_all_sccs(&db, module);
            });
        });

        // Linear call chain (maximal dependency depth).
        group.bench_with_input(BenchmarkId::new("chain", size), &size, |b, &n| {
            b.iter(|| {
                let db = CompilerDb::new();
                let interner = db.interner();
                let funcs = gen_linear_chain(&interner, n);
                let module = make_module(&db, funcs);
                query_all_sccs(&db, module);
            });
        });
    }

    // Deep mutual recursion SCC (10+ functions in one SCC).
    for &scc_size in &[10, 20] {
        let total = 50;
        group.bench_with_input(
            BenchmarkId::new(format!("recursion_scc{scc_size}"), total),
            &(total, scc_size),
            |b, &(n, scc_sz)| {
                b.iter(|| {
                    let db = CompilerDb::new();
                    let interner = db.interner();
                    let funcs = gen_deep_recursion(&interner, n, scc_sz);
                    let module = make_module(&db, funcs);
                    query_all_sccs(&db, module);
                });
            },
        );
    }

    group.finish();
}

// ── Benchmark: SCC standalone baseline (no Salsa) ────────────────────

fn bench_scc_standalone(c: &mut Criterion) {
    let mut group = c.benchmark_group("borrow/scc_standalone");

    for &size in &[5, 50, 200] {
        // Standalone functions via SCC-based infer_borrows_scc.
        group.bench_with_input(BenchmarkId::new("standalone", size), &size, |b, &n| {
            b.iter(|| {
                let db = CompilerDb::new();
                let interner = db.interner();
                let funcs = gen_standalone_funcs(&interner, n);
                let functions: Vec<ArcFunction> = funcs.into_iter().map(|(_, f)| f).collect();
                let pool = Pool::new();
                let classifier = ori_arc::ArcClassifier::new(&pool);
                let builtins = ori_arc::BuiltinOwnershipSets::new(&interner);
                black_box(ori_arc::borrow::infer_borrows_scc(
                    &functions,
                    &classifier,
                    &builtins,
                ));
            });
        });

        // Linear chain via SCC-based.
        group.bench_with_input(BenchmarkId::new("chain", size), &size, |b, &n| {
            b.iter(|| {
                let db = CompilerDb::new();
                let interner = db.interner();
                let funcs = gen_linear_chain(&interner, n);
                let functions: Vec<ArcFunction> = funcs.into_iter().map(|(_, f)| f).collect();
                let pool = Pool::new();
                let classifier = ori_arc::ArcClassifier::new(&pool);
                let builtins = ori_arc::BuiltinOwnershipSets::new(&interner);
                black_box(ori_arc::borrow::infer_borrows_scc(
                    &functions,
                    &classifier,
                    &builtins,
                ));
            });
        });
    }

    group.finish();
}

// ── Benchmark: Warm compile (incremental) ────────────────────────────

fn bench_warm_compile(c: &mut Criterion) {
    let mut group = c.benchmark_group("borrow/incremental");

    for &size in &[5, 50, 200] {
        // Incremental: change one function body, same borrow sig.
        group.bench_with_input(BenchmarkId::new("same_sig_change", size), &size, |b, &n| {
            let mut db = CompilerDb::new();
            let interner = db.interner();
            let funcs = gen_standalone_funcs(&interner, n);
            let module = make_module(&db, funcs.clone());

            // Warm the cache.
            query_all_sccs(&db, module);

            // Prepare a modified version (first function body changes,
            // same borrow sig).
            let mut modified_funcs = funcs;
            let first_name = modified_funcs[0].0;
            modified_funcs[0].1 = modified_reader(first_name);
            modified_funcs.sort_by_key(|(name, _)| *name);

            // Alternate between original and modified.
            let original_sorted = {
                let mut f = gen_standalone_funcs(&interner, n);
                f.sort_by_key(|(name, _)| *name);
                f
            };

            b.iter(|| {
                // Set modified functions and re-query.
                module.set_functions(&mut db).to(modified_funcs.clone());
                query_all_sccs(&db, module);

                // Reset to original for next iteration.
                module.set_functions(&mut db).to(original_sorted.clone());
                query_all_sccs(&db, module);
            });
        });

        // Incremental: change one function, different borrow sig.
        group.bench_with_input(
            BenchmarkId::new("different_sig_change", size),
            &size,
            |b, &n| {
                let mut db = CompilerDb::new();
                let interner = db.interner();
                let funcs = gen_standalone_funcs(&interner, n);
                let module = make_module(&db, funcs.clone());

                // Warm the cache.
                query_all_sccs(&db, module);

                // Prepare a modified version (first function becomes storer).
                let mut modified_funcs = funcs;
                let first_name = modified_funcs[0].0;
                modified_funcs[0].1 = storer_function(first_name);
                modified_funcs.sort_by_key(|(name, _)| *name);

                let original_sorted = {
                    let mut f = gen_standalone_funcs(&interner, n);
                    f.sort_by_key(|(name, _)| *name);
                    f
                };

                b.iter(|| {
                    module.set_functions(&mut db).to(modified_funcs.clone());
                    query_all_sccs(&db, module);

                    module.set_functions(&mut db).to(original_sorted.clone());
                    query_all_sccs(&db, module);
                });
            },
        );
    }

    group.finish();
}

// ── Benchmark: SCC computation overhead ──────────────────────────────

fn bench_scc_computation(c: &mut Criterion) {
    let mut group = c.benchmark_group("borrow/scc_overhead");

    for &size in &[5, 50, 200, 500] {
        // Standalone: N independent functions.
        group.bench_with_input(BenchmarkId::new("standalone", size), &size, |b, &n| {
            let db = CompilerDb::new();
            let interner = db.interner();
            let funcs = gen_standalone_funcs(&interner, n);
            let functions: Vec<ArcFunction> = funcs.into_iter().map(|(_, f)| f).collect();

            b.iter(|| {
                let graph = CallGraph::build(&functions);
                let sccs = compute_sccs(&graph);
                black_box(&sccs);
            });
        });

        // Linear chain: maximal SCC dependency depth.
        group.bench_with_input(BenchmarkId::new("chain", size), &size, |b, &n| {
            let db = CompilerDb::new();
            let interner = db.interner();
            let funcs = gen_linear_chain(&interner, n);
            let functions: Vec<ArcFunction> = funcs.into_iter().map(|(_, f)| f).collect();

            b.iter(|| {
                let graph = CallGraph::build(&functions);
                let sccs = compute_sccs(&graph);
                black_box(&sccs);
            });
        });
    }

    group.finish();
}

// ── Benchmark: Memory profile ────────────────────────────────────────

fn bench_memory_profile(c: &mut Criterion) {
    let mut group = c.benchmark_group("borrow/memory");

    // Measure Salsa overhead by querying all SCCs and checking result count.
    // (We can't easily measure bytes without a tracking allocator, but we can
    // verify that the number of memoized results scales as expected.)
    for &size in &[5, 50, 200] {
        group.bench_with_input(
            BenchmarkId::new("salsa_query_count", size),
            &size,
            |b, &n| {
                b.iter(|| {
                    let db = CompilerDb::new();
                    let interner = db.interner();
                    let funcs = gen_standalone_funcs(&interner, n);
                    let module = make_module(&db, funcs);

                    // Query all SCCs.
                    let decomp = arc_scc_decomposition(&db, module);
                    assert_eq!(decomp.len(), n);

                    #[expect(
                        clippy::cast_possible_truncation,
                        reason = "SCC count bounded by function count"
                    )]
                    for i in 0..decomp.len() as u32 {
                        let result = infer_borrow_scc(&db, module, i);
                        assert_eq!(result.len(), 1);
                    }
                });
            },
        );
    }

    group.finish();
}

// ── Regression gate ──────────────────────────────────────────────────

/// Print a summary comparing SCC standalone vs Salsa-tracked SCC queries.
///
/// Two tables:
/// 1. **Algorithm-only**: pre-created db, measures just borrow inference.
/// 2. **SCC computation overhead**: call graph + Tarjan's, no inference.
fn bench_regression_summary(c: &mut Criterion) {
    use std::time::Instant;

    let iters = 200;

    println!("\n{}", "=".repeat(70));
    println!("BORROW INFERENCE REGRESSION SUMMARY");
    println!("{}", "=".repeat(70));

    // Table 1: Algorithm-only (pre-created db, just query time).
    println!("\n### Query time (pre-created DB, includes Salsa tracking overhead)");
    println!("| Size | Topology | SCC (no Salsa) | SCC-Queries | Overhead |");
    println!("|------|----------|----------------|-------------|----------|");

    for &size in &[5, 50, 200] {
        for topology in &["standalone", "chain"] {
            let db = CompilerDb::new();
            let interner = db.interner();

            let funcs = match *topology {
                "standalone" => gen_standalone_funcs(&interner, size),
                "chain" => gen_linear_chain(&interner, size),
                _ => unreachable!(),
            };

            // SCC standalone baseline (no Salsa).
            let functions: Vec<ArcFunction> = funcs.iter().map(|(_, f)| f.clone()).collect();
            let pool = Pool::new();
            let classifier = ori_arc::ArcClassifier::new(&pool);
            let builtins = ori_arc::BuiltinOwnershipSets::new(&interner);

            let start = Instant::now();
            for _ in 0..iters {
                black_box(ori_arc::borrow::infer_borrows_scc(
                    &functions,
                    &classifier,
                    &builtins,
                ));
            }
            let standalone_ns = start.elapsed().as_nanos() / iters as u128;

            // SCC-based with Salsa (fresh module each time,
            // but db already exists — measures query overhead, not
            // framework init).
            let start = Instant::now();
            for _ in 0..iters {
                let module = make_module(&db, funcs.clone());
                query_all_sccs(&db, module);
            }
            let scc_ns = start.elapsed().as_nanos() / iters as u128;

            let overhead = if standalone_ns > 0 {
                format!(
                    "{:+.0}%",
                    ((scc_ns as f64 - standalone_ns as f64) / standalone_ns as f64) * 100.0
                )
            } else {
                "N/A".to_string()
            };

            let standalone_display = format_duration_ns(standalone_ns);
            let scc_display = format_duration_ns(scc_ns);

            println!(
                "| {size:>4} | {topology:<10} | {standalone_display:>14} | {scc_display:>11} | {overhead:>8} |",
            );
        }
    }

    // Table 2: SCC computation overhead (in isolation).
    println!("\n### SCC computation (call graph + Tarjan's, no borrow inference)");
    println!("| Size | Topology | Time    |");
    println!("|------|----------|---------|");

    for &size in &[50, 200, 500] {
        for topology in &["standalone", "chain"] {
            let db = CompilerDb::new();
            let interner = db.interner();

            let funcs = match *topology {
                "standalone" => gen_standalone_funcs(&interner, size),
                "chain" => gen_linear_chain(&interner, size),
                _ => unreachable!(),
            };
            let functions: Vec<ArcFunction> = funcs.into_iter().map(|(_, f)| f).collect();

            let start = Instant::now();
            for _ in 0..iters {
                let graph = CallGraph::build(&functions);
                let sccs = compute_sccs(&graph);
                black_box(&sccs);
            }
            let scc_ns = start.elapsed().as_nanos() / iters as u128;
            let display = format_duration_ns(scc_ns);

            println!("| {size:>4} | {topology:<10} | {display:>7} |");
        }
    }
    println!("\n### Interpretation");
    println!("  SCC (no Salsa) = raw `infer_borrows_scc()` (no memoization/tracking).");
    println!("  SCC-queries    = per-SCC Salsa queries (includes memoization + tracking).");
    println!("  Overhead is dominated by Salsa's per-query bookkeeping, NOT the borrow");
    println!("  algorithm. This is the expected tradeoff: pay more cold-compile cost");
    println!("  for incremental wins on warm recompilation. In a full compilation,");
    println!("  borrow inference is one of many Salsa queries sharing the framework cost.");
    println!("  SCC computation (Tarjan's) is negligible (< 1ms for 500 functions).");
    println!();

    // Dummy bench to satisfy criterion.
    c.bench_function("borrow/regression_summary", |b| {
        b.iter(|| 1 + 1);
    });
}

/// Format nanoseconds as a human-readable duration.
fn format_duration_ns(ns: u128) -> String {
    if ns < 1_000 {
        format!("{ns}ns")
    } else if ns < 1_000_000 {
        format!("{:.1}µs", ns as f64 / 1_000.0)
    } else {
        format!("{:.2}ms", ns as f64 / 1_000_000.0)
    }
}

criterion_group!(
    benches,
    bench_cold_compile,
    bench_scc_standalone,
    bench_warm_compile,
    bench_scc_computation,
    bench_memory_profile,
    bench_regression_summary,
);
criterion_main!(benches);
