//! Lexer benchmarks for Ori.
//!
//! Measures tokenization performance across different input sizes and complexity.
//!
//! Two benchmark categories:
//! - `lexer/*` — Through Salsa query system (measures real-world usage)
//! - `lexer/raw/*` — Direct lexer calls (for comparison with other compilers)

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use ori_ir::StringInterner;
use oric::query::tokens;
use oric::{CompilerDb, SourceFile};
use std::path::PathBuf;

/// Simple function: @add (a: int, b: int) -> int = a + b
const SIMPLE_FUNCTION: &str = "@add (a: int, b: int) -> int = a + b";

/// Function with arithmetic
const ARITHMETIC_FUNCTION: &str = r"
@calculate (x: int, y: int, z: int) -> int =
    x * y + z - x / y
";

/// Multiple functions
fn generate_n_functions(n: usize) -> String {
    (0..n)
        .map(|i| format!("@func{i} (x: int) -> int = x + {i}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Function with patterns
const PATTERN_FUNCTION: &str = r"
@transform (items: [int]) -> [int] = map(
    over: items,
    transform: x -> x * 2,
)
";

fn bench_lexer_simple(c: &mut Criterion) {
    let db = CompilerDb::new();

    c.bench_function("lexer/simple_function", |b| {
        b.iter(|| {
            let file = SourceFile::new(
                &db,
                PathBuf::from("/bench.ori"),
                SIMPLE_FUNCTION.to_string(),
            );
            black_box(tokens(&db, file));
        });
    });
}

fn bench_lexer_arithmetic(c: &mut Criterion) {
    let db = CompilerDb::new();

    c.bench_function("lexer/arithmetic_function", |b| {
        b.iter(|| {
            let file = SourceFile::new(
                &db,
                PathBuf::from("/bench.ori"),
                ARITHMETIC_FUNCTION.to_string(),
            );
            black_box(tokens(&db, file));
        });
    });
}

fn bench_lexer_pattern(c: &mut Criterion) {
    let db = CompilerDb::new();

    c.bench_function("lexer/pattern_function", |b| {
        b.iter(|| {
            let file = SourceFile::new(
                &db,
                PathBuf::from("/bench.ori"),
                PATTERN_FUNCTION.to_string(),
            );
            black_box(tokens(&db, file));
        });
    });
}

fn bench_lexer_scaling(c: &mut Criterion) {
    let db = CompilerDb::new();
    let mut group = c.benchmark_group("lexer/scaling");

    for size in &[10, 50, 100, 500, 1000] {
        let source = generate_n_functions(*size);
        group.bench_with_input(BenchmarkId::new("functions", size), &source, |b, src| {
            b.iter(|| {
                let file = SourceFile::new(&db, PathBuf::from("/bench.ori"), src.clone());
                black_box(tokens(&db, file));
            });
        });
    }

    group.finish();
}

fn bench_lexer_incremental(c: &mut Criterion) {
    let db = CompilerDb::new();
    let source = generate_n_functions(100);

    // First pass to warm up cache
    let file = SourceFile::new(&db, PathBuf::from("/bench.ori"), source.clone());
    let _ = tokens(&db, file);

    c.bench_function("lexer/incremental_cached", |b| {
        b.iter(|| {
            // Same file, should be cached by Salsa
            black_box(tokens(&db, file));
        });
    });
}

/// Raw lexer benchmarks — bypasses Salsa for fair comparison with other compilers.
///
/// These benchmarks call `ori_lexer::lex()` directly, measuring pure tokenization
/// throughput without query system overhead. Use these numbers when comparing to
/// Zig (~1 GB/s), Go (~300 MB/s), Rust (~100 MB/s), etc.
mod raw_benches {
    use super::{
        black_box, generate_n_functions, BenchmarkId, Criterion, StringInterner, Throughput,
    };

    /// Benchmark raw lexer throughput at various scales.
    pub fn bench_raw_throughput(c: &mut Criterion) {
        let interner = StringInterner::new();
        let mut group = c.benchmark_group("lexer/raw");

        for num_functions in [10, 50, 100, 500, 1000, 5000] {
            let source = generate_n_functions(num_functions);
            let bytes = source.len() as u64;

            group.throughput(Throughput::Bytes(bytes));
            group.bench_with_input(
                BenchmarkId::new("throughput", num_functions),
                &source,
                |b, src| {
                    b.iter(|| {
                        black_box(ori_lexer::lex(src, &interner));
                    });
                },
            );
        }

        group.finish();
    }

    /// Benchmark with realistic file sizes (simulating real codebases).
    pub fn bench_raw_realistic(c: &mut Criterion) {
        let interner = StringInterner::new();
        let mut group = c.benchmark_group("lexer/raw/realistic");

        // Small file (~1KB) - typical utility module
        let small = generate_n_functions(30);
        group.throughput(Throughput::Bytes(small.len() as u64));
        group.bench_function("small_1kb", |b| {
            b.iter(|| black_box(ori_lexer::lex(&small, &interner)));
        });

        // Medium file (~10KB) - typical application module
        let medium = generate_n_functions(300);
        group.throughput(Throughput::Bytes(medium.len() as u64));
        group.bench_function("medium_10kb", |b| {
            b.iter(|| black_box(ori_lexer::lex(&medium, &interner)));
        });

        // Large file (~50KB) - large module or generated code
        let large = generate_n_functions(1500);
        group.throughput(Throughput::Bytes(large.len() as u64));
        group.bench_function("large_50kb", |b| {
            b.iter(|| black_box(ori_lexer::lex(&large, &interner)));
        });

        group.finish();
    }

    /// Token throughput (tokens/second instead of bytes/second).
    pub fn bench_raw_tokens(c: &mut Criterion) {
        let interner = StringInterner::new();
        let mut group = c.benchmark_group("lexer/raw/tokens");

        let source = generate_n_functions(500);
        // Pre-lex to count tokens
        let token_count = ori_lexer::lex(&source, &interner).len() as u64;

        group.throughput(Throughput::Elements(token_count));
        group.bench_function("500_functions", |b| {
            b.iter(|| black_box(ori_lexer::lex(&source, &interner)));
        });

        group.finish();
    }
}

fn bench_raw_throughput(c: &mut Criterion) {
    raw_benches::bench_raw_throughput(c);
}

fn bench_raw_realistic(c: &mut Criterion) {
    raw_benches::bench_raw_realistic(c);
}

fn bench_raw_tokens(c: &mut Criterion) {
    raw_benches::bench_raw_tokens(c);
}

criterion_group!(
    benches,
    // Salsa query benchmarks (real-world usage)
    bench_lexer_simple,
    bench_lexer_arithmetic,
    bench_lexer_pattern,
    bench_lexer_scaling,
    bench_lexer_incremental,
    // Raw benchmarks (for comparison with other compilers)
    bench_raw_throughput,
    bench_raw_realistic,
    bench_raw_tokens,
);
criterion_main!(benches);
