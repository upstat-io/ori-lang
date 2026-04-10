---
paths:
  - "**/*.rs"
---

# API, Compilation & Concurrency

Extracted from `impl-hygiene.md` -- conditional compilation, lifetime annotations, API stability, dependencies, and concurrency rules.

## Conditional Compilation

- `#[cfg(test)]` for test modules and test helpers only, not for production logic branching
- `#[cfg(debug_assertions)]` for debug-only checks
- Production code must not change behavior based on `#[cfg(test)]`

## Lifetime Annotations

- Prefer elision when possible
- Descriptive names for long-lived borrows: `'src`, `'ast`, `'ctx`
- Single-letter (`'a`) only for local/obvious cases
- Avoid >2 lifetime parameters per function

## API Stability

- Pub items in `lib.rs` are the stable API surface
- Breaking changes to pub crate APIs must update all downstream consumers in the same commit
- When replacing a code path, remove the old code in the same commit. No deprecation for internal compiler code.

## Dependencies

- Prefer `std` over external crates
- New external deps require justification
- Features are additive only (never remove functionality). Each feature documented in `Cargo.toml`.

## Concurrency

- Compiler internals are single-threaded (Salsa handles parallelism)
- No global mutable state (`static mut`, `lazy_static` with mutation). All state flows through function parameters or Salsa queries.
- Thread-safety required only at `ori_rt` FFI boundary
