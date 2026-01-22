//! End-to-end benchmarks for full compilation pipeline.
//!
//! ## Phase 3.5 Baselines (Full Pipeline)
//!
//! | Benchmark                    | Time      | LOC   | Notes                          |
//! |------------------------------|-----------|-------|--------------------------------|
//! | e2e/10_functions             | TBD       | ~200  | Small project                  |
//! | e2e/50_functions             | TBD       | ~1000 | Medium project                 |
//! | e2e/100_functions            | TBD       | ~2000 | Large module                   |
//! | e2e/500_functions            | TBD       | ~10K  | Very large module              |
//!
//! ## Targets (from design docs)
//! - Cold compile 1K LOC: <50ms
//! - Cold compile 10K LOC: <300ms
//! - Memory 10K LOC: <50MB

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput, SamplingMode};
use std::rc::Rc;
use std::cell::RefCell;
use std::collections::HashMap;

use sigilc_v2::intern::StringInterner;
use sigilc_v2::syntax::{Lexer, Parser, ItemKind};
use sigilc_v2::eval::{Evaluator, Value, FunctionValue, Environment};

/// Generate a Sigil module with N functions.
fn generate_module(num_functions: usize) -> String {
    let mut code = String::with_capacity(num_functions * 200);

    // Generate helper functions
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

    // Generate main that calls some functions
    code.push_str(&format!(
        r#"
@main () -> int = run(
    let result = func_0(1, 2),
    let result2 = func_1(result, 3),
    result2
)
"#
    ));

    code
}

/// Generate a module with realistic patterns (map, filter, fold).
fn generate_pattern_module(num_functions: usize) -> String {
    let mut code = String::with_capacity(num_functions * 300);

    for i in 0..num_functions {
        code.push_str(&format!(
            r#"
@process_{i} (items: [int]) -> int = run(
    let doubled = map(.over: items, .transform: x -> x * 2),
    let filtered = filter(.over: doubled, .predicate: x -> x > 10),
    let sum = fold(.over: filtered, .init: 0, .op: (acc, x) -> acc + x),
    sum
)
"#
        ));
    }

    code.push_str(
        r#"
@main () -> int = run(
    let data = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
    let result = process_0(data),
    result
)
"#
    );

    code
}

/// Register all functions from parsed items into the environment.
fn register_functions(
    env: &mut Environment,
    items: &[sigilc_v2::syntax::Item],
    arena: &sigilc_v2::syntax::ExprArena,
) {
    for item in items {
        if let ItemKind::Function(func) = &item.kind {
            let params: Vec<_> = arena.get_params(func.params)
                .iter()
                .map(|p| p.name)
                .collect();

            let func_val = Value::Function(FunctionValue {
                params,
                body: func.body,
                captures: Rc::new(RefCell::new(HashMap::new())),
            });

            env.define(func.name, func_val, false);
        }
    }
}

/// Run the full pipeline: lex -> parse -> setup env -> eval @main
fn run_full_pipeline(source: &str, interner: &StringInterner) -> Value {
    // Lex
    let lexer = Lexer::new(source, interner);
    let tokens = lexer.lex_all();

    // Parse
    let parser = Parser::new(&tokens, interner);
    let parse_result = parser.parse_module();

    // Setup environment
    let mut env = Environment::new();
    register_functions(&mut env, &parse_result.items, &parse_result.arena);

    // Find and eval @main
    let mut evaluator = Evaluator::with_env(interner, &parse_result.arena, env);
    
    for item in &parse_result.items {
        if let ItemKind::Function(func) = &item.kind {
            let name = interner.lookup(func.name);
            if name == "main" {
                return evaluator.eval(func.body).unwrap_or(Value::Void);
            }
        }
    }
    
    Value::Void
}

fn bench_e2e_simple(c: &mut Criterion) {
    let mut group = c.benchmark_group("e2e_simple");
    // Use flat sampling to reduce iterations and avoid memory exhaustion
    group.sampling_mode(SamplingMode::Flat);
    group.sample_size(10);

    // Reduced from [10, 50, 100, 500] to avoid memory exhaustion in WSL
    for num_funcs in [10, 50, 100] {
        let code = generate_module(num_funcs);
        let bytes = code.len() as u64;
        let lines = code.lines().count();

        group.throughput(Throughput::Bytes(bytes));
        group.bench_with_input(
            BenchmarkId::new("functions", format!("{}_({}_LOC)", num_funcs, lines)),
            &code,
            |b, code| {
                b.iter(|| {
                    let interner = StringInterner::new();
                    black_box(run_full_pipeline(code, &interner))
                })
            },
        );
    }

    group.finish();
}

fn bench_e2e_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("e2e_patterns");
    group.sampling_mode(SamplingMode::Flat);
    group.sample_size(10);

    for num_funcs in [10, 50, 100] {
        let code = generate_pattern_module(num_funcs);
        let bytes = code.len() as u64;
        let lines = code.lines().count();

        group.throughput(Throughput::Bytes(bytes));
        group.bench_with_input(
            BenchmarkId::new("functions", format!("{}_({}_LOC)", num_funcs, lines)),
            &code,
            |b, code| {
                b.iter(|| {
                    let interner = StringInterner::new();
                    black_box(run_full_pipeline(code, &interner))
                })
            },
        );
    }

    group.finish();
}

fn bench_e2e_phases(c: &mut Criterion) {
    let code = generate_module(100);
    let interner = StringInterner::new();

    let mut group = c.benchmark_group("e2e_phases_100_funcs");
    group.sampling_mode(SamplingMode::Flat);
    group.sample_size(10);

    // Lex only
    group.bench_function("1_lex", |b| {
        b.iter(|| {
            let lexer = Lexer::new(black_box(&code), &interner);
            black_box(lexer.lex_all())
        })
    });

    // Lex + Parse
    let tokens = Lexer::new(&code, &interner).lex_all();
    group.bench_function("2_parse", |b| {
        b.iter(|| {
            let parser = Parser::new(black_box(&tokens), &interner);
            black_box(parser.parse_module())
        })
    });

    // Full pipeline
    group.bench_function("3_full", |b| {
        b.iter(|| {
            let interner = StringInterner::new();
            black_box(run_full_pipeline(&code, &interner))
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_e2e_simple,
    bench_e2e_patterns,
    bench_e2e_phases,
);

criterion_main!(benches);
