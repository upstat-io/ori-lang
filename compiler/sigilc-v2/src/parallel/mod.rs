//! Parallel compilation infrastructure for Sigil V2.
//!
//! This module provides parallelism at multiple levels:
//!
//! - **File-level**: Parse multiple files concurrently using Rayon
//! - **Module-level**: Type check independent modules in parallel
//! - **Function-level**: Generate code for functions concurrently
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │                    Parallel Pipeline                     │
//! ├─────────────────────────────────────────────────────────┤
//! │  Files ──► Parallel Lex/Parse ──► Parsed Modules        │
//! │                                                          │
//! │  Modules ──► Dependency Graph ──► Topological Order     │
//! │                                                          │
//! │  Levels ──► Work-Stealing Pool ──► Type Checked         │
//! │                                                          │
//! │  Functions ──► Parallel Codegen ──► Output Files        │
//! └─────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Performance Targets
//!
//! - Near-linear speedup to 8 cores
//! - <50ms incremental rebuild for single file changes
//! - Efficient work distribution via work-stealing

mod file_parser;
mod dependency;
mod work_pool;
mod codegen;

pub use file_parser::{ParallelParser, ParsedFile, ParserConfig};
pub use dependency::{DependencyGraph, ModuleNode, DependencyLevel};
pub use work_pool::{WorkPool, WorkItem, WorkResult};
pub use codegen::{ParallelCodegen, CodegenTask, CodegenResult};

use rayon::prelude::*;
use std::path::Path;

/// Configuration for parallel compilation.
#[derive(Clone, Debug)]
pub struct ParallelConfig {
    /// Number of threads to use (0 = auto-detect).
    pub num_threads: usize,
    /// Minimum file size in bytes for parallel lexing.
    pub parallel_lex_threshold: usize,
    /// Chunk size for parallel lexing.
    pub lex_chunk_size: usize,
    /// Whether to use work-stealing for type checking.
    pub work_stealing: bool,
}

impl Default for ParallelConfig {
    fn default() -> Self {
        ParallelConfig {
            num_threads: 0, // auto-detect
            parallel_lex_threshold: 100 * 1024, // 100KB
            lex_chunk_size: 16 * 1024, // 16KB chunks
            work_stealing: true,
        }
    }
}

impl ParallelConfig {
    /// Create a config optimized for single-threaded execution.
    pub fn single_threaded() -> Self {
        ParallelConfig {
            num_threads: 1,
            parallel_lex_threshold: usize::MAX,
            lex_chunk_size: usize::MAX,
            work_stealing: false,
        }
    }

    /// Create a config with specified thread count.
    pub fn with_threads(num_threads: usize) -> Self {
        ParallelConfig {
            num_threads,
            ..Default::default()
        }
    }

    /// Get the effective number of threads.
    pub fn effective_threads(&self) -> usize {
        if self.num_threads == 0 {
            rayon::current_num_threads()
        } else {
            self.num_threads
        }
    }
}

/// Statistics from parallel compilation.
#[derive(Clone, Debug, Default)]
pub struct ParallelStats {
    /// Number of files parsed.
    pub files_parsed: usize,
    /// Number of files parsed in parallel.
    pub files_parallel: usize,
    /// Number of modules type checked.
    pub modules_checked: usize,
    /// Number of dependency levels.
    pub dependency_levels: usize,
    /// Number of functions generated.
    pub functions_generated: usize,
    /// Total parse time in microseconds.
    pub parse_time_us: u64,
    /// Total type check time in microseconds.
    pub typecheck_time_us: u64,
    /// Total codegen time in microseconds.
    pub codegen_time_us: u64,
}

impl ParallelStats {
    /// Merge another stats into this one.
    pub fn merge(&mut self, other: &ParallelStats) {
        self.files_parsed += other.files_parsed;
        self.files_parallel += other.files_parallel;
        self.modules_checked += other.modules_checked;
        self.dependency_levels = self.dependency_levels.max(other.dependency_levels);
        self.functions_generated += other.functions_generated;
        self.parse_time_us += other.parse_time_us;
        self.typecheck_time_us += other.typecheck_time_us;
        self.codegen_time_us += other.codegen_time_us;
    }

    /// Calculate the speedup factor.
    pub fn speedup(&self, sequential_time_us: u64) -> f64 {
        let parallel_time = self.parse_time_us + self.typecheck_time_us + self.codegen_time_us;
        if parallel_time == 0 {
            1.0
        } else {
            sequential_time_us as f64 / parallel_time as f64
        }
    }
}

/// Coordinator for parallel compilation.
pub struct ParallelCompiler {
    config: ParallelConfig,
    stats: ParallelStats,
}

impl ParallelCompiler {
    /// Create a new parallel compiler with default configuration.
    pub fn new() -> Self {
        ParallelCompiler {
            config: ParallelConfig::default(),
            stats: ParallelStats::default(),
        }
    }

    /// Create a parallel compiler with custom configuration.
    pub fn with_config(config: ParallelConfig) -> Self {
        ParallelCompiler {
            config,
            stats: ParallelStats::default(),
        }
    }

    /// Get the configuration.
    pub fn config(&self) -> &ParallelConfig {
        &self.config
    }

    /// Get the statistics.
    pub fn stats(&self) -> &ParallelStats {
        &self.stats
    }

    /// Reset statistics.
    pub fn reset_stats(&mut self) {
        self.stats = ParallelStats::default();
    }

    /// Parse multiple files in parallel.
    pub fn parse_files<P: AsRef<Path> + Sync>(
        &mut self,
        files: &[P],
        interner: &crate::intern::StringInterner,
    ) -> Vec<ParsedFile> {
        let start = std::time::Instant::now();
        let parser = ParallelParser::new(interner, self.config.clone());

        let results = parser.parse_files(files);

        self.stats.files_parsed = results.len();
        self.stats.files_parallel = if results.len() > 1 { results.len() } else { 0 };
        self.stats.parse_time_us = start.elapsed().as_micros() as u64;

        results
    }

    /// Build dependency graph from parsed modules.
    pub fn build_dependency_graph(&self, modules: &[ParsedFile]) -> DependencyGraph {
        DependencyGraph::from_modules(modules)
    }
}

impl Default for ParallelCompiler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parallel_config_default() {
        let config = ParallelConfig::default();
        assert_eq!(config.num_threads, 0);
        assert_eq!(config.parallel_lex_threshold, 100 * 1024);
        assert!(config.work_stealing);
    }

    #[test]
    fn test_parallel_config_single_threaded() {
        let config = ParallelConfig::single_threaded();
        assert_eq!(config.num_threads, 1);
        assert!(!config.work_stealing);
    }

    #[test]
    fn test_parallel_stats_merge() {
        let mut stats1 = ParallelStats {
            files_parsed: 5,
            files_parallel: 4,
            modules_checked: 10,
            dependency_levels: 3,
            functions_generated: 50,
            parse_time_us: 1000,
            typecheck_time_us: 2000,
            codegen_time_us: 500,
        };

        let stats2 = ParallelStats {
            files_parsed: 3,
            files_parallel: 2,
            modules_checked: 5,
            dependency_levels: 4,
            functions_generated: 20,
            parse_time_us: 500,
            typecheck_time_us: 1000,
            codegen_time_us: 250,
        };

        stats1.merge(&stats2);
        assert_eq!(stats1.files_parsed, 8);
        assert_eq!(stats1.dependency_levels, 4); // max
        assert_eq!(stats1.parse_time_us, 1500);
    }

    #[test]
    fn test_parallel_compiler_creation() {
        let compiler = ParallelCompiler::new();
        assert_eq!(compiler.stats().files_parsed, 0);
    }
}
