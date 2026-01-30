#![allow(clippy::unwrap_used, clippy::expect_used)]
#![allow(clippy::manual_assert, clippy::uninlined_format_args)]
//! Formatter benchmarks for Ori.
//!
//! Measures formatting performance across different input sizes.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use ori_fmt::{format_incremental, format_module, format_module_with_comments, IncrementalResult};
use ori_ir::StringInterner;
use rayon::prelude::*;
use std::fs;
use std::path::Path;
use std::time::Duration;

/// Generate N simple functions for benchmarking.
fn generate_n_functions(n: usize) -> String {
    (0..n)
        .map(|i| format!("@f{i} (x: int) -> int = x * {i}"))
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Generate N types for benchmarking.
fn generate_n_types(n: usize) -> String {
    (0..n)
        .map(|i| format!("type T{i} = {{ id: int, name: str, value: int }}"))
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Generate N functions with run patterns.
fn generate_n_run_patterns(n: usize) -> String {
    (0..n)
        .map(|i| {
            format!(
                "@p{i} (value: int) -> int = run(\n    let step1 = value + {i},\n    let result = step1 * 2,\n    result,\n)"
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Parse and format source code, returning formatted output.
fn parse_and_format(source: &str) -> String {
    let interner = StringInterner::new();
    let tokens = ori_lexer::lex(source, &interner);
    let output = ori_parse::parse(&tokens, &interner);

    if output.has_errors() {
        panic!("Parse errors in benchmark input");
    }

    format_module(&output.module, &output.arena, &interner)
}

/// Parse and format with comments for full format comparison.
fn parse_and_format_full(source: &str) -> String {
    let interner = StringInterner::new();
    let lex_output = ori_lexer::lex_with_comments(source, &interner);
    let output = ori_parse::parse(&lex_output.tokens, &interner);

    if output.has_errors() {
        panic!("Parse errors in benchmark input");
    }

    format_module_with_comments(
        &output.module,
        &lex_output.comments,
        &output.arena,
        &interner,
    )
}

/// Parse and do incremental format for a given byte range.
fn parse_and_format_incremental(
    source: &str,
    change_start: usize,
    change_end: usize,
) -> IncrementalResult {
    let interner = StringInterner::new();
    let lex_output = ori_lexer::lex_with_comments(source, &interner);
    let output = ori_parse::parse(&lex_output.tokens, &interner);

    if output.has_errors() {
        panic!("Parse errors in benchmark input");
    }

    format_incremental(
        &output.module,
        &lex_output.comments,
        &output.arena,
        &interner,
        change_start,
        change_end,
    )
}

fn bench_format_simple_functions(c: &mut Criterion) {
    let mut group = c.benchmark_group("formatter/simple_functions");
    group.measurement_time(Duration::from_secs(5));

    for size in &[10, 50, 100, 500, 1000] {
        let source = generate_n_functions(*size);
        group.bench_with_input(BenchmarkId::new("count", size), &source, |b, src| {
            b.iter(|| black_box(parse_and_format(src)));
        });
    }

    group.finish();
}

fn bench_format_types(c: &mut Criterion) {
    let mut group = c.benchmark_group("formatter/types");
    group.measurement_time(Duration::from_secs(5));

    for size in &[10, 50, 100, 500, 1000] {
        let source = generate_n_types(*size);
        group.bench_with_input(BenchmarkId::new("count", size), &source, |b, src| {
            b.iter(|| black_box(parse_and_format(src)));
        });
    }

    group.finish();
}

fn bench_format_run_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("formatter/run_patterns");
    group.measurement_time(Duration::from_secs(5));

    for size in &[10, 50, 100, 500] {
        let source = generate_n_run_patterns(*size);
        group.bench_with_input(BenchmarkId::new("count", size), &source, |b, src| {
            b.iter(|| black_box(parse_and_format(src)));
        });
    }

    group.finish();
}

fn bench_format_large_file(c: &mut Criterion) {
    let mut group = c.benchmark_group("formatter/large_file");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(20);

    // Try to load the 10k line benchmark file
    let benchmark_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tests/fmt/edge-cases/real/benchmark_10k.ori");

    if let Ok(source) = fs::read_to_string(&benchmark_path) {
        let lines = source.lines().count();
        group.bench_with_input(BenchmarkId::new("lines", lines), &source, |b, src| {
            b.iter(|| black_box(parse_and_format(src)));
        });
    }

    // Also test the 5k line file
    let large_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tests/fmt/edge-cases/real/large_file.ori");

    if let Ok(source) = fs::read_to_string(&large_path) {
        let lines = source.lines().count();
        group.bench_with_input(BenchmarkId::new("lines", lines), &source, |b, src| {
            b.iter(|| black_box(parse_and_format(src)));
        });
    }

    group.finish();
}

fn bench_format_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("formatter/scaling");
    group.measurement_time(Duration::from_secs(5));

    // Test scaling with mixed content
    for size in &[100, 500, 1000, 2000, 5000] {
        let functions = generate_n_functions(size / 2);
        let types = generate_n_types(size / 2);
        let source = format!("{}\n\n{}", types, functions);
        let lines = source.lines().count();

        group.bench_with_input(BenchmarkId::new("mixed_lines", lines), &source, |b, src| {
            b.iter(|| black_box(parse_and_format(src)));
        });
    }

    group.finish();
}

fn bench_format_many_files(c: &mut Criterion) {
    let mut group = c.benchmark_group("formatter/many_files");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(20);

    // Generate many small files in memory
    let files: Vec<String> = (0..1000)
        .map(|i| {
            format!(
                "type T{i} = {{ id: int, value: int }}\n\n@f{i} (x: int) -> int = x * {i}\n\n@t{i} tests @f{i} () -> void = assert_eq(actual: f{i}(x: 1), expected: {i})\n"
            )
        })
        .collect();

    // Sequential benchmark
    group.bench_function("sequential/1000", |b| {
        b.iter(|| {
            for src in &files {
                black_box(parse_and_format(src));
            }
        });
    });

    // Parallel benchmark using rayon
    group.bench_function("parallel/1000", |b| {
        b.iter(|| {
            files.par_iter().for_each(|src| {
                black_box(parse_and_format(src));
            });
        });
    });

    group.finish();
}

fn bench_format_parallel_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("formatter/parallel_scaling");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(20);

    for size in &[100, 500, 1000] {
        let files: Vec<String> = (0..*size)
            .map(|i| format!("@f{i} (x: int) -> int = x * {i}\n"))
            .collect();

        group.bench_with_input(BenchmarkId::new("parallel", size), &files, |b, files| {
            b.iter(|| {
                files.par_iter().for_each(|src| {
                    black_box(parse_and_format(src));
                });
            });
        });
    }

    group.finish();
}

fn bench_incremental_vs_full(c: &mut Criterion) {
    let mut group = c.benchmark_group("formatter/incremental");
    group.measurement_time(Duration::from_secs(5));

    // Test incremental formatting speedup for different file sizes
    for size in &[10, 50, 100, 500, 1000] {
        let source = generate_n_functions(*size);
        let source_len = source.len();

        // Full format benchmark
        group.bench_with_input(BenchmarkId::new("full", size), &source, |b, src| {
            b.iter(|| black_box(parse_and_format_full(src)));
        });

        // Incremental format benchmark (change in first function only)
        // First function is approximately bytes 0-30
        group.bench_with_input(
            BenchmarkId::new("incremental_first", size),
            &source,
            |b, src| {
                b.iter(|| black_box(parse_and_format_incremental(src, 0, 30)));
            },
        );

        // Incremental format benchmark (change in middle)
        let mid = source_len / 2;
        group.bench_with_input(
            BenchmarkId::new("incremental_middle", size),
            &source,
            |b, src| {
                b.iter(|| black_box(parse_and_format_incremental(src, mid, mid + 30)));
            },
        );
    }

    group.finish();
}

fn bench_incremental_large_file(c: &mut Criterion) {
    let mut group = c.benchmark_group("formatter/incremental_large");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(20);

    // Generate a large file (2000 functions)
    let source = generate_n_functions(2000);
    let source_len = source.len();

    // Full format
    group.bench_function("full/2000_funcs", |b| {
        b.iter(|| black_box(parse_and_format_full(&source)));
    });

    // Incremental format single function
    group.bench_function("incremental/2000_funcs_single", |b| {
        b.iter(|| black_box(parse_and_format_incremental(&source, 0, 30)));
    });

    // Incremental format middle function
    let mid = source_len / 2;
    group.bench_function("incremental/2000_funcs_middle", |b| {
        b.iter(|| black_box(parse_and_format_incremental(&source, mid, mid + 30)));
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_format_simple_functions,
    bench_format_types,
    bench_format_run_patterns,
    bench_format_large_file,
    bench_format_scaling,
    bench_format_many_files,
    bench_format_parallel_scaling,
    bench_incremental_vs_full,
    bench_incremental_large_file,
);
criterion_main!(benches);
