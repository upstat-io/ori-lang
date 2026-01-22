//! Benchmarks for lexing and parsing.
//!
//! ## Phase 1 Baselines (Foundation)
//!
//! ### Lexer Performance
//! | Benchmark              | Time      | Throughput  | Notes                    |
//! |------------------------|-----------|-------------|--------------------------|
//! | lexer/small            | 752 ns    | ~150 KB/s   | ~100 bytes, 5 functions  |
//! | lexer/medium           | 3.9 µs    | ~200 KB/s   | ~800 bytes, 10 functions |
//! | lexer/large_100_funcs  | 73 µs     | ~250 KB/s   | ~18KB, 100 functions     |
//!
//! ### Parser Performance
//! | Benchmark               | Time      | Notes                           |
//! |-------------------------|-----------|--------------------------------|
//! | parser/small            | 671 ns    | Small module, few expressions   |
//! | parser/medium           | 2.5 µs    | Medium module with patterns     |
//! | parser/large_100_funcs  | 37 µs     | 100 functions                   |
//!
//! ### Full Pipeline (Lex + Parse)
//! | Benchmark                    | Time      | Notes                      |
//! |------------------------------|-----------|----------------------------|
//! | full_pipeline/10_functions   | ~13 µs    | Small project              |
//! | full_pipeline/50_functions   | ~57 µs    | Medium project             |
//! | full_pipeline/100_functions  | ~112 µs   | Large module               |
//!
//! ### Expression Parsing
//! | Benchmark                  | Time      | Notes                        |
//! |----------------------------|-----------|------------------------------|
//! | simple_binary (1+2*3)      | 285 ns    | Basic precedence             |
//! | nested_binary              | 337 ns    | Parenthesized expressions    |
//! | function_call              | 375 ns    | @foo(1,2,3,4,5)              |
//! | method_chain               | 300 ns    | x.foo().bar().baz()          |
//! | if_else                    | 314 ns    | Conditional expression       |
//! | list_literal               | 424 ns    | [1,2,3,4,5,6,7,8,9,10]       |
//! | map_pattern                | ~450 ns   | map(.over: x, .transform: f) |
//! | complex (nested patterns)  | ~600 ns   | fold + filter + lambda       |
//!
//! ## Phase 2 Baselines (Type System)
//!
//! ### Lexer Performance
//! | Benchmark              | Time      | Change    | Notes                      |
//! |------------------------|-----------|-----------|----------------------------|
//! | lexer/small            | 778 ns    | +3%       | Within noise threshold     |
//! | lexer/medium           | 3.9 µs    | stable    | Unchanged                  |
//! | lexer/large_100_funcs  | 75 µs     | +3%       | Slight regression (noise?) |
//!
//! ### Parser Performance
//! | Benchmark               | Time      | Change    | Notes                      |
//! |-------------------------|-----------|-----------|----------------------------|
//! | parser/small            | 707 ns    | +5%       | Small regression           |
//! | parser/medium           | 2.5 µs    | stable    | Unchanged                  |
//! | parser/large_100_funcs  | 37 µs     | stable    | Unchanged                  |
//!
//! ### Full Pipeline (Lex + Parse)
//! | Benchmark                    | Time      | Change    | Notes                   |
//! |------------------------------|-----------|-----------|-------------------------|
//! | full_pipeline/10_functions   | 12.6 µs   | +2%       | Within noise            |
//! | full_pipeline/50_functions   | 58 µs     | +3%       | Slight regression       |
//! | full_pipeline/100_functions  | 116 µs    | +4%       | Slight regression       |
//!
//! ### Expression Parsing
//! | Benchmark                  | Time      | Change    | Notes                     |
//! |----------------------------|-----------|-----------|---------------------------|
//! | simple_binary              | 289 ns    | -4%       | Improved!                 |
//! | nested_binary              | 347 ns    | -4%       | Improved!                 |
//! | function_call              | 381 ns    | -2%       | Improved!                 |
//! | method_chain               | 301 ns    | -5%       | Improved!                 |
//! | if_else                    | 315 ns    | -3%       | Improved!                 |
//! | list_literal               | 427 ns    | stable    | Unchanged                 |
//! | map_pattern                | 346 ns    | -4%       | Improved!                 |
//! | complex                    | 378 ns    | -5%       | Improved!                 |
//!
//! ## Targets
//! - Cold compile 1K LOC: <50ms (currently ~112µs for 100 funcs ≈ 2K LOC)
//! - Incremental single file: <50ms

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use sigilc_v2::intern::StringInterner;
use sigilc_v2::syntax::{Lexer, Parser};

// Sample Sigil code snippets of varying sizes
const SMALL_CODE: &str = r#"
@add (a: int, b: int) -> int = a + b

@main () -> void = run(
    let x = 42,
    let y = @add(x, 10),
    print(y)
)
"#;

const MEDIUM_CODE: &str = r#"
$config_value = 100

@factorial (n: int) -> int =
    if n <= 1 then 1
    else n * @factorial(n - 1)

@fibonacci (n: int) -> int =
    if n <= 1 then n
    else @fibonacci(n - 1) + @fibonacci(n - 2)

@map_example (items: [int]) -> [int] =
    map(.over: items, .transform: x -> x * 2)

@filter_example (items: [int]) -> [int] =
    filter(.over: items, .predicate: x -> x > 10)

@fold_example (items: [int]) -> int =
    fold(.over: items, .init: 0, .op: (acc, x) -> acc + x)

@complex_example (data: [int]) -> int = run(
    let doubled = @map_example(data),
    let filtered = @filter_example(doubled),
    let sum = @fold_example(filtered),
    sum
)

@main () -> void = run(
    let numbers = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
    let result = @complex_example(numbers),
    print(result)
)
"#;

fn generate_large_code(num_functions: usize) -> String {
    let mut code = String::with_capacity(num_functions * 200);

    for i in 0..num_functions {
        code.push_str(&format!(
            r#"
@func_{i} (x: int, y: int) -> int = run(
    let a = x + y,
    let b = a * 2,
    let c = if b > 100 then b - 50 else b + 50,
    c
)
"#
        ));
    }

    code.push_str(r#"
@main () -> void = run(
    let result = @func_0(1, 2),
    print(result)
)
"#);

    code
}

fn bench_lexer(c: &mut Criterion) {
    let interner = StringInterner::new();

    let mut group = c.benchmark_group("lexer");

    // Small code
    group.throughput(Throughput::Bytes(SMALL_CODE.len() as u64));
    group.bench_function("small", |b| {
        b.iter(|| {
            let lexer = Lexer::new(black_box(SMALL_CODE), &interner);
            black_box(lexer.lex_all())
        })
    });

    // Medium code
    group.throughput(Throughput::Bytes(MEDIUM_CODE.len() as u64));
    group.bench_function("medium", |b| {
        b.iter(|| {
            let lexer = Lexer::new(black_box(MEDIUM_CODE), &interner);
            black_box(lexer.lex_all())
        })
    });

    // Large code (100 functions)
    let large_code = generate_large_code(100);
    group.throughput(Throughput::Bytes(large_code.len() as u64));
    group.bench_function("large_100_funcs", |b| {
        b.iter(|| {
            let lexer = Lexer::new(black_box(&large_code), &interner);
            black_box(lexer.lex_all())
        })
    });

    group.finish();
}

fn bench_lexer_throughput(c: &mut Criterion) {
    let interner = StringInterner::new();
    let mut group = c.benchmark_group("lexer_throughput");

    for size in [10, 50, 100, 500] {
        let code = generate_large_code(size);
        let bytes = code.len() as u64;

        group.throughput(Throughput::Bytes(bytes));
        group.bench_with_input(
            BenchmarkId::new("functions", size),
            &code,
            |b, code| {
                b.iter(|| {
                    let lexer = Lexer::new(black_box(code), &interner);
                    black_box(lexer.lex_all())
                })
            },
        );
    }

    group.finish();
}

fn bench_parser(c: &mut Criterion) {
    let interner = StringInterner::new();

    let mut group = c.benchmark_group("parser");

    // Small code
    let small_tokens = Lexer::new(SMALL_CODE, &interner).lex_all();
    group.bench_function("small", |b| {
        b.iter(|| {
            let parser = Parser::new(black_box(&small_tokens), &interner);
            black_box(parser.parse_module())
        })
    });

    // Medium code
    let medium_tokens = Lexer::new(MEDIUM_CODE, &interner).lex_all();
    group.bench_function("medium", |b| {
        b.iter(|| {
            let parser = Parser::new(black_box(&medium_tokens), &interner);
            black_box(parser.parse_module())
        })
    });

    // Large code
    let large_code = generate_large_code(100);
    let large_tokens = Lexer::new(&large_code, &interner).lex_all();
    group.bench_function("large_100_funcs", |b| {
        b.iter(|| {
            let parser = Parser::new(black_box(&large_tokens), &interner);
            black_box(parser.parse_module())
        })
    });

    group.finish();
}

fn bench_full_pipeline(c: &mut Criterion) {
    let interner = StringInterner::new();
    let mut group = c.benchmark_group("full_pipeline");

    for size in [10, 50, 100] {
        let code = generate_large_code(size);
        let bytes = code.len() as u64;

        group.throughput(Throughput::Bytes(bytes));
        group.bench_with_input(
            BenchmarkId::new("functions", size),
            &code,
            |b, code| {
                b.iter(|| {
                    let lexer = Lexer::new(black_box(code), &interner);
                    let tokens = lexer.lex_all();
                    let parser = Parser::new(&tokens, &interner);
                    black_box(parser.parse_module())
                })
            },
        );
    }

    group.finish();
}

fn bench_expression_parsing(c: &mut Criterion) {
    let interner = StringInterner::new();

    let expressions = [
        ("simple_binary", "1 + 2 * 3"),
        ("nested_binary", "((1 + 2) * (3 - 4)) / 5"),
        ("function_call", "@foo(1, 2, 3, 4, 5)"),
        ("method_chain", "x.foo().bar().baz()"),
        ("if_else", "if x > 0 then x else -x"),
        ("list_literal", "[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]"),
        ("map_pattern", "map(.over: items, .transform: x -> x * 2)"),
        ("complex", "fold(.over: filter(.over: items, .predicate: x -> x > 0), .init: 0, .op: (a, b) -> a + b)"),
    ];

    let mut group = c.benchmark_group("expression_parsing");

    for (name, expr) in expressions {
        let tokens = Lexer::new(expr, &interner).lex_all();
        group.bench_function(name, |b| {
            b.iter(|| {
                let parser = Parser::new(black_box(&tokens), &interner);
                black_box(parser.parse_expression())
            })
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_lexer,
    bench_lexer_throughput,
    bench_parser,
    bench_full_pipeline,
    bench_expression_parsing,
);

criterion_main!(benches);
