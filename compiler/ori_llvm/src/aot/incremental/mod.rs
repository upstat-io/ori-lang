//! Incremental Compilation Support
//!
//! This module provides incremental compilation for faster rebuilds.
//! It tracks source file changes, dependencies, and caches compilation artifacts.
//!
//! # Overview
//!
//! Incremental compilation works by:
//! 1. **Hashing sources** — Detect which files have changed
//! 2. **Tracking dependencies** — Know what needs recompilation when a file changes
//! 3. **Caching artifacts** — Reuse previously compiled objects
//!
//! # Cache Directory Structure
//!
//! ```text
//! build/
//! └── cache/
//!     ├── hashes.json          # Source file content hashes
//!     ├── deps/                # Dependency graphs
//!     │   ├── module_a.json
//!     │   └── module_b.json
//!     ├── objects/             # Cached object files
//!     │   ├── <hash>.o
//!     │   └── <hash>.meta
//!     └── version              # Compiler version for cache invalidation
//! ```

pub mod arc_cache;
pub mod cache;
pub mod deps;
pub mod function_hash;
pub mod hash;

// Re-export key types
pub use arc_cache::ArcIrCache;
pub use cache::{ArtifactCache, CacheConfig, CacheKey};
pub use deps::{DependencyGraph, DependencyTracker};
pub use function_hash::{
    compute_module_hash, extract_function_hashes, extract_function_hashes_with_canon,
    FunctionContentHash,
};
pub use hash::{ContentHash, SourceHasher};
