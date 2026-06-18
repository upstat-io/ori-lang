//! Parser benchmarks for Ori.
//!
//! Measures parsing performance for various AST structures.
//!
//! Two benchmark categories:
//! - `parser/*` — Through Salsa query system (measures real-world usage)
//! - `parser/raw/*` — Direct parser calls (for comparison with other compilers)

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use ori_ir::StringInterner;
use oric::query::parsed;
use oric::{CompilerDb, SourceFile};
use salsa::Setter;
use std::path::PathBuf;

/// Simple function
const SIMPLE_FUNCTION: &str = "@add (a: int, b: int) -> int = a + b";

/// Nested arithmetic expression
const NESTED_ARITHMETIC: &str = r"
@complex (a: int, b: int, c: int, d: int) -> int =
    ((a + b) * (c - d)) / ((a - b) + (c * d))
";

/// Conditional expression
const CONDITIONAL: &str = r"
@max (a: int, b: int) -> int =
    if a > b then a else b
";

/// Function with list literal
const LIST_LITERAL: &str = r"
@make_list () = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
";

/// Multiple functions
fn generate_n_functions(n: usize) -> String {
    (0..n)
        .map(|i| format!("@func{i} (x: int) -> int = x + {i}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Deeply nested conditionals
fn generate_nested_conditionals(depth: usize) -> String {
    let mut expr = "x".to_string();
    for i in 0..depth {
        expr = format!("if x > {i} then {} else {i}", expr.clone());
    }
    format!("@nested (x: int) -> int = {expr}")
}

/// Deeply nested binary expressions — exercises the Pratt parser loop.
fn generate_expr_heavy(depth: usize) -> String {
    let mut expr = "x".to_string();
    for _ in 0..depth {
        expr = format!("({expr} + y * z - w)");
    }
    format!("@compute (x: int, y: int, z: int, w: int) -> int = {expr}")
}

/// Pattern expressions
const PATTERN_MAP: &str = r"
@transform (items: [int]) -> [int] = map(
    over: items,
    transform: x -> x * 2,
)
";

const PATTERN_FOLD: &str = r"
@sum (items: [int]) -> int = fold(
    over: items,
    init: 0,
    op: (acc, x) -> acc + x,
)
";

fn bench_parser_simple(c: &mut Criterion) {
    let db = CompilerDb::new();

    c.bench_function("parser/simple_function", |b| {
        b.iter(|| {
            let file = SourceFile::new(
                &db,
                PathBuf::from("/bench.ori"),
                SIMPLE_FUNCTION.to_string(),
            );
            black_box(parsed(&db, file));
        });
    });
}

fn bench_parser_nested_arithmetic(c: &mut Criterion) {
    let db = CompilerDb::new();

    c.bench_function("parser/nested_arithmetic", |b| {
        b.iter(|| {
            let file = SourceFile::new(
                &db,
                PathBuf::from("/bench.ori"),
                NESTED_ARITHMETIC.to_string(),
            );
            black_box(parsed(&db, file));
        });
    });
}

fn bench_parser_conditional(c: &mut Criterion) {
    let db = CompilerDb::new();

    c.bench_function("parser/conditional", |b| {
        b.iter(|| {
            let file = SourceFile::new(&db, PathBuf::from("/bench.ori"), CONDITIONAL.to_string());
            black_box(parsed(&db, file));
        });
    });
}

fn bench_parser_list(c: &mut Criterion) {
    let db = CompilerDb::new();

    c.bench_function("parser/list_literal", |b| {
        b.iter(|| {
            let file = SourceFile::new(&db, PathBuf::from("/bench.ori"), LIST_LITERAL.to_string());
            black_box(parsed(&db, file));
        });
    });
}

fn bench_parser_patterns(c: &mut Criterion) {
    let db = CompilerDb::new();
    let mut group = c.benchmark_group("parser/patterns");

    group.bench_function("map", |b| {
        b.iter(|| {
            let file = SourceFile::new(&db, PathBuf::from("/bench.ori"), PATTERN_MAP.to_string());
            black_box(parsed(&db, file));
        });
    });

    group.bench_function("fold", |b| {
        b.iter(|| {
            let file = SourceFile::new(&db, PathBuf::from("/bench.ori"), PATTERN_FOLD.to_string());
            black_box(parsed(&db, file));
        });
    });

    group.finish();
}

fn bench_parser_scaling(c: &mut Criterion) {
    let db = CompilerDb::new();
    let mut group = c.benchmark_group("parser/scaling");

    for size in &[10, 50, 100, 500] {
        let source = generate_n_functions(*size);
        group.bench_with_input(BenchmarkId::new("functions", size), &source, |b, src| {
            b.iter(|| {
                let file = SourceFile::new(&db, PathBuf::from("/bench.ori"), src.clone());
                black_box(parsed(&db, file));
            });
        });
    }

    group.finish();
}

fn bench_parser_nesting(c: &mut Criterion) {
    let db = CompilerDb::new();
    let mut group = c.benchmark_group("parser/nesting");

    for depth in &[5, 10, 20, 50] {
        let source = generate_nested_conditionals(*depth);
        group.bench_with_input(
            BenchmarkId::new("conditionals", depth),
            &source,
            |b, src| {
                b.iter(|| {
                    let file = SourceFile::new(&db, PathBuf::from("/bench.ori"), src.clone());
                    black_box(parsed(&db, file));
                });
            },
        );
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
            black_box(parsed(&db, file));
        });
    });
}

/// Benchmarks for the actual incremental parsing infrastructure (bypassing Salsa).
///
/// These benchmarks measure the performance of `parse_incremental()` which reuses
/// unchanged AST nodes when a small edit is made.
mod incremental_benches {
    use std::hint::black_box;

    use criterion::{BenchmarkId, Criterion};
    use ori_ir::incremental::TextChange;
    use ori_ir::StringInterner;
    use ori_parse::{parse, parse_incremental};

    /// Generate source with N functions for benchmarking.
    fn generate_source(n: usize) -> String {
        (0..n)
            .map(|i| format!("@func{i} (x: int) -> int = x + {i}"))
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    /// Benchmark: Full reparse vs incremental reparse for a small edit.
    pub fn bench_incremental_vs_full(c: &mut Criterion) {
        let interner = StringInterner::new();
        let mut group = c.benchmark_group("parser/incremental");

        for num_functions in [10, 50, 100, 200] {
            // Original source
            let source = generate_source(num_functions);
            let tokens = ori_lexer::lex(&source, &interner);
            let old_result = parse(&tokens, &interner);

            // Create a modified source (change "0" to "999" in first function)
            // "@func0 (x: int) -> int = x + 0" -> "@func0 (x: int) -> int = x + 999"
            let new_source = source.replacen("x + 0", "x + 999", 1);
            let new_tokens = ori_lexer::lex(&new_source, &interner);

            // Find the position of the change
            // "x + 0" starts at position 27 in "@func0 (x: int) -> int = x + 0"
            #[allow(
                clippy::cast_possible_truncation,
                reason = "benchmark source is <10KB, fits in u32"
            )]
            let change_start = source.find("x + 0").unwrap_or(27) as u32 + 4; // position of "0"
            let change = TextChange::new(change_start, change_start + 1, 3); // "0" (1 char) -> "999" (3 chars)

            // Benchmark full reparse
            group.bench_with_input(
                BenchmarkId::new("full_reparse", num_functions),
                &new_tokens,
                |b, tokens| {
                    b.iter(|| {
                        black_box(parse(tokens, &interner));
                    });
                },
            );

            // Benchmark incremental reparse
            group.bench_with_input(
                BenchmarkId::new("incremental", num_functions),
                &(&new_tokens, &old_result, change),
                |b, (tokens, old, change)| {
                    b.iter(|| {
                        black_box(parse_incremental(tokens, &interner, old, *change));
                    });
                },
            );
        }

        group.finish();
    }

    /// Benchmark: Measure reuse rate for different edit locations.
    pub fn bench_incremental_reuse_rate(c: &mut Criterion) {
        use ori_ir::incremental::ChangeMarker;
        use ori_parse::incremental::SyntaxCursor;

        let interner = StringInterner::new();

        // Create a file with 20 functions
        let source = generate_source(20);
        let tokens = ori_lexer::lex(&source, &interner);
        let result = parse(&tokens, &interner);

        // Find the positions of each function
        let func_positions: Vec<(usize, usize)> = source
            .match_indices("@func")
            .map(|(start, _)| {
                let end = source[start..]
                    .find("\n\n")
                    .map_or(source.len(), |e| start + e);
                (start, end)
            })
            .collect();

        let mut group = c.benchmark_group("parser/incremental_reuse");

        // Test edits at different positions
        for (edit_name, func_idx) in [("start", 0), ("middle", 10), ("end", 19)] {
            let (func_start, _) = func_positions[func_idx];
            #[allow(
                clippy::cast_possible_truncation,
                reason = "benchmark source is <10KB, fits in u32"
            )]
            let change_pos = func_start as u32 + 10; // Edit somewhere in the function

            let change = TextChange::new(change_pos, change_pos + 1, 2);
            let marker = ChangeMarker::from_change(&change, change_pos.saturating_sub(5));

            let cursor = SyntaxCursor::new(&result.module, &result.arena, marker.clone());
            let total_decls = cursor.total_declarations();

            // Count reusable declarations
            let mut cursor = SyntaxCursor::new(&result.module, &result.arena, marker);
            let mut reusable = 0;
            while let Some(decl) = cursor.find_at(0) {
                if !cursor.marker().intersects(decl.span) {
                    reusable += 1;
                }
                cursor.advance();
            }

            group.bench_function(BenchmarkId::new("edit_at", edit_name), |b| {
                b.iter(|| {
                    let marker = ChangeMarker::from_change(&change, change_pos.saturating_sub(5));
                    let mut cursor = SyntaxCursor::new(&result.module, &result.arena, marker);
                    let mut count = 0;
                    while cursor.find_at(0).is_some() {
                        count += 1;
                        cursor.advance();
                    }
                    black_box(count)
                });
            });

            #[allow(
                clippy::cast_precision_loss,
                reason = "acceptable for percentage display in diagnostic output"
            )]
            let reuse_pct = (f64::from(reusable) / total_decls as f64) * 100.0;
            eprintln!(
                "Edit at {edit_name} (func {func_idx}): {reusable}/{total_decls} reusable ({reuse_pct:.1}%)"
            );
        }

        group.finish();
    }
}

fn bench_incremental_vs_full(c: &mut Criterion) {
    incremental_benches::bench_incremental_vs_full(c);
}

fn bench_incremental_reuse_rate(c: &mut Criterion) {
    incremental_benches::bench_incremental_reuse_rate(c);
}

/// Raw parser benchmarks — bypasses Salsa for fair comparison with other compilers.
///
/// These benchmarks call `ori_lexer::lex()` and `ori_parse::parse()` directly,
/// measuring pure parsing throughput without query system overhead.
mod raw_benches {
    use super::{
        black_box, generate_n_functions, BenchmarkId, Criterion, StringInterner, Throughput,
    };

    /// Benchmark raw parser throughput (lexer + parser, no Salsa).
    pub fn bench_raw_throughput(c: &mut Criterion) {
        let interner = StringInterner::new();
        let mut group = c.benchmark_group("parser/raw");

        for num_functions in [10, 50, 100, 500, 1000] {
            let source = generate_n_functions(num_functions);
            let bytes = source.len() as u64;

            group.throughput(Throughput::Bytes(bytes));
            group.bench_with_input(
                BenchmarkId::new("throughput", num_functions),
                &source,
                |b, src| {
                    b.iter(|| {
                        let tokens = ori_lexer::lex(src, &interner);
                        black_box(ori_parse::parse(&tokens, &interner));
                    });
                },
            );
        }

        group.finish();
    }

    /// Benchmark parser-only throughput (tokens already lexed).
    pub fn bench_raw_parser_only(c: &mut Criterion) {
        let interner = StringInterner::new();
        let mut group = c.benchmark_group("parser/raw/parser_only");

        for num_functions in [100, 500, 1000] {
            let source = generate_n_functions(num_functions);
            let tokens = ori_lexer::lex(&source, &interner);
            let bytes = source.len() as u64;

            group.throughput(Throughput::Bytes(bytes));
            group.bench_with_input(
                BenchmarkId::new("throughput", num_functions),
                &tokens,
                |b, toks| {
                    b.iter(|| {
                        black_box(ori_parse::parse(toks, &interner));
                    });
                },
            );
        }

        group.finish();
    }

    /// Benchmark with realistic file sizes.
    pub fn bench_raw_realistic(c: &mut Criterion) {
        let interner = StringInterner::new();
        let mut group = c.benchmark_group("parser/raw/realistic");

        // Small file (~1KB)
        let small = generate_n_functions(30);
        group.throughput(Throughput::Bytes(small.len() as u64));
        group.bench_function("small_1kb", |b| {
            b.iter(|| {
                let tokens = ori_lexer::lex(&small, &interner);
                black_box(ori_parse::parse(&tokens, &interner));
            });
        });

        // Medium file (~10KB)
        let medium = generate_n_functions(300);
        group.throughput(Throughput::Bytes(medium.len() as u64));
        group.bench_function("medium_10kb", |b| {
            b.iter(|| {
                let tokens = ori_lexer::lex(&medium, &interner);
                black_box(ori_parse::parse(&tokens, &interner));
            });
        });

        // Large file (~50KB)
        let large = generate_n_functions(1500);
        group.throughput(Throughput::Bytes(large.len() as u64));
        group.bench_function("large_50kb", |b| {
            b.iter(|| {
                let tokens = ori_lexer::lex(&large, &interner);
                black_box(ori_parse::parse(&tokens, &interner));
            });
        });

        group.finish();
    }

    /// AST node throughput (nodes/second).
    pub fn bench_raw_ast_nodes(c: &mut Criterion) {
        let interner = StringInterner::new();
        let mut group = c.benchmark_group("parser/raw/ast_nodes");

        let source = generate_n_functions(500);
        let tokens = ori_lexer::lex(&source, &interner);

        // Each function generates: 1 decl + 1 signature + ~3 exprs = ~5 nodes
        // 500 functions ≈ 2500 nodes (rough estimate)
        let estimated_nodes = 500u64 * 5;

        group.throughput(Throughput::Elements(estimated_nodes));
        group.bench_function("500_functions", |b| {
            b.iter(|| {
                black_box(ori_parse::parse(&tokens, &interner));
            });
        });

        group.finish();
    }
}

fn bench_raw_throughput(c: &mut Criterion) {
    raw_benches::bench_raw_throughput(c);
}

fn bench_raw_parser_only(c: &mut Criterion) {
    raw_benches::bench_raw_parser_only(c);
}

fn bench_raw_realistic(c: &mut Criterion) {
    raw_benches::bench_raw_realistic(c);
}

fn bench_raw_ast_nodes(c: &mut Criterion) {
    raw_benches::bench_raw_ast_nodes(c);
}

/// Expression-heavy parsing — deeply nested binary ops stress the Pratt parser loop.
fn bench_expr_heavy(c: &mut Criterion) {
    let interner = StringInterner::new();
    let mut group = c.benchmark_group("parser/raw/expr_heavy");

    for depth in [10, 50, 100, 500] {
        let source = generate_expr_heavy(depth);
        let bytes = source.len() as u64;
        group.throughput(Throughput::Bytes(bytes));
        group.bench_with_input(BenchmarkId::new("depth", depth), &source, |b, src| {
            b.iter(|| {
                let tokens = ori_lexer::lex(src, &interner);
                black_box(ori_parse::parse(&tokens, &interner));
            });
        });
    }

    group.finish();
}

/// Isolate Salsa query-system overhead: identical workload, raw path vs via Salsa.
fn bench_salsa_overhead(c: &mut Criterion) {
    let interner = StringInterner::new();
    let db = CompilerDb::new();
    let source = generate_n_functions(500);
    let bytes = source.len() as u64;
    let mut group = c.benchmark_group("parser/salsa_overhead");
    group.throughput(Throughput::Bytes(bytes));

    group.bench_function("raw", |b| {
        b.iter(|| {
            let tokens = ori_lexer::lex(&source, &interner);
            black_box(ori_parse::parse(&tokens, &interner));
        });
    });

    group.bench_function("via_salsa", |b| {
        b.iter(|| {
            let file = SourceFile::new(&db, PathBuf::from("/bench.ori"), source.clone());
            black_box(parsed(&db, file));
        });
    });

    group.finish();
}

/// A second 500-function workload of equivalent parse cost to `generate_n_functions`,
/// distinct in text so `set_text` registers a real change and forces a recompute.
fn generate_n_functions_alt(n: usize) -> String {
    (0..n)
        .map(|i| format!("@gunc{i} (y: int) -> int = y + {i}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Corrected Salsa-overhead isolation per the baselines.json `salsa_overhead` caveat:
/// the original `via_salsa` constructs a fresh `SourceFile` + query per iteration, so
/// its timing includes Salsa INPUT-SETUP, NOT pure query-recompute. This splits the two.
///
/// - `salsa_input_setup`: per-iter construct `SourceFile` (on a fresh-per-iter `CompilerDb`)
///   then run `parsed` once — captures input-construction + first-eval (matches the original
///   `via_salsa` shape, but the db is fresh each iter so no Salsa caching masks setup).
/// - `salsa_recompute`: construct `CompilerDb` + `SourceFile` ONCE outside the timing loop,
///   warm the query, then per-iter `set_text` to an alternate equivalent-cost 500-fn source
///   (the documented invalidation path) and re-query `parsed` — pure query-recompute with
///   NO input/db reconstruction.
/// - `raw`: no-Salsa `lex` + `parse` reference denominator.
fn bench_salsa_overhead_split(c: &mut Criterion) {
    let interner = StringInterner::new();
    let source = generate_n_functions(500);
    let source_alt = generate_n_functions_alt(500);
    let bytes = source.len() as u64;
    let mut group = c.benchmark_group("parser/salsa_overhead_split");
    group.throughput(Throughput::Bytes(bytes));

    // Reference denominator: raw lex + parse, no Salsa.
    group.bench_function("raw", |b| {
        b.iter(|| {
            let tokens = ori_lexer::lex(&source, &interner);
            black_box(ori_parse::parse(&tokens, &interner));
        });
    });

    // Input-setup + first-eval: fresh db + SourceFile per iteration (no caching masks setup).
    group.bench_function("salsa_input_setup", |b| {
        b.iter(|| {
            let db = CompilerDb::new();
            let file = SourceFile::new(&db, PathBuf::from("/bench.ori"), source.clone());
            black_box(parsed(&db, file));
        });
    });

    // Pure recompute: db + file built ONCE; per-iter set_text forces invalidation + re-query.
    group.bench_function("salsa_recompute", |b| {
        let mut db = CompilerDb::new();
        let file = SourceFile::new(&db, PathBuf::from("/bench.ori"), source.clone());
        let _ = parsed(&db, file); // warm
        let mut toggle = false;
        b.iter(|| {
            // Alternate the input text between two equivalent-cost 500-fn sources so each
            // iteration registers a genuine change and forces a recompute of equal work.
            let next = if toggle {
                source.clone()
            } else {
                source_alt.clone()
            };
            toggle = !toggle;
            file.set_text(&mut db).to(next);
            black_box(parsed(&db, file));
        });
    });

    group.finish();
}

/// A realistic multi-file corpus: distinct modules of varying size, mirroring a
/// real codebase rather than one giant file.
fn generate_macro_corpus(n_files: usize) -> Vec<String> {
    (0..n_files)
        .map(|f| {
            // Vary module size so the corpus is not uniform (30..130 functions).
            let fns = 30 + (f * 13) % 100;
            (0..fns)
                .map(|i| format!("@mod{f}_fn{i} (a: int, b: int) -> int = (a + {i}) * b - {f}"))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .collect()
}

/// Macro baseline: end-to-end frontend throughput (lex + parse) across a realistic
/// multi-file corpus. The clean frontend anchor §06 re-measures vs parser-perf-v0
/// to prove the micro-tuning wins compound (raw path — no Salsa input-setup confound
/// per the baselines.json `salsa_overhead` caveat).
fn bench_macro_frontend(c: &mut Criterion) {
    let interner = StringInterner::new();
    let mut group = c.benchmark_group("parser/macro/frontend");

    for n_files in [10, 50, 100] {
        let corpus = generate_macro_corpus(n_files);
        let total_bytes: u64 = corpus.iter().map(|s| s.len() as u64).sum();
        group.throughput(Throughput::Bytes(total_bytes));
        group.bench_with_input(BenchmarkId::new("files", n_files), &corpus, |b, files| {
            b.iter(|| {
                for src in files {
                    let tokens = ori_lexer::lex(src, &interner);
                    black_box(ori_parse::parse(&tokens, &interner));
                }
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    // End-to-end frontend macro anchor (lex+parse over a multi-file corpus)
    bench_macro_frontend,
    // Pratt-loop stress + Salsa overhead isolation
    bench_expr_heavy,
    bench_salsa_overhead,
    bench_salsa_overhead_split,
    // Salsa query benchmarks (real-world usage)
    bench_parser_simple,
    bench_parser_nested_arithmetic,
    bench_parser_conditional,
    bench_parser_list,
    bench_parser_patterns,
    bench_parser_scaling,
    bench_parser_nesting,
    bench_parser_incremental,
    bench_incremental_vs_full,
    bench_incremental_reuse_rate,
    // Raw benchmarks (for comparison with other compilers)
    bench_raw_throughput,
    bench_raw_parser_only,
    bench_raw_realistic,
    bench_raw_ast_nodes,
);
criterion_main!(benches);
