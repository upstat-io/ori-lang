//! Raw scanner benchmarks for `ori_lexer_core`.
//!
//! Measures pure tokenization throughput — no keyword resolution, no literal
//! parsing, no interning, no diagnostics. This is the apples-to-apples
//! comparison point with other published lexer benchmarks (`rustc_lexer`, Zig,
//! Go, tree-sitter).

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use ori_lexer_core::{RawScanner, RawTag, SourceBuffer};

/// Generate N simple functions for scaling benchmarks.
fn generate_n_functions(n: usize) -> String {
    (0..n)
        .map(|i| format!("@func{i} (x: int) -> int = x + {i}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Benchmark raw scanner throughput at various scales.
///
/// Consumes tokens in a tight loop without collecting into a Vec,
/// measuring pure scanning speed.
fn bench_core_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("lexer_core/raw/throughput");

    for num_functions in [10, 50, 100, 500, 1000, 5000] {
        let source = generate_n_functions(num_functions);
        let bytes = source.len() as u64;

        group.throughput(Throughput::Bytes(bytes));
        group.bench_with_input(
            BenchmarkId::from_parameter(num_functions),
            &source,
            |b, src| {
                b.iter(|| {
                    let buf = SourceBuffer::new(src);
                    let mut scanner = RawScanner::new(buf.cursor());
                    loop {
                        let tok = scanner.next_token();
                        if tok.tag == RawTag::Eof {
                            break;
                        }
                        black_box(tok);
                    }
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_core_throughput);
criterion_main!(benches);
