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
//! 4. **Parallel compilation** — Compile independent modules concurrently
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
//!
//! # Example
//!
//! ```ignore
//! use ori_llvm::aot::incremental::{IncrementalBuilder, BuildConfig};
//!
//! let config = BuildConfig::new("build/cache")
//!     .with_jobs(4)
//!     .with_optimization_level(OptimizationLevel::Release);
//!
//! let builder = IncrementalBuilder::new(config)?;
//!
//! // Check what needs recompilation
//! let plan = builder.plan_build(&["src/main.ori", "src/lib.ori"])?;
//!
//! // Execute the build
//! let result = builder.execute(plan)?;
//! ```

pub mod arc_cache;
pub mod cache;
pub mod deps;
pub mod function_deps;
pub mod function_hash;
pub mod hash;
pub mod parallel;

// Re-export key types
pub use arc_cache::ArcIrCache;
pub use cache::{ArtifactCache, CacheConfig, CacheKey};
pub use deps::{DependencyGraph, DependencyTracker};
pub use function_deps::FunctionDependencyGraph;
pub use function_hash::{compute_module_hash, extract_function_hashes, FunctionContentHash};
pub use hash::{ContentHash, SourceHasher};
pub use parallel::{
    execute_parallel, CompilationPlan, CompilationStats, CompileError, CompileResult,
    ParallelCompiler, WorkItem,
};
