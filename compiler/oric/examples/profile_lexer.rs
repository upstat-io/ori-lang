//! Standalone lexer profiling harness.
//!
//! Lexes a generated ~50KB input repeatedly, suitable for perf/callgrind/flamegraph.
//!
//! Usage:
//!   `cargo build -p oric --example profile_lexer --release`
//!   `valgrind --tool=callgrind target/release/examples/profile_lexer 50`
//!   `perf record -g target/release/examples/profile_lexer 200`
//!   `cargo flamegraph --example profile_lexer -- 200`

use std::hint::black_box;

fn generate_n_functions(n: usize) -> String {
    (0..n)
        .map(|i| format!("@func{i} (x: int) -> int = x + {i}"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[allow(
    clippy::cast_precision_loss,
    reason = "total bytes fits comfortably in f64 for throughput display"
)]
fn main() {
    let source = generate_n_functions(1500); // ~50KB
    let interner = ori_ir::StringInterner::new();

    let iterations: usize = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(100);

    eprintln!(
        "Lexing {} bytes x {} iterations ({:.1} MB total)",
        source.len(),
        iterations,
        (source.len() * iterations) as f64 / 1_000_000.0
    );

    for _ in 0..iterations {
        black_box(ori_lexer::lex(&source, &interner));
    }
}
