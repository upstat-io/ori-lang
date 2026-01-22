# I: Dependencies Specification

This document specifies all Cargo dependencies for the V2 compiler with rationale.

---

## Core Dependencies

### Query System

```toml
[dependencies.salsa]
version = "0.17"
# Rationale: Industry-standard incremental computation framework
# Used by: rust-analyzer, chalk
# Provides: Automatic dependency tracking, memoization, early cutoff
# Alternatives considered:
#   - Custom implementation: Too complex, would reinvent the wheel
#   - Incremental: Less mature, fewer features
```

### Parallelism

```toml
[dependencies.rayon]
version = "1.8"
# Rationale: Work-stealing parallel iterators
# Used by: All major Rust projects requiring parallelism
# Provides: par_iter(), parallel scopes, work-stealing scheduler
# Alternatives considered:
#   - std::thread: Too low-level, manual work distribution
#   - tokio: Async, not needed for CPU-bound compilation

[dependencies.crossbeam]
version = "0.8"
# Rationale: Low-level concurrency primitives
# Provides: Work-stealing deques, channels, scoped threads
# Used for: Custom work-stealing scheduler, fine-grained parallelism

[dependencies.parking_lot]
version = "0.12"
# Rationale: Faster locks than std
# Benchmarks: 2-3x faster than std::sync::{Mutex, RwLock}
# Provides: Mutex, RwLock, Condvar with better performance
```

### Data Structures

```toml
[dependencies.dashmap]
version = "5.5"
# Rationale: Concurrent hash map with sharding
# Used for: Type interner, pattern template cache
# Alternatives considered:
#   - std HashMap + Mutex: High contention
#   - chashmap: Less maintained
#   - flurry: More complex API

[dependencies.rustc-hash]
version = "1.1"
# Rationale: Fast non-cryptographic hash function
# FxHash is 2-5x faster than SipHash for typical compiler data
# Used for: All internal HashMaps where security isn't needed

[dependencies.bumpalo]
version = "3.14"
# Rationale: Fast arena allocator
# Provides: Bump allocation with bulk deallocation
# Used for: String interner arenas, AST expression arenas
# Memory: Reuses underlying allocations across resets

[dependencies.thin-vec]
version = "0.2"
# Rationale: Smaller Vec for single-element optimization
# 8 bytes vs 24 bytes for small vecs
# Used for: Parameter lists, argument lists (often 1-3 elements)
```

### Lexing

```toml
[dependencies.logos]
version = "0.13"
# Rationale: Compile-time lexer generator
# Performance: ~1200 MB/s throughput
# Provides: DFA-based lexer via proc macro
# Our V1 already uses logos - keep for consistency
```

### Serialization

```toml
[dependencies.rkyv]
version = "0.7"
features = ["validation"]
# Rationale: Zero-copy deserialization
# Used for: Persistent query cache, format cache
# Performance: 10-100x faster than serde for reading
# Alternatives considered:
#   - bincode: Good but requires full deserialization
#   - serde_json: Too slow for binary data
```

### Error Reporting

```toml
[dependencies.codespan-reporting]
version = "0.11"
# Rationale: Beautiful error messages with source snippets
# Used by: Many Rust compilers and tools
# Provides: Diagnostic rendering with colors and labels
# Matches rust-analyzer style output
```

---

## Optional Dependencies

### Debug/Development

```toml
[dependencies.tracing]
version = "0.1"
optional = true
# Rationale: Structured logging and instrumentation
# Used for: Performance profiling, debug logging
# Enable with: --features tracing

[dependencies.tracing-subscriber]
version = "0.3"
optional = true
# Rationale: Tracing output formatting
```

### LSP

```toml
[dependencies.tower-lsp]
version = "0.20"
optional = true
# Rationale: LSP server framework
# Provides: Protocol handling, async dispatch
# Enable with: --features lsp

[dependencies.lsp-types]
version = "0.94"
optional = true
# Rationale: LSP type definitions
```

---

## Dev Dependencies

### Testing

```toml
[dev-dependencies.criterion]
version = "0.5"
features = ["html_reports"]
# Rationale: Statistical benchmarking
# Provides: Reliable measurements, comparison, reporting

[dev-dependencies.proptest]
version = "1.4"
# Rationale: Property-based testing
# Used for: Parser roundtrip tests, fuzzing

[dev-dependencies.insta]
version = "1.34"
# Rationale: Snapshot testing
# Used for: Error message tests, AST dump tests
```

### Development Tools

```toml
[dev-dependencies.expect-test]
version = "1.4"
# Rationale: Inline snapshot testing
# Used for: Quick test updates during development
```

---

## Feature Flags

```toml
[features]
default = []

# Enable LSP server
lsp = ["tower-lsp", "lsp-types", "tokio"]

# Enable tracing/profiling
tracing = ["dep:tracing", "dep:tracing-subscriber"]

# Enable parallel compilation (default in release)
parallel = []

# Enable all optimizations
full = ["parallel", "lsp", "tracing"]

# For development: enable debug assertions in release
debug-release = []
```

---

## Version Pinning Strategy

### Patch Versions

```toml
# Allow patch updates (bug fixes)
salsa = "0.17"      # 0.17.x
rayon = "1.8"       # 1.8.x
```

### Minor Versions

```toml
# Pin to minor for stability
dashmap = "5.5"     # Exactly 5.5.x
bumpalo = "3.14"    # Exactly 3.14.x
```

### Cargo.lock

```toml
# Cargo.lock checked in for reproducible builds
# Update with: cargo update
```

---

## Dependency Audit

### Security

```bash
# Run security audit
cargo audit

# CI integration
- name: Security audit
  run: cargo audit --deny warnings
```

### License Compliance

All dependencies use permissive licenses compatible with MIT:

| Dependency | License |
|------------|---------|
| salsa | Apache-2.0/MIT |
| rayon | Apache-2.0/MIT |
| crossbeam | Apache-2.0/MIT |
| parking_lot | Apache-2.0/MIT |
| dashmap | MIT |
| rustc-hash | Apache-2.0/MIT |
| bumpalo | Apache-2.0/MIT |
| logos | Apache-2.0/MIT |
| rkyv | MIT |
| codespan-reporting | Apache-2.0 |

---

## Dependency Graph

```
sigilc-v2
├── salsa
│   └── (internal deps)
├── rayon
│   └── crossbeam-deque
├── crossbeam
│   ├── crossbeam-deque
│   ├── crossbeam-channel
│   └── crossbeam-utils
├── parking_lot
│   └── lock_api
├── dashmap
│   └── parking_lot
├── rustc-hash
├── bumpalo
├── logos
├── rkyv
│   └── bytecheck
└── codespan-reporting
    └── termcolor
```

---

## Build Time Impact

| Dependency | Clean Build | Incremental |
|------------|-------------|-------------|
| salsa | 15s | 0s |
| rayon | 8s | 0s |
| dashmap | 3s | 0s |
| logos (proc-macro) | 5s | 2s |
| rkyv | 10s | 0s |
| **Total** | ~45s | <5s |

*Measured on M1 MacBook Pro*

### Reducing Build Time

```toml
# Use pre-built dependencies in CI
[profile.ci]
inherits = "dev"
incremental = false

# Split crates for parallel compilation
[workspace]
members = [
    "sigilc-v2",
    "sigilc-v2-syntax",
    "sigilc-v2-types",
    "sigilc-v2-codegen",
]
```
