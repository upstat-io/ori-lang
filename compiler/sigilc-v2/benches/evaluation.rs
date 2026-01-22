//! Benchmarks for the Sigil interpreter and pattern system.
//!
//! ## Phase 3 Baselines (Patterns & Evaluation)
//!
//! | Benchmark                     | Time      | Notes                              |
//! |-------------------------------|-----------|-----------------------------------|
//! | eval_arithmetic               | 90 ns     | Basic arithmetic (1+2*3-4/2)      |
//! | eval_comparison               | 86 ns     | Comparison and boolean ops        |
//! | eval_if_expression            | 30 ns     | Simple conditional evaluation     |
//! | eval_list_construction        | 239 ns    | Building list of 10 elements      |
//! | scope_push_pop                | 246 ns    | 10 push/pop cycles                |
//! | environment_define_lookup     | 370 ns    | Define and lookup 10 variables    |
//! | struct_field_access           | 71 ns     | O(1) field lookup (8 fields)      |
//! | value_type_check              | 6.2 ns    | is_truthy + type_name (7 values)  |
//! | pattern_registry_lookup       | 36 ns     | 7 pattern lookups                 |
//! | fusion_hints_creation         | 8.5 ns    | Create hints for 3 fused patterns |
//! | value_clone                   | 16 ns     | Clone 4 value types               |
//!
//! Target: Keep simple expression eval under 100ns, function call under 500ns

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::rc::Rc;
use std::collections::HashMap;

use sigilc_v2::eval::{Value, Environment};
use sigilc_v2::intern::StringInterner;
use sigilc_v2::syntax::{Lexer, Parser};
use sigilc_v2::patterns::{PatternRegistry, FusedPattern, FusionHints};

fn bench_eval_arithmetic(c: &mut Criterion) {
    let interner = StringInterner::new();
    let lexer = Lexer::new("1 + 2 * 3 - 4 / 2", &interner);
    let tokens = lexer.lex_all();
    let parser = Parser::new(&tokens, &interner);
    let (expr_id, arena, _) = parser.parse_expression();

    c.bench_function("eval_arithmetic", |b| {
        b.iter(|| {
            let mut evaluator = sigilc_v2::eval::Evaluator::new(&interner, &arena);
            black_box(evaluator.eval(expr_id))
        })
    });
}

fn bench_eval_comparison(c: &mut Criterion) {
    let interner = StringInterner::new();
    let lexer = Lexer::new("1 < 2 && 3 > 2 || false", &interner);
    let tokens = lexer.lex_all();
    let parser = Parser::new(&tokens, &interner);
    let (expr_id, arena, _) = parser.parse_expression();

    c.bench_function("eval_comparison", |b| {
        b.iter(|| {
            let mut evaluator = sigilc_v2::eval::Evaluator::new(&interner, &arena);
            black_box(evaluator.eval(expr_id))
        })
    });
}

fn bench_eval_if_expression(c: &mut Criterion) {
    let interner = StringInterner::new();
    let lexer = Lexer::new("if true then 1 else 2", &interner);
    let tokens = lexer.lex_all();
    let parser = Parser::new(&tokens, &interner);
    let (expr_id, arena, _) = parser.parse_expression();

    c.bench_function("eval_if_expression", |b| {
        b.iter(|| {
            let mut evaluator = sigilc_v2::eval::Evaluator::new(&interner, &arena);
            black_box(evaluator.eval(expr_id))
        })
    });
}

fn bench_eval_list_construction(c: &mut Criterion) {
    let interner = StringInterner::new();
    let lexer = Lexer::new("[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]", &interner);
    let tokens = lexer.lex_all();
    let parser = Parser::new(&tokens, &interner);
    let (expr_id, arena, _) = parser.parse_expression();

    c.bench_function("eval_list_construction", |b| {
        b.iter(|| {
            let mut evaluator = sigilc_v2::eval::Evaluator::new(&interner, &arena);
            black_box(evaluator.eval(expr_id))
        })
    });
}

fn bench_scope_push_pop(c: &mut Criterion) {
    c.bench_function("scope_push_pop", |b| {
        b.iter(|| {
            let mut env = Environment::new();
            for _ in 0..10 {
                env.push_scope();
            }
            for _ in 0..10 {
                env.pop_scope();
            }
            black_box(&env);
        })
    });
}

fn bench_environment_define_lookup(c: &mut Criterion) {
    let interner = StringInterner::new();
    let names: Vec<_> = (0..10)
        .map(|i| interner.intern(&format!("var_{}", i)))
        .collect();

    c.bench_function("environment_define_lookup", |b| {
        b.iter(|| {
            let mut env = Environment::new();
            // Define 10 variables
            for (i, name) in names.iter().enumerate() {
                env.define(*name, Value::Int(i as i64), false);
            }
            // Look up all variables
            for name in &names {
                black_box(env.lookup(*name));
            }
        })
    });
}

fn bench_struct_field_access(c: &mut Criterion) {
    use sigilc_v2::eval::{StructValue, StructLayout};

    let interner = StringInterner::new();
    let field_names: Vec<_> = ["x", "y", "z", "w", "a", "b", "c", "d"]
        .iter()
        .map(|s| interner.intern(s))
        .collect();

    let layout = Rc::new(StructLayout::new(&field_names));
    let fields = Rc::new(vec![
        Value::Int(1), Value::Int(2), Value::Int(3), Value::Int(4),
        Value::Int(5), Value::Int(6), Value::Int(7), Value::Int(8),
    ]);

    let struct_val = StructValue {
        type_name: interner.intern("Point"),
        fields: fields.clone(),
        layout: layout.clone(),
    };

    c.bench_function("struct_field_access", |b| {
        b.iter(|| {
            for name in &field_names {
                black_box(struct_val.get_field(*name));
            }
        })
    });
}

fn bench_value_type_check(c: &mut Criterion) {
    let values = vec![
        Value::Int(42),
        Value::Float(3.14),
        Value::Bool(true),
        Value::Str(Rc::new("hello".to_string())),
        Value::List(Rc::new(vec![Value::Int(1), Value::Int(2)])),
        Value::None,
        Value::Some(Box::new(Value::Int(1))),
    ];

    c.bench_function("value_type_check", |b| {
        b.iter(|| {
            for v in &values {
                black_box(v.is_truthy());
                black_box(v.type_name());
            }
        })
    });
}

fn bench_pattern_registry_lookup(c: &mut Criterion) {
    let mut registry = PatternRegistry::new();
    // Register all builtins
    sigilc_v2::patterns::builtins::register_all(&mut registry);

    let keywords = ["map", "filter", "fold", "find", "collect", "run", "try"];

    c.bench_function("pattern_registry_lookup", |b| {
        b.iter(|| {
            for kw in &keywords {
                black_box(registry.get(kw));
            }
        })
    });
}

fn bench_fusion_hints(c: &mut Criterion) {
    use sigilc_v2::syntax::{ExprId, PatternKind};

    c.bench_function("fusion_hints_creation", |b| {
        b.iter(|| {
            // Create fusion hints for various patterns
            let patterns = [
                FusedPattern::Single(PatternKind::Map),
                FusedPattern::MapFilter {
                    map_transform: ExprId::INVALID,
                    filter_predicate: ExprId::INVALID,
                },
                FusedPattern::MapFold {
                    map_transform: ExprId::INVALID,
                    fold_init: ExprId::INVALID,
                    fold_op: ExprId::INVALID,
                },
            ];

            for p in &patterns {
                black_box(FusionHints::for_fused(p.clone()));
            }
        })
    });
}

fn bench_value_clone(c: &mut Criterion) {
    // Test cloning performance for various value types
    let values = vec![
        Value::Int(42),
        Value::Str(Rc::new("a moderately long string for testing".to_string())),
        Value::List(Rc::new((0..100).map(Value::Int).collect())),
        Value::Map(Rc::new({
            let mut m = HashMap::new();
            for i in 0..10 {
                m.insert(format!("key_{}", i), Value::Int(i));
            }
            m
        })),
    ];

    c.bench_function("value_clone", |b| {
        b.iter(|| {
            for v in &values {
                black_box(v.clone());
            }
        })
    });
}

criterion_group!(
    benches,
    bench_eval_arithmetic,
    bench_eval_comparison,
    bench_eval_if_expression,
    bench_eval_list_construction,
    bench_scope_push_pop,
    bench_environment_define_lookup,
    bench_struct_field_access,
    bench_value_type_check,
    bench_pattern_registry_lookup,
    bench_fusion_hints,
    bench_value_clone,
);

criterion_main!(benches);
