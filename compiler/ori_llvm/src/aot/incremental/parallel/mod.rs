//! Parallel Compilation Support
//!
//! Provides parallel compilation of independent modules for faster builds.

mod executor;

#[expect(
    deprecated,
    reason = "re-exported for backward compatibility; tests still use it"
)]
pub use executor::compile_parallel;
pub use executor::{execute_parallel, ParallelCompiler};

use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::thread;

use rustc_hash::FxHashMap;

use super::deps::DependencyGraph;
use super::hash::ContentHash;

/// A work item to be compiled.
#[derive(Debug, Clone)]
pub struct WorkItem {
    /// Path to the source file.
    pub path: PathBuf,
    /// Content hash of the source.
    pub hash: ContentHash,
    /// Dependencies that must be compiled first.
    pub dependencies: Vec<PathBuf>,
    /// Priority (lower = higher priority).
    pub priority: usize,
}

impl WorkItem {
    /// Create a new work item.
    #[must_use]
    pub fn new(path: PathBuf, hash: ContentHash) -> Self {
        Self {
            path,
            hash,
            dependencies: Vec::new(),
            priority: 0,
        }
    }

    /// Set dependencies.
    #[must_use]
    pub fn with_dependencies(mut self, deps: Vec<PathBuf>) -> Self {
        self.dependencies = deps;
        self
    }

    /// Set priority.
    #[must_use]
    pub fn with_priority(mut self, priority: usize) -> Self {
        self.priority = priority;
        self
    }
}

/// A compilation plan describing what to compile and in what order.
#[derive(Debug, Default)]
pub struct CompilationPlan {
    /// Work items to compile.
    items: Vec<WorkItem>,
    /// Items that are ready (all deps satisfied).
    ready: VecDeque<usize>,
    /// Items waiting for dependencies.
    pending: HashSet<usize>,
    /// Completed items.
    completed: HashSet<PathBuf>,
    /// Items that failed compilation (used for failure cascade).
    failed_items: HashSet<usize>,
    /// Reverse index: dep path -> items that depend on it (for O(1) lookup on completion).
    dependents: FxHashMap<PathBuf, Vec<usize>>,
    /// Count of unsatisfied dependencies per item.
    unsatisfied_deps: Vec<usize>,
    /// Path-to-index mapping for O(1) failure marking.
    path_to_index: FxHashMap<PathBuf, usize>,
}

impl CompilationPlan {
    /// Create a new empty compilation plan.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a compilation plan from a dependency graph.
    #[must_use]
    pub fn from_graph(graph: &DependencyGraph, files: &[PathBuf]) -> Self {
        use std::collections::HashSet;

        let mut plan = Self::new();

        // Get topological order for proper scheduling
        let order = graph.topological_order().unwrap_or_default();

        // Pre-build HashSet for O(1) lookup instead of O(n) Vec::contains
        let files_set: HashSet<&PathBuf> = files.iter().collect();

        // Create work items
        for path in files {
            if let Some(hash) = graph.get_hash(path) {
                let deps: Vec<PathBuf> = graph
                    .get_imports(path)
                    .map(<[PathBuf]>::to_vec)
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|d| files_set.contains(&d))
                    .collect();

                // Priority based on position in topological order
                let priority = order.iter().position(|p| p == path).unwrap_or(0);

                let item = WorkItem::new(path.clone(), hash)
                    .with_dependencies(deps)
                    .with_priority(priority);

                plan.add_item(item);
            }
        }

        plan
    }

    /// Add a work item to the plan.
    pub fn add_item(&mut self, item: WorkItem) {
        let idx = self.items.len();
        let dep_count = item.dependencies.len();

        // Build path-to-index mapping for O(1) failure marking
        self.path_to_index.insert(item.path.clone(), idx);

        // Build reverse index: for each dependency, record that this item depends on it
        for dep in &item.dependencies {
            self.dependents.entry(dep.clone()).or_default().push(idx);
        }

        // Track unsatisfied dependency count
        self.unsatisfied_deps.push(dep_count);

        if dep_count == 0 {
            self.ready.push_back(idx);
        } else {
            self.pending.insert(idx);
        }

        self.items.push(item);
    }

    /// Get the next ready item.
    pub fn take_next(&mut self) -> Option<&WorkItem> {
        self.ready.pop_front().map(|idx| &self.items[idx])
    }

    /// Mark an item as completed.
    ///
    /// Uses O(dependents) lookup instead of O(pending * deps) iteration.
    pub fn complete(&mut self, path: &Path) {
        self.completed.insert(path.to_path_buf());

        // Only check items that directly depend on the completed path (O(1) lookup + O(dependents))
        if let Some(dependent_indices) = self.dependents.get(path) {
            for &idx in dependent_indices {
                // Decrement unsatisfied count
                if self.unsatisfied_deps[idx] > 0 {
                    self.unsatisfied_deps[idx] -= 1;

                    // If all deps satisfied, move from pending to ready
                    if self.unsatisfied_deps[idx] == 0 && self.pending.remove(&idx) {
                        self.ready.push_back(idx);
                    }
                }
            }
        }
    }

    /// Check if the plan is complete.
    ///
    /// A plan is complete when there are no more ready or pending items.
    /// Items may still be in `failed_items` — the plan is "done" even if
    /// some items failed (their dependents were cascade-failed).
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.ready.is_empty() && self.pending.is_empty()
    }

    /// Mark an item as failed and cascade the failure to all dependents.
    ///
    /// Removes the item and all transitive dependents from pending/ready,
    /// preventing wasted compilation of items that can't succeed.
    pub fn mark_failed(&mut self, path: &Path) {
        if let Some(&idx) = self.path_to_index.get(path) {
            self.failed_items.insert(idx);
            self.pending.remove(&idx);
            // Remove from ready queue if present
            self.ready.retain(|&i| i != idx);
        }

        // Cascade to all transitive dependents
        let dependents = self.transitive_dependents(path);
        for dep_path in &dependents {
            if let Some(&dep_idx) = self.path_to_index.get(dep_path) {
                self.failed_items.insert(dep_idx);
                self.pending.remove(&dep_idx);
                self.ready.retain(|&i| i != dep_idx);
            }
        }
    }

    /// Compute all transitive dependents of a path via BFS.
    ///
    /// Returns all items that directly or indirectly depend on the given path.
    /// Used for failure cascade: if A fails, everything that depends on A
    /// (and everything that depends on those, etc.) is also marked failed.
    #[must_use]
    pub fn transitive_dependents(&self, path: &Path) -> Vec<PathBuf> {
        let mut result = Vec::new();
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(path.to_path_buf());
        visited.insert(path.to_path_buf());

        while let Some(current) = queue.pop_front() {
            if let Some(dep_indices) = self.dependents.get(&current) {
                for &idx in dep_indices {
                    let dep_path = &self.items[idx].path;
                    if visited.insert(dep_path.clone()) {
                        result.push(dep_path.clone());
                        queue.push_back(dep_path.clone());
                    }
                }
            }
        }

        result
    }

    /// Check if an item has been marked as failed.
    #[must_use]
    pub fn is_failed(&self, path: &Path) -> bool {
        self.path_to_index
            .get(path)
            .is_some_and(|idx| self.failed_items.contains(idx))
    }

    /// Get the number of failed items.
    #[must_use]
    pub fn failed_count(&self) -> usize {
        self.failed_items.len()
    }

    /// Get the total number of items.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Check if the plan is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Get the number of completed items.
    #[must_use]
    pub fn completed_count(&self) -> usize {
        self.completed.len()
    }

    /// Get all work items.
    #[must_use]
    pub fn items(&self) -> &[WorkItem] {
        &self.items
    }
}

/// Configuration for parallel compilation.
#[derive(Debug, Clone, Default)]
pub struct ParallelConfig {
    /// Number of worker threads (0 = auto-detect).
    pub jobs: usize,
    /// Whether to show progress.
    pub show_progress: bool,
}

impl ParallelConfig {
    /// Create a new configuration with the given job count.
    #[must_use]
    pub fn new(jobs: usize) -> Self {
        Self {
            jobs,
            show_progress: false,
        }
    }

    /// Auto-detect the number of CPUs.
    #[must_use]
    pub fn auto() -> Self {
        Self {
            jobs: 0,
            show_progress: false,
        }
    }

    /// Enable progress reporting.
    #[must_use]
    pub fn with_progress(mut self, show: bool) -> Self {
        self.show_progress = show;
        self
    }

    /// Get the effective number of jobs.
    #[must_use]
    pub fn effective_jobs(&self) -> usize {
        if self.jobs == 0 {
            // Auto-detect
            thread::available_parallelism()
                .map(std::num::NonZero::get)
                .unwrap_or(1)
        } else {
            self.jobs
        }
    }
}

/// Result of compiling a single item.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CompileResult {
    /// Path to the source file.
    pub path: PathBuf,
    /// Path to the compiled object file.
    pub output: PathBuf,
    /// Whether compilation was from cache.
    pub cached: bool,
    /// Compilation time in milliseconds.
    pub time_ms: u64,
}

/// Error during parallel compilation.
#[derive(Debug, Clone)]
pub struct CompileError {
    /// Path to the source file.
    pub path: PathBuf,
    /// Error message.
    pub message: String,
}

impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "compilation of '{}' failed: {}",
            self.path.display(),
            self.message
        )
    }
}

impl std::error::Error for CompileError {}

/// Statistics from parallel compilation.
#[derive(Debug, Default, Clone)]
pub struct CompilationStats {
    /// Total items compiled.
    pub total: usize,
    /// Items from cache.
    pub cached: usize,
    /// Items compiled fresh.
    pub compiled: usize,
    /// Total time in milliseconds.
    pub total_time_ms: u64,
}

#[cfg(test)]
#[allow(
    clippy::disallowed_types,
    clippy::redundant_closure_for_method_calls,
    clippy::items_after_statements,
    reason = "test code — Arc needed for cross-thread sharing, closures for readability, inline imports for locality"
)]
mod tests;
