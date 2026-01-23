//! Lexer benchmarks for Sigil V3.
//!
//! Measures tokenization performance across different input sizes and complexity.

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use sigilc_v3::{CompilerDb, SourceFile, Db};
use sigilc_v3::query::tokens;
use std::path::PathBuf;

/// Simple function: @add (a: int, b: int) -> int = a + b
const SIMPLE_FUNCTION: &str = "@add (a: int, b: int) -> int = a + b";

/// Function with arithmetic
const ARITHMETIC_FUNCTION: &str = r#"
@calculate (x: int, y: int, z: int) -> int =
    x * y + z - x / y
"#;

/// Multiple functions
fn generate_n_functions(n: usize) -> String {
    (0..n)
        .map(|i| format!("@func{} (x: int) -> int = x + {}", i, i))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Function with patterns
const PATTERN_FUNCTION: &str = r#"
@transform (items: [int]) -> [int] = map(
    .over: items,
    .transform: x -> x * 2,
)
"#;

fn bench_lexer_simple(c: &mut Criterion) {
    let db = CompilerDb::new();

    c.bench_function("lexer/simple_function", |b| {
        b.iter(|| {
            let file = SourceFile::new(&db, PathBuf::from("/bench.si"), SIMPLE_FUNCTION.to_string());
            black_box(tokens(&db, file))
        })
    });
}

fn bench_lexer_arithmetic(c: &mut Criterion) {
    let db = CompilerDb::new();

    c.bench_function("lexer/arithmetic_function", |b| {
        b.iter(|| {
            let file = SourceFile::new(&db, PathBuf::from("/bench.si"), ARITHMETIC_FUNCTION.to_string());
            black_box(tokens(&db, file))
        })
    });
}

fn bench_lexer_pattern(c: &mut Criterion) {
    let db = CompilerDb::new();

    c.bench_function("lexer/pattern_function", |b| {
        b.iter(|| {
            let file = SourceFile::new(&db, PathBuf::from("/bench.si"), PATTERN_FUNCTION.to_string());
            black_box(tokens(&db, file))
        })
    });
}

fn bench_lexer_scaling(c: &mut Criterion) {
    let db = CompilerDb::new();
    let mut group = c.benchmark_group("lexer/scaling");

    for size in [10, 50, 100, 500, 1000].iter() {
        let source = generate_n_functions(*size);
        group.bench_with_input(BenchmarkId::new("functions", size), &source, |b, src| {
            b.iter(|| {
                let file = SourceFile::new(&db, PathBuf::from("/bench.si"), src.clone());
                black_box(tokens(&db, file))
            })
        });
    }

    group.finish();
}

fn bench_lexer_incremental(c: &mut Criterion) {
    let db = CompilerDb::new();
    let source = generate_n_functions(100);

    // First pass to warm up cache
    let file = SourceFile::new(&db, PathBuf::from("/bench.si"), source.clone());
    let _ = tokens(&db, file);

    c.bench_function("lexer/incremental_cached", |b| {
        b.iter(|| {
            // Same file, should be cached by Salsa
            black_box(tokens(&db, file))
        })
    });
}

criterion_group!(
    benches,
    bench_lexer_simple,
    bench_lexer_arithmetic,
    bench_lexer_pattern,
    bench_lexer_scaling,
    bench_lexer_incremental,
);
criterion_main!(benches);
