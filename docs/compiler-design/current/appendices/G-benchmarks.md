# G: Benchmarks Specification

This document specifies benchmark targets, methodology, and Criterion setup for the V2 compiler.

---

## Performance Targets

### Cold Compilation

| Project Size | V1 Baseline | V2 Target | Improvement |
|--------------|-------------|-----------|-------------|
| 100 LOC | 50ms | 5ms | 10x |
| 1K LOC | 500ms | 50ms | 10x |
| 10K LOC | 5s | 300ms | 16x |
| 100K LOC | 50s | 2s | 25x |

### Incremental Compilation

| Change Type | V1 Baseline | V2 Target |
|-------------|-------------|-----------|
| Comment change | 500ms | <10ms |
| Single statement | 500ms | <30ms |
| Function body | 500ms | <50ms |
| Function signature | 500ms | <100ms |
| Type definition | 500ms | <200ms |
| New file | 500ms | <100ms |

### Memory Usage

| Project Size | V1 Baseline | V2 Target | Reduction |
|--------------|-------------|-----------|-----------|
| 1K LOC | 20MB | 5MB | 4x |
| 10K LOC | 200MB | 50MB | 4x |
| 100K LOC | 2GB | 400MB | 5x |

### Throughput

| Operation | V1 Baseline | V2 Target |
|-----------|-------------|-----------|
| Lexing | 50K LOC/s | 500K LOC/s |
| Parsing | 20K LOC/s | 200K LOC/s |
| Type checking | 5K LOC/s | 50K LOC/s |
| Codegen | 10K LOC/s | 100K LOC/s |

---

## Benchmark Corpus

### Synthetic Benchmarks

```rust
/// Generate synthetic source for benchmarking
pub fn generate_source(config: &SourceConfig) -> String {
    let mut source = String::new();

    // Imports
    for i in 0..config.import_count {
        writeln!(source, "use std.test{} {{ func{} }}", i, i);
    }
    source.push('\n');

    // Configs
    for i in 0..config.config_count {
        writeln!(source, "$config{} = {}", i, i);
    }
    source.push('\n');

    // Functions
    for i in 0..config.function_count {
        writeln!(source, "@func{} (x: int) -> int = run(", i);
        for j in 0..config.statements_per_function {
            writeln!(source, "    let v{} = x + {},", j, j);
        }
        writeln!(source, "    v{})", config.statements_per_function - 1);
        source.push('\n');
    }

    // Tests
    for i in 0..config.function_count {
        writeln!(source, "@test_func{} tests @func{} () -> void = run(", i, i);
        writeln!(source, "    assert_eq(func{}(0), {})", i, config.statements_per_function - 1);
        writeln!(source, ")");
        source.push('\n');
    }

    source
}

pub struct SourceConfig {
    pub import_count: usize,
    pub config_count: usize,
    pub function_count: usize,
    pub statements_per_function: usize,
}

impl SourceConfig {
    pub fn small() -> Self {
        Self {
            import_count: 5,
            config_count: 2,
            function_count: 10,
            statements_per_function: 5,
        }
    }

    pub fn medium() -> Self {
        Self {
            import_count: 20,
            config_count: 10,
            function_count: 100,
            statements_per_function: 10,
        }
    }

    pub fn large() -> Self {
        Self {
            import_count: 50,
            config_count: 20,
            function_count: 1000,
            statements_per_function: 20,
        }
    }
}
```

### Real-World Corpus

```
benches/corpus/
├── small/           # <1K LOC each
│   ├── hello.si
│   ├── fibonacci.si
│   └── calculator.si
├── medium/          # 1K-10K LOC each
│   ├── json_parser.si
│   ├── http_client.si
│   └── data_processor.si
└── large/           # >10K LOC
    ├── stdlib.si    # Full stdlib
    └── compiler.si  # Self-hosted (future)
```

---

## Criterion Setup

### Cargo.toml

```toml
[dev-dependencies]
criterion = { version = "0.5", features = ["html_reports"] }

[[bench]]
name = "lexer"
harness = false

[[bench]]
name = "parser"
harness = false

[[bench]]
name = "type_check"
harness = false

[[bench]]
name = "codegen"
harness = false

[[bench]]
name = "incremental"
harness = false

[[bench]]
name = "e2e"
harness = false
```

### Lexer Benchmarks

```rust
// benches/lexer.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};

fn bench_lex_small(c: &mut Criterion) {
    let source = include_str!("corpus/small/hello.si");

    let mut group = c.benchmark_group("lexer/small");
    group.throughput(Throughput::Bytes(source.len() as u64));

    group.bench_function("lex", |b| {
        let interner = StringInterner::new();
        b.iter(|| {
            let tokens = lex(black_box(source), &interner);
            black_box(tokens)
        })
    });

    group.finish();
}

fn bench_lex_medium(c: &mut Criterion) {
    let source = generate_source(&SourceConfig::medium());

    let mut group = c.benchmark_group("lexer/medium");
    group.throughput(Throughput::Bytes(source.len() as u64));
    group.sample_size(50);

    group.bench_function("lex", |b| {
        let interner = StringInterner::new();
        b.iter(|| lex(black_box(&source), &interner))
    });

    group.finish();
}

fn bench_lex_large(c: &mut Criterion) {
    let source = generate_source(&SourceConfig::large());

    let mut group = c.benchmark_group("lexer/large");
    group.throughput(Throughput::Bytes(source.len() as u64));
    group.sample_size(20);

    group.bench_function("lex", |b| {
        let interner = StringInterner::new();
        b.iter(|| lex(black_box(&source), &interner))
    });

    group.finish();
}

criterion_group!(benches, bench_lex_small, bench_lex_medium, bench_lex_large);
criterion_main!(benches);
```

### Parser Benchmarks

```rust
// benches/parser.rs
fn bench_parse_small(c: &mut Criterion) {
    let source = include_str!("corpus/small/hello.si");
    let interner = StringInterner::new();
    let tokens = lex(source, &interner);

    let mut group = c.benchmark_group("parser/small");
    group.throughput(Throughput::Elements(count_functions(source) as u64));

    group.bench_function("parse", |b| {
        b.iter(|| {
            let mut arena = ExprArena::new();
            let mut parser = Parser::new(&tokens, &interner, &mut arena);
            black_box(parser.parse_module())
        })
    });

    group.finish();
}

fn bench_parse_parallel(c: &mut Criterion) {
    let files: Vec<_> = (0..100)
        .map(|i| generate_source(&SourceConfig::small()))
        .collect();

    let mut group = c.benchmark_group("parser/parallel");
    group.throughput(Throughput::Elements(files.len() as u64));
    group.sample_size(20);

    group.bench_function("parse_all", |b| {
        let interner = Arc::new(StringInterner::new());

        b.iter(|| {
            files.par_iter()
                .map(|source| {
                    let tokens = lex(source, &interner);
                    let mut arena = ExprArena::new();
                    let mut parser = Parser::new(&tokens, &interner, &mut arena);
                    parser.parse_module()
                })
                .collect::<Vec<_>>()
        })
    });

    group.finish();
}
```

### Type Checking Benchmarks

```rust
// benches/type_check.rs
fn bench_typecheck_medium(c: &mut Criterion) {
    let source = generate_source(&SourceConfig::medium());
    let db = setup_db_with_source(&source);

    let mut group = c.benchmark_group("typecheck/medium");
    group.throughput(Throughput::Elements(100));  // 100 functions
    group.sample_size(30);

    group.bench_function("check", |b| {
        b.iter(|| {
            let file = db.source_file(PathBuf::from("test.si"));
            let module = parsed_module(&db, file);
            black_box(typed_module(&db, module))
        })
    });

    group.finish();
}

fn bench_typecheck_parallel(c: &mut Criterion) {
    let db = setup_multimodule_project(50);  // 50 modules

    let mut group = c.benchmark_group("typecheck/parallel");
    group.sample_size(20);

    group.bench_function("check_all", |b| {
        b.iter(|| {
            black_box(type_check_project(&db))
        })
    });

    group.finish();
}
```

### Incremental Benchmarks

```rust
// benches/incremental.rs
fn bench_incremental_comment(c: &mut Criterion) {
    let source = generate_source(&SourceConfig::medium());
    let mut db = setup_db_with_source(&source);

    // Initial compile
    let file = db.source_file(PathBuf::from("test.si"));
    let _ = typed_module(&db, parsed_module(&db, file));

    let mut group = c.benchmark_group("incremental/comment");

    group.bench_function("recompile", |b| {
        let mut modified = source.clone();

        b.iter(|| {
            // Add a comment
            modified.insert_str(0, "// comment\n");
            file.set_text(&mut db).to(modified.clone());

            // Recompile
            let module = parsed_module(&db, file);
            black_box(typed_module(&db, module))
        })
    });

    group.finish();
}

fn bench_incremental_function_body(c: &mut Criterion) {
    let source = generate_source(&SourceConfig::medium());
    let mut db = setup_db_with_source(&source);

    // Initial compile
    let file = db.source_file(PathBuf::from("test.si"));
    let _ = typed_module(&db, parsed_module(&db, file));

    let mut group = c.benchmark_group("incremental/function_body");

    group.bench_function("recompile", |b| {
        let mut counter = 0;

        b.iter(|| {
            // Modify a function body
            counter += 1;
            let modified = source.replace("x + 0", &format!("x + {}", counter));
            file.set_text(&mut db).to(modified);

            // Recompile
            let module = parsed_module(&db, file);
            black_box(typed_module(&db, module))
        })
    });

    group.finish();
}
```

### Memory Benchmarks

```rust
// benches/memory.rs
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

struct CountingAllocator;

static ALLOCATED: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATED.fetch_add(layout.size(), Ordering::Relaxed);
        System.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        ALLOCATED.fetch_sub(layout.size(), Ordering::Relaxed);
        System.dealloc(ptr, layout)
    }
}

fn measure_memory<T>(f: impl FnOnce() -> T) -> (T, usize) {
    let before = ALLOCATED.load(Ordering::Relaxed);
    let result = f();
    let after = ALLOCATED.load(Ordering::Relaxed);
    (result, after - before)
}

fn bench_memory_large(c: &mut Criterion) {
    let source = generate_source(&SourceConfig::large());

    let mut group = c.benchmark_group("memory/large");
    group.sample_size(10);

    group.bench_function("compile", |b| {
        b.iter_custom(|iters| {
            let mut total_mem = 0;

            for _ in 0..iters {
                let (_, mem) = measure_memory(|| {
                    let db = setup_db_with_source(&source);
                    let file = db.source_file(PathBuf::from("test.si"));
                    let module = parsed_module(&db, file);
                    typed_module(&db, module)
                });
                total_mem += mem;
            }

            Duration::from_nanos(total_mem as u64)  // Abuse duration for memory
        })
    });

    group.finish();
}
```

---

## Running Benchmarks

### Full Suite

```bash
# Run all benchmarks
cargo bench

# Run specific benchmark
cargo bench --bench lexer

# With detailed HTML report
cargo bench -- --save-baseline main
```

### Comparison

```bash
# Save baseline
cargo bench -- --save-baseline before

# Make changes...

# Compare
cargo bench -- --baseline before
```

### CI Integration

```yaml
# .github/workflows/bench.yml
name: Benchmarks

on:
  pull_request:
    branches: [main]

jobs:
  benchmark:
    runs-on: ubuntu-latest

    steps:
      - uses: actions/checkout@v3

      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable

      - name: Run benchmarks
        run: cargo bench -- --save-baseline pr

      - name: Compare with main
        run: |
          git checkout main
          cargo bench -- --save-baseline main
          git checkout -
          cargo bench -- --baseline main --load-baseline pr
```

---

## Reporting

### Benchmark Report Format

```markdown
## Benchmark Results

### Lexer
| Test | Time | Throughput |
|------|------|------------|
| small | 45μs | 220 MB/s |
| medium | 1.2ms | 250 MB/s |
| large | 12ms | 280 MB/s |

### Parser
| Test | Time | Throughput |
|------|------|------------|
| small | 120μs | 8K funcs/s |
| medium | 3.5ms | 28K funcs/s |
| large | 35ms | 28K funcs/s |

### Type Checking
| Test | Time | Throughput |
|------|------|------------|
| medium | 15ms | 6.6K funcs/s |
| parallel (50 modules) | 45ms | 1.1K modules/s |

### Incremental
| Change | Time |
|--------|------|
| Comment | 2ms |
| Function body | 8ms |
| Function signature | 25ms |

### Memory
| Size | Peak Memory |
|------|-------------|
| 1K LOC | 4.2 MB |
| 10K LOC | 38 MB |
| 100K LOC | 350 MB |
```
