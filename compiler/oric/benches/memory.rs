//! Memory benchmarks for Ori compiler.
//!
//! Measures heap allocations and peak memory usage for lexer and parser.
//!
//! Unlike throughput benchmarks, these run single iterations with detailed
//! allocation tracking to understand memory characteristics.

// Benchmark-specific lints
#![allow(unsafe_code)] // Required for GlobalAlloc
#![allow(clippy::cast_precision_loss)] // Acceptable for KB display
#![allow(clippy::uninlined_format_args)] // Clearer in benchmarks

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use ori_ir::StringInterner;

/// Tracking allocator that records allocation statistics.
struct TrackingAllocator {
    allocated: AtomicUsize,
    peak: AtomicUsize,
    allocation_count: AtomicUsize,
}

impl TrackingAllocator {
    const fn new() -> Self {
        Self {
            allocated: AtomicUsize::new(0),
            peak: AtomicUsize::new(0),
            allocation_count: AtomicUsize::new(0),
        }
    }

    fn reset(&self) {
        self.allocated.store(0, Ordering::SeqCst);
        self.peak.store(0, Ordering::SeqCst);
        self.allocation_count.store(0, Ordering::SeqCst);
    }

    fn stats(&self) -> MemoryStats {
        MemoryStats {
            peak_bytes: self.peak.load(Ordering::SeqCst),
            allocation_count: self.allocation_count.load(Ordering::SeqCst),
        }
    }
}

unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = System.alloc(layout);
        if !ptr.is_null() {
            let size = layout.size();
            let current = self.allocated.fetch_add(size, Ordering::SeqCst) + size;
            self.allocation_count.fetch_add(1, Ordering::SeqCst);
            // Update peak if current exceeds it
            let mut peak = self.peak.load(Ordering::SeqCst);
            while current > peak {
                match self.peak.compare_exchange_weak(
                    peak,
                    current,
                    Ordering::SeqCst,
                    Ordering::SeqCst,
                ) {
                    Ok(_) => break,
                    Err(p) => peak = p,
                }
            }
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.allocated.fetch_sub(layout.size(), Ordering::SeqCst);
        System.dealloc(ptr, layout);
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_ptr = System.realloc(ptr, layout, new_size);
        if !new_ptr.is_null() {
            let old_size = layout.size();
            if new_size > old_size {
                let diff = new_size - old_size;
                let current = self.allocated.fetch_add(diff, Ordering::SeqCst) + diff;
                // Update peak
                let mut peak = self.peak.load(Ordering::SeqCst);
                while current > peak {
                    match self.peak.compare_exchange_weak(
                        peak,
                        current,
                        Ordering::SeqCst,
                        Ordering::SeqCst,
                    ) {
                        Ok(_) => break,
                        Err(p) => peak = p,
                    }
                }
            } else {
                self.allocated
                    .fetch_sub(old_size - new_size, Ordering::SeqCst);
            }
        }
        new_ptr
    }
}

#[global_allocator]
static ALLOCATOR: TrackingAllocator = TrackingAllocator::new();

#[derive(Debug, Clone, Copy)]
struct MemoryStats {
    peak_bytes: usize,
    allocation_count: usize,
}

impl MemoryStats {
    fn peak_kb(&self) -> f64 {
        self.peak_bytes as f64 / 1024.0
    }

    fn bytes_per_source_byte(&self, source_len: usize) -> f64 {
        if source_len == 0 {
            0.0
        } else {
            self.peak_bytes as f64 / source_len as f64
        }
    }
}

/// Generate N simple functions.
fn generate_n_functions(n: usize) -> String {
    (0..n)
        .map(|i| format!("@func{i} (x: int) -> int = x + {i}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Measure memory for lexer only.
fn measure_lexer_memory(source: &str) -> MemoryStats {
    ALLOCATOR.reset();

    let interner = StringInterner::new();
    let _tokens = ori_lexer::lex(source, &interner);

    ALLOCATOR.stats()
}

/// Measure memory for parser (includes lexer).
fn measure_parser_memory(source: &str) -> MemoryStats {
    ALLOCATOR.reset();

    let interner = StringInterner::new();
    let tokens = ori_lexer::lex(source, &interner);
    let _module = ori_parse::parse(&tokens, &interner);

    ALLOCATOR.stats()
}

fn bench_memory_lexer(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory/lexer");

    for num_functions in [10, 100, 500, 1000] {
        let source = generate_n_functions(num_functions);
        let source_len = source.len();

        group.bench_with_input(
            BenchmarkId::new("peak_kb", num_functions),
            &source,
            |b, source| {
                b.iter_custom(|iters| {
                    let mut total_peak = 0usize;
                    for _ in 0..iters {
                        let stats = measure_lexer_memory(source);
                        total_peak += stats.peak_bytes;
                    }
                    // Return as duration (abusing the API to report memory)
                    std::time::Duration::from_nanos(total_peak as u64 / iters)
                });
            },
        );

        // Single measurement for detailed stats
        let stats = measure_lexer_memory(&source);
        println!(
            "\n  Lexer ({} funcs, {} bytes source):",
            num_functions, source_len
        );
        println!("    Peak: {:.1} KB", stats.peak_kb());
        println!("    Allocations: {}", stats.allocation_count);
        println!(
            "    Memory amplification: {:.1}x source size",
            stats.bytes_per_source_byte(source_len)
        );
    }

    group.finish();
}

fn bench_memory_parser(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory/parser");

    for num_functions in [10, 100, 500, 1000] {
        let source = generate_n_functions(num_functions);
        let source_len = source.len();

        group.bench_with_input(
            BenchmarkId::new("peak_kb", num_functions),
            &source,
            |b, source| {
                b.iter_custom(|iters| {
                    let mut total_peak = 0usize;
                    for _ in 0..iters {
                        let stats = measure_parser_memory(source);
                        total_peak += stats.peak_bytes;
                    }
                    std::time::Duration::from_nanos(total_peak as u64 / iters)
                });
            },
        );

        // Single measurement for detailed stats
        let stats = measure_parser_memory(&source);
        println!(
            "\n  Parser ({} funcs, {} bytes source):",
            num_functions, source_len
        );
        println!("    Peak: {:.1} KB", stats.peak_kb());
        println!("    Allocations: {}", stats.allocation_count);
        println!(
            "    Memory amplification: {:.1}x source size",
            stats.bytes_per_source_byte(source_len)
        );
    }

    group.finish();
}

/// Summary benchmark that prints a memory report.
fn bench_memory_summary(c: &mut Criterion) {
    println!("\n{}", "=".repeat(60));
    println!("MEMORY SUMMARY");
    println!("{}", "=".repeat(60));

    let workloads = [
        ("small", 10),
        ("medium", 100),
        ("large", 500),
        ("xlarge", 1000),
    ];

    println!("\n| Workload | Source | Lexer Peak | Parser Peak | Amplification |");
    println!("|----------|--------|------------|-------------|---------------|");

    for (name, n) in workloads {
        let source = generate_n_functions(n);
        let source_kb = source.len() as f64 / 1024.0;

        let lexer_stats = measure_lexer_memory(&source);
        let parser_stats = measure_parser_memory(&source);

        println!(
            "| {} ({} funcs) | {:.1} KB | {:.1} KB | {:.1} KB | {:.1}x |",
            name,
            n,
            source_kb,
            lexer_stats.peak_kb(),
            parser_stats.peak_kb(),
            parser_stats.bytes_per_source_byte(source.len())
        );
    }

    println!();

    // Dummy benchmark to satisfy Criterion
    c.bench_function("memory/summary", |b| {
        b.iter(|| 1 + 1);
    });
}

criterion_group!(
    benches,
    bench_memory_lexer,
    bench_memory_parser,
    bench_memory_summary,
);
criterion_main!(benches);
