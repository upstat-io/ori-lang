//! Benchmarks for string and type interning.
//!
//! ## Phase 1 Baselines (Foundation)
//!
//! | Benchmark                | Time      | Notes                              |
//! |--------------------------|-----------|-----------------------------------|
//! | intern_new_string        | 300 ns    | Allocating + hashing new string   |
//! | intern_existing_string   | 8.6 ns    | Fast path: already interned       |
//! | lookup_string            | 3.9 ns    | Name -> &str lookup               |
//! | intern_keywords          | 71 ns     | 10 keywords (~7ns each)           |
//! | name_equality            | 392 ps    | u32 comparison (sub-nanosecond)   |
//! | intern_primitive_types   | 52 ns     | 4 primitive types (~13ns each)    |
//! | intern_list_type         | 15 ns     | Compound type interning           |
//! | concurrent/1 thread      | 94 µs     | 100 strings, 1 thread             |
//! | concurrent/4 threads     | 200 µs    | 100 strings each, 4 threads       |
//!
//! ## Phase 2 Baselines (Type System)
//!
//! | Benchmark                | Time      | Change   | Notes                        |
//! |--------------------------|-----------|----------|------------------------------|
//! | intern_new_string        | 269 ns    | -10%     | Performance improved         |
//! | intern_existing_string   | 8.7 ns    | stable   | Fast path still fast         |
//! | lookup_string            | 3.9 ns    | stable   | Lookup unchanged             |
//! | intern_keywords          | 71 ns     | stable   | Keyword interning unchanged  |
//! | name_equality            | 391 ps    | stable   | u32 comparison unchanged     |
//! | intern_primitive_types   | 51 ns     | -2%      | Slightly improved            |
//! | intern_list_type         | 15 ns     | stable   | Compound type unchanged      |
//! | concurrent/1 thread      | 97 µs     | stable   | Within noise                 |
//! | concurrent/4 threads     | 197 µs    | stable   | Within noise                 |
//!
//! Target: Keep existing string lookup under 10ns, new string under 500ns

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use sigilc_v2::intern::{StringInterner, TypeId, TypeInterner, TypeKind};

fn bench_string_intern_new(c: &mut Criterion) {
    let interner = StringInterner::new();

    c.bench_function("intern_new_string", |b| {
        let mut i = 0u64;
        b.iter(|| {
            i += 1;
            let s = format!("unique_string_{}", i);
            black_box(interner.intern(&s))
        })
    });
}

fn bench_string_intern_existing(c: &mut Criterion) {
    let interner = StringInterner::new();
    // Pre-intern the string
    interner.intern("existing_string");

    c.bench_function("intern_existing_string", |b| {
        b.iter(|| {
            black_box(interner.intern("existing_string"))
        })
    });
}

fn bench_string_lookup(c: &mut Criterion) {
    let interner = StringInterner::new();
    let name = interner.intern("lookup_test");

    c.bench_function("lookup_string", |b| {
        b.iter(|| {
            black_box(interner.lookup(name))
        })
    });
}

fn bench_string_intern_keywords(c: &mut Criterion) {
    let interner = StringInterner::new();
    let keywords = ["if", "else", "for", "let", "fn", "type", "match", "true", "false", "map"];

    c.bench_function("intern_keywords", |b| {
        b.iter(|| {
            for kw in &keywords {
                black_box(interner.intern(kw));
            }
        })
    });
}

fn bench_name_comparison(c: &mut Criterion) {
    let interner = StringInterner::new();
    let name1 = interner.intern("compare_test");
    let name2 = interner.intern("compare_test");
    let name3 = interner.intern("different");

    c.bench_function("name_equality", |b| {
        b.iter(|| {
            black_box(name1 == name2);
            black_box(name1 == name3);
        })
    });
}

fn bench_type_intern_primitives(c: &mut Criterion) {
    let interner = TypeInterner::new();

    c.bench_function("intern_primitive_types", |b| {
        b.iter(|| {
            black_box(interner.intern(TypeKind::Int));
            black_box(interner.intern(TypeKind::Float));
            black_box(interner.intern(TypeKind::Bool));
            black_box(interner.intern(TypeKind::Str));
        })
    });
}

fn bench_type_intern_compound(c: &mut Criterion) {
    let interner = TypeInterner::new();

    c.bench_function("intern_list_type", |b| {
        b.iter(|| {
            black_box(interner.intern_list(TypeId::INT))
        })
    });
}

fn bench_concurrent_interning(c: &mut Criterion) {
    use std::sync::Arc;

    let mut group = c.benchmark_group("concurrent_interning");

    for num_threads in [1, 2, 4, 8] {
        group.bench_with_input(
            BenchmarkId::new("threads", num_threads),
            &num_threads,
            |b, &num_threads| {
                let interner = Arc::new(StringInterner::new());
                b.iter(|| {
                    let handles: Vec<_> = (0..num_threads)
                        .map(|t| {
                            let interner = Arc::clone(&interner);
                            std::thread::spawn(move || {
                                for i in 0..100 {
                                    let s = format!("thread_{}_{}", t, i);
                                    black_box(interner.intern(&s));
                                }
                            })
                        })
                        .collect();

                    for h in handles {
                        h.join().unwrap();
                    }
                })
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_string_intern_new,
    bench_string_intern_existing,
    bench_string_lookup,
    bench_string_intern_keywords,
    bench_name_comparison,
    bench_type_intern_primitives,
    bench_type_intern_compound,
    bench_concurrent_interning,
);

criterion_main!(benches);
