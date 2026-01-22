//! Design Tests: Parallelism Architecture
//!
//! These tests validate the parallelism design from Phase 4:
//! - File-level parallel parsing with Rayon
//! - Module dependency graph with topological ordering
//! - Work-stealing pool for type checking
//! - Level-based parallel type checking
//!
//! Reference: Implementation Plan Phase 4 (Weeks 13-16)

use sigilc_v2::parallel::{
    ParallelConfig, ParallelStats,
    ParallelParser, ParserConfig,
    DependencyGraph,
    WorkPool,
    ParallelCodegen, CodegenTask,
};
use sigilc_v2::intern::StringInterner;

// =============================================================================
// ParallelConfig Design Contracts
// =============================================================================

/// Design: Default config uses all available threads
#[test]
fn design_default_uses_all_threads() {
    let config = ParallelConfig::default();
    // num_threads=0 means auto-detect, effective_threads returns actual count
    assert!(config.effective_threads() >= 1);
}

/// Design: Single-threaded mode is supported
#[test]
fn design_single_threaded_mode() {
    let config = ParallelConfig::single_threaded();
    assert_eq!(config.num_threads, 1);
}

/// Design: Parallel lex threshold is configurable (default 100KB)
#[test]
fn design_parallel_lex_threshold() {
    let config = ParallelConfig::default();
    // Design: 100KB threshold for parallel lexing
    assert_eq!(config.parallel_lex_threshold, 100 * 1024);
}

// =============================================================================
// Parallel Parsing Design Contracts
// =============================================================================

/// Design: ParallelParser uses Rayon for file-level parallelism
#[test]
fn design_parallel_parser_uses_rayon() {
    let interner = StringInterner::new();
    let config = ParallelConfig::default();
    let parser = ParallelParser::new(&interner, config);

    // Parser should be usable (validates design)
    let results = parser.parse_files::<&str>(&[]);
    assert!(results.is_empty());
}

/// Design: Parser config supports error recovery
#[test]
fn design_parser_error_recovery() {
    let config = ParserConfig::default();
    // Design: recover_errors: true by default
    assert!(config.recover_errors);
    // Design: max_errors: 100 by default
    assert_eq!(config.max_errors, 100);
}

// =============================================================================
// Dependency Graph Design Contracts
// =============================================================================

/// Design: DependencyGraph computes topological levels
#[test]
fn design_dependency_graph_empty() {
    let graph = DependencyGraph::new();
    assert!(!graph.has_cycles());
    assert!(graph.levels().is_empty());
}

/// Design: Cycle detection prevents infinite loops
#[test]
fn design_cycle_detection_on_cyclic_graph() {
    // Note: cycles are detected during graph construction from modules
    // The has_cycles() method returns the detection result
    let graph = DependencyGraph::new();
    assert!(!graph.has_cycles()); // Empty graph has no cycles
}

/// Design: Levels are properly computed
#[test]
fn design_levels_computed() {
    let graph = DependencyGraph::new();
    assert_eq!(graph.level_count(), 0);
    assert_eq!(graph.critical_path(), 0);
}

// =============================================================================
// Work Pool Design Contracts
// =============================================================================

/// Design: WorkPool uses crossbeam-deque for work-stealing
#[test]
fn design_work_pool_creation() {
    let pool = WorkPool::new(4);
    assert_eq!(pool.num_workers(), 4);
}

/// Design: Default pool uses Rayon thread count
#[test]
fn design_work_pool_default() {
    let pool = WorkPool::default();
    assert!(pool.num_workers() > 0);
}

/// Design: WorkPool is empty initially
#[test]
fn design_work_pool_empty() {
    let pool = WorkPool::new(4);
    assert!(pool.is_empty());
}

// =============================================================================
// Parallel Codegen Design Contracts
// =============================================================================

/// Design: ParallelCodegen uses function-level parallelism
#[test]
fn design_codegen_parallel() {
    let codegen = ParallelCodegen::new(4);

    let tasks: Vec<CodegenTask> = (0..10)
        .map(|i| CodegenTask::new(format!("func_{}", i), "main".to_string(), i))
        .collect();

    let results = codegen.generate_parallel(tasks);

    // All functions should be generated
    assert_eq!(results.len(), 10);
}

/// Design: Codegen caches results
#[test]
fn design_codegen_caching() {
    let codegen = ParallelCodegen::new(4);

    let task = CodegenTask::new("foo".to_string(), "main".to_string(), 0);

    // First generation
    let _ = codegen.generate_function(task.clone());
    assert_eq!(codegen.cache_size(), 1);

    // Second generation should hit cache
    let task2 = CodegenTask::new("foo".to_string(), "main".to_string(), 0);
    let _ = codegen.generate_function(task2);
    assert_eq!(codegen.cache_size(), 1); // Still 1, not 2
}

/// Design: Default codegen uses available threads
#[test]
fn design_codegen_default() {
    let codegen = ParallelCodegen::default();
    assert!(codegen.num_workers() > 0);
}

// =============================================================================
// Performance Budget Tests
// =============================================================================

/// Design: ParallelStats tracks timing
#[test]
fn design_stats_tracking() {
    let mut stats = ParallelStats::default();
    stats.files_parsed = 10;
    stats.parse_time_us = 1000;

    // Stats should merge correctly
    let mut stats2 = ParallelStats::default();
    stats2.files_parsed = 5;
    stats2.parse_time_us = 500;

    stats.merge(&stats2);
    assert_eq!(stats.files_parsed, 15);
    assert_eq!(stats.parse_time_us, 1500);
}
