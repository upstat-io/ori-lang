//! Parser benchmarks for Ori.
//!
//! Measures parsing performance for various AST structures.

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use oric::{CompilerDb, SourceFile};
use oric::query::parsed;
use std::path::PathBuf;

/// Simple function
const SIMPLE_FUNCTION: &str = "@add (a: int, b: int) -> int = a + b";

/// Nested arithmetic expression
const NESTED_ARITHMETIC: &str = r#"
@complex (a: int, b: int, c: int, d: int) -> int =
    ((a + b) * (c - d)) / ((a - b) + (c * d))
"#;

/// Conditional expression
const CONDITIONAL: &str = r#"
@max (a: int, b: int) -> int =
    if a > b then a else b
"#;

/// Function with list literal
const LIST_LITERAL: &str = r#"
@make_list () = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
"#;

/// Multiple functions
fn generate_n_functions(n: usize) -> String {
    (0..n)
        .map(|i| format!("@func{} (x: int) -> int = x + {}", i, i))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Deeply nested conditionals
fn generate_nested_conditionals(depth: usize) -> String {
    let mut expr = "x".to_string();
    for i in 0..depth {
        expr = format!("if x > {} then {} else {}", i, expr.clone(), i);
    }
    format!("@nested (x: int) -> int = {}", expr)
}

/// Pattern expressions
const PATTERN_MAP: &str = r#"
@transform (items: [int]) -> [int] = map(
    over: items,
    transform: x -> x * 2,
)
"#;

const PATTERN_FOLD: &str = r#"
@sum (items: [int]) -> int = fold(
    over: items,
    init: 0,
    op: (acc, x) -> acc + x,
)
"#;

fn bench_parser_simple(c: &mut Criterion) {
    let db = CompilerDb::new();

    c.bench_function("parser/simple_function", |b| {
        b.iter(|| {
            let file = SourceFile::new(&db, PathBuf::from("/bench.ori"), SIMPLE_FUNCTION.to_string());
            black_box(parsed(&db, file))
        })
    });
}

fn bench_parser_nested_arithmetic(c: &mut Criterion) {
    let db = CompilerDb::new();

    c.bench_function("parser/nested_arithmetic", |b| {
        b.iter(|| {
            let file = SourceFile::new(&db, PathBuf::from("/bench.ori"), NESTED_ARITHMETIC.to_string());
            black_box(parsed(&db, file))
        })
    });
}

fn bench_parser_conditional(c: &mut Criterion) {
    let db = CompilerDb::new();

    c.bench_function("parser/conditional", |b| {
        b.iter(|| {
            let file = SourceFile::new(&db, PathBuf::from("/bench.ori"), CONDITIONAL.to_string());
            black_box(parsed(&db, file))
        })
    });
}

fn bench_parser_list(c: &mut Criterion) {
    let db = CompilerDb::new();

    c.bench_function("parser/list_literal", |b| {
        b.iter(|| {
            let file = SourceFile::new(&db, PathBuf::from("/bench.ori"), LIST_LITERAL.to_string());
            black_box(parsed(&db, file))
        })
    });
}

fn bench_parser_patterns(c: &mut Criterion) {
    let db = CompilerDb::new();
    let mut group = c.benchmark_group("parser/patterns");

    group.bench_function("map", |b| {
        b.iter(|| {
            let file = SourceFile::new(&db, PathBuf::from("/bench.ori"), PATTERN_MAP.to_string());
            black_box(parsed(&db, file))
        })
    });

    group.bench_function("fold", |b| {
        b.iter(|| {
            let file = SourceFile::new(&db, PathBuf::from("/bench.ori"), PATTERN_FOLD.to_string());
            black_box(parsed(&db, file))
        })
    });

    group.finish();
}

fn bench_parser_scaling(c: &mut Criterion) {
    let db = CompilerDb::new();
    let mut group = c.benchmark_group("parser/scaling");

    for size in [10, 50, 100, 500].iter() {
        let source = generate_n_functions(*size);
        group.bench_with_input(BenchmarkId::new("functions", size), &source, |b, src| {
            b.iter(|| {
                let file = SourceFile::new(&db, PathBuf::from("/bench.ori"), src.clone());
                black_box(parsed(&db, file))
            })
        });
    }

    group.finish();
}

fn bench_parser_nesting(c: &mut Criterion) {
    let db = CompilerDb::new();
    let mut group = c.benchmark_group("parser/nesting");

    for depth in [5, 10, 20, 50].iter() {
        let source = generate_nested_conditionals(*depth);
        group.bench_with_input(BenchmarkId::new("conditionals", depth), &source, |b, src| {
            b.iter(|| {
                let file = SourceFile::new(&db, PathBuf::from("/bench.ori"), src.clone());
                black_box(parsed(&db, file))
            })
        });
    }

    group.finish();
}

fn bench_parser_incremental(c: &mut Criterion) {
    let db = CompilerDb::new();
    let source = generate_n_functions(100);

    // First pass to warm up cache
    let file = SourceFile::new(&db, PathBuf::from("/bench.ori"), source.clone());
    let _ = parsed(&db, file);

    c.bench_function("parser/incremental_cached", |b| {
        b.iter(|| {
            // Same file, should be cached by Salsa
            black_box(parsed(&db, file))
        })
    });
}

criterion_group!(
    benches,
    bench_parser_simple,
    bench_parser_nested_arithmetic,
    bench_parser_conditional,
    bench_parser_list,
    bench_parser_patterns,
    bench_parser_scaling,
    bench_parser_nesting,
    bench_parser_incremental,
);
criterion_main!(benches);
