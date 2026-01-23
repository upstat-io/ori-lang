//! Type checker benchmarks for Sigil.
//!
//! Measures type inference and checking performance.

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use sigilc::{CompilerDb, SourceFile, Db};
use sigilc::query::typed;
use std::path::PathBuf;

/// Simple function with type annotations
const TYPED_FUNCTION: &str = "@add (a: int, b: int) -> int = a + b";

/// Function without type annotations (inference needed)
const INFERRED_FUNCTION: &str = "@add (a, b) = a + b";

/// Function with mixed annotations
const MIXED_FUNCTION: &str = "@process (x: int, y) -> int = x + y";

/// Multiple typed functions
fn generate_typed_functions(n: usize) -> String {
    (0..n)
        .map(|i| format!("@func{} (x: int) -> int = x + {}", i, i))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Multiple inferred functions
fn generate_inferred_functions(n: usize) -> String {
    (0..n)
        .map(|i| format!("@func{} (x) = x + {}", i, i))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Complex expression requiring inference
const COMPLEX_INFERENCE: &str = r#"
@complex (a: int, b: int) -> int =
    if a > b then a * 2 else b * 3
"#;

/// List operations
const LIST_OPERATIONS: &str = r#"
@make_list () = [1, 2, 3, 4, 5]
"#;

/// Nested let bindings
fn generate_let_chain(n: usize) -> String {
    let lets: Vec<String> = (0..n)
        .map(|i| format!("let x{}: int = {}", i, i))
        .collect();
    let final_expr = format!("x{}", n - 1);
    format!(
        "@chain () -> int = run({})",
        lets.into_iter()
            .chain(std::iter::once(final_expr))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn bench_typeck_annotated(c: &mut Criterion) {
    let db = CompilerDb::new();

    c.bench_function("typeck/annotated_function", |b| {
        b.iter(|| {
            let file = SourceFile::new(&db, PathBuf::from("/bench.si"), TYPED_FUNCTION.to_string());
            black_box(typed(&db, file))
        })
    });
}

fn bench_typeck_inferred(c: &mut Criterion) {
    let db = CompilerDb::new();

    c.bench_function("typeck/inferred_function", |b| {
        b.iter(|| {
            let file = SourceFile::new(&db, PathBuf::from("/bench.si"), INFERRED_FUNCTION.to_string());
            black_box(typed(&db, file))
        })
    });
}

fn bench_typeck_mixed(c: &mut Criterion) {
    let db = CompilerDb::new();

    c.bench_function("typeck/mixed_annotations", |b| {
        b.iter(|| {
            let file = SourceFile::new(&db, PathBuf::from("/bench.si"), MIXED_FUNCTION.to_string());
            black_box(typed(&db, file))
        })
    });
}

fn bench_typeck_complex(c: &mut Criterion) {
    let db = CompilerDb::new();

    c.bench_function("typeck/complex_inference", |b| {
        b.iter(|| {
            let file = SourceFile::new(&db, PathBuf::from("/bench.si"), COMPLEX_INFERENCE.to_string());
            black_box(typed(&db, file))
        })
    });
}

fn bench_typeck_list(c: &mut Criterion) {
    let db = CompilerDb::new();

    c.bench_function("typeck/list_operations", |b| {
        b.iter(|| {
            let file = SourceFile::new(&db, PathBuf::from("/bench.si"), LIST_OPERATIONS.to_string());
            black_box(typed(&db, file))
        })
    });
}

fn bench_typeck_scaling_annotated(c: &mut Criterion) {
    let db = CompilerDb::new();
    let mut group = c.benchmark_group("typeck/scaling_annotated");

    for size in [10, 50, 100, 500].iter() {
        let source = generate_typed_functions(*size);
        group.bench_with_input(BenchmarkId::new("functions", size), &source, |b, src| {
            b.iter(|| {
                let file = SourceFile::new(&db, PathBuf::from("/bench.si"), src.clone());
                black_box(typed(&db, file))
            })
        });
    }

    group.finish();
}

fn bench_typeck_scaling_inferred(c: &mut Criterion) {
    let db = CompilerDb::new();
    let mut group = c.benchmark_group("typeck/scaling_inferred");

    for size in [10, 50, 100, 500].iter() {
        let source = generate_inferred_functions(*size);
        group.bench_with_input(BenchmarkId::new("functions", size), &source, |b, src| {
            b.iter(|| {
                let file = SourceFile::new(&db, PathBuf::from("/bench.si"), src.clone());
                black_box(typed(&db, file))
            })
        });
    }

    group.finish();
}

fn bench_typeck_incremental(c: &mut Criterion) {
    let db = CompilerDb::new();
    let source = generate_typed_functions(100);

    // First pass to warm up cache
    let file = SourceFile::new(&db, PathBuf::from("/bench.si"), source.clone());
    let _ = typed(&db, file);

    c.bench_function("typeck/incremental_cached", |b| {
        b.iter(|| {
            // Same file, should be cached by Salsa
            black_box(typed(&db, file))
        })
    });
}

fn bench_typeck_annotation_impact(c: &mut Criterion) {
    let db = CompilerDb::new();
    let mut group = c.benchmark_group("typeck/annotation_impact");

    // Compare annotated vs inferred for same number of functions
    let size = 100;

    let annotated = generate_typed_functions(size);
    let inferred = generate_inferred_functions(size);

    group.bench_function("with_annotations", |b| {
        b.iter(|| {
            let file = SourceFile::new(&db, PathBuf::from("/bench.si"), annotated.clone());
            black_box(typed(&db, file))
        })
    });

    group.bench_function("with_inference", |b| {
        b.iter(|| {
            let file = SourceFile::new(&db, PathBuf::from("/bench.si"), inferred.clone());
            black_box(typed(&db, file))
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_typeck_annotated,
    bench_typeck_inferred,
    bench_typeck_mixed,
    bench_typeck_complex,
    bench_typeck_list,
    bench_typeck_scaling_annotated,
    bench_typeck_scaling_inferred,
    bench_typeck_incremental,
    bench_typeck_annotation_impact,
);
criterion_main!(benches);
