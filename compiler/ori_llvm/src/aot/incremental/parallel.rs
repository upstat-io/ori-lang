//! Parallel Compilation Support
//!
//! Provides parallel compilation of independent modules for faster builds.

use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};

use rustc_hash::FxHashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
#[expect(
    clippy::disallowed_types,
    reason = "Arc required for thread-safe sharing"
)]
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::thread;

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

/// Parallel compiler coordinator.
///
/// Coordinates parallel compilation of multiple source files.
#[expect(
    clippy::disallowed_types,
    reason = "Arc required for thread-safe progress tracking"
)]
pub struct ParallelCompiler {
    /// Configuration.
    config: ParallelConfig,
    /// Current progress (for reporting).
    progress: Arc<AtomicUsize>,
}

impl ParallelCompiler {
    /// Create a new parallel compiler.
    #[must_use]
    #[expect(
        clippy::disallowed_types,
        reason = "Arc required for thread-safe progress tracking"
    )]
    pub fn new(config: ParallelConfig) -> Self {
        Self {
            config,
            progress: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Get the number of worker threads.
    #[must_use]
    pub fn jobs(&self) -> usize {
        self.config.effective_jobs()
    }

    /// Execute a compilation plan.
    ///
    /// This is a placeholder that returns a plan for the items.
    /// Actual compilation would be done by a callback.
    pub fn execute<F>(
        &self,
        mut plan: CompilationPlan,
        mut compile_fn: F,
    ) -> Result<CompilationStats, Vec<CompileError>>
    where
        F: FnMut(&WorkItem) -> Result<CompileResult, CompileError>,
    {
        let mut stats = CompilationStats::default();
        let mut errors = Vec::new();

        // For single-threaded execution (simpler, avoid complex threading)
        while let Some(item) = plan.take_next() {
            let item = item.clone();

            match compile_fn(&item) {
                Ok(result) => {
                    stats.total += 1;
                    stats.total_time_ms += result.time_ms;
                    if result.cached {
                        stats.cached += 1;
                    } else {
                        stats.compiled += 1;
                    }
                    plan.complete(&item.path);
                    self.progress.fetch_add(1, Ordering::Relaxed);
                }
                Err(e) => {
                    errors.push(e);
                    // Don't mark as complete - dependents can't proceed
                }
            }
        }

        if errors.is_empty() {
            Ok(stats)
        } else {
            Err(errors)
        }
    }

    /// Get current progress count.
    #[must_use]
    pub fn progress(&self) -> usize {
        self.progress.load(Ordering::Relaxed)
    }

    /// Reset progress counter.
    pub fn reset_progress(&self) {
        self.progress.store(0, Ordering::Relaxed);
    }
}

/// Shared state for the dependency-aware parallel executor.
///
/// Protected by a `Mutex` and coordinated via `Condvar` for blocking
/// when no work is available.
struct SharedPlanState {
    plan: Mutex<CompilationPlan>,
    condvar: Condvar,
}

/// Execute a compilation plan in parallel with dependency tracking.
///
/// Unlike [`compile_parallel`] (which ignores dependencies and round-robins),
/// this function respects the dependency graph:
/// - Workers block on `Condvar` when no work is ready
/// - Completing a module may unblock dependent modules
/// - Failure cascade: if a module fails, all transitive dependents are skipped
///
/// `jobs` specifies the number of worker threads (0 = auto-detect).
/// `compile_fn` receives a `&WorkItem` and returns `Result<CompileResult, CompileError>`.
///
/// Returns `CompilationStats` on success, or a list of errors on failure.
#[expect(
    clippy::disallowed_types,
    reason = "Arc required for thread-safe sharing across worker threads"
)]
pub fn execute_parallel<F>(
    plan: CompilationPlan,
    jobs: usize,
    compile_fn: F,
) -> Result<CompilationStats, Vec<CompileError>>
where
    F: Fn(&WorkItem) -> Result<CompileResult, CompileError> + Send + Sync + 'static,
{
    let effective_jobs = if jobs == 0 {
        thread::available_parallelism()
            .map(std::num::NonZero::get)
            .unwrap_or(1)
    } else {
        jobs
    };

    // Single-thread fallback: simpler, avoid threading overhead
    if effective_jobs == 1 || plan.len() <= 1 {
        return execute_sequential(plan, &compile_fn);
    }

    let state = Arc::new(SharedPlanState {
        plan: Mutex::new(plan),
        condvar: Condvar::new(),
    });

    let compile_fn = Arc::new(compile_fn);
    let comp_stats = Arc::new(Mutex::new(CompilationStats::default()));
    let errors = Arc::new(Mutex::new(Vec::<CompileError>::new()));

    let mut handles = Vec::with_capacity(effective_jobs);

    for _ in 0..effective_jobs {
        let state = Arc::clone(&state);
        let compile_fn = Arc::clone(&compile_fn);
        let comp_stats = Arc::clone(&comp_stats);
        let errors = Arc::clone(&errors);

        let handle = thread::spawn(move || {
            loop {
                // Take next ready item under the lock
                let item = {
                    let mut plan = state.plan.lock().unwrap_or_else(PoisonError::into_inner);

                    loop {
                        // Try to take a ready item
                        if let Some(item) = plan.take_next() {
                            break Some(item.clone());
                        }

                        // No ready items — are we done?
                        if plan.is_complete() {
                            break None;
                        }

                        // Wait for a signal (item completed or failed)
                        plan = state
                            .condvar
                            .wait(plan)
                            .unwrap_or_else(PoisonError::into_inner);
                    }
                };

                let Some(item) = item else {
                    // Plan is complete — exit worker loop
                    break;
                };

                // Compile outside the lock (the expensive part)
                match compile_fn(&item) {
                    Ok(result) => {
                        let mut s = comp_stats.lock().unwrap_or_else(PoisonError::into_inner);
                        s.total += 1;
                        s.total_time_ms += result.time_ms;
                        if result.cached {
                            s.cached += 1;
                        } else {
                            s.compiled += 1;
                        }
                        drop(s);

                        // Mark complete and wake others
                        let mut plan = state.plan.lock().unwrap_or_else(PoisonError::into_inner);
                        plan.complete(&item.path);
                        state.condvar.notify_all();
                    }
                    Err(e) => {
                        errors
                            .lock()
                            .unwrap_or_else(PoisonError::into_inner)
                            .push(e);

                        // Mark failed and cascade
                        let mut plan = state.plan.lock().unwrap_or_else(PoisonError::into_inner);
                        plan.mark_failed(&item.path);
                        state.condvar.notify_all();
                    }
                }
            }
        });

        handles.push(handle);
    }

    // Wait for all workers
    for handle in handles {
        handle.join().unwrap_or_else(|_| {
            // Thread panicked — add an error
            errors
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(CompileError {
                    path: PathBuf::from("<worker>"),
                    message: "worker thread panicked".to_string(),
                });
        });
    }

    let errors = match Arc::try_unwrap(errors) {
        Ok(mutex) => mutex.into_inner().unwrap_or_default(),
        Err(arc) => arc.lock().unwrap_or_else(PoisonError::into_inner).clone(),
    };

    if errors.is_empty() {
        let comp_stats = match Arc::try_unwrap(comp_stats) {
            Ok(mutex) => mutex.into_inner().unwrap_or_default(),
            Err(arc) => arc.lock().unwrap_or_else(PoisonError::into_inner).clone(),
        };
        Ok(comp_stats)
    } else {
        Err(errors)
    }
}

/// Sequential execution fallback for single-threaded or small plans.
fn execute_sequential<F>(
    mut plan: CompilationPlan,
    compile_fn: &F,
) -> Result<CompilationStats, Vec<CompileError>>
where
    F: Fn(&WorkItem) -> Result<CompileResult, CompileError>,
{
    let mut stats = CompilationStats::default();
    let mut errors = Vec::new();

    while let Some(item) = plan.take_next() {
        let item = item.clone();

        match compile_fn(&item) {
            Ok(result) => {
                stats.total += 1;
                stats.total_time_ms += result.time_ms;
                if result.cached {
                    stats.cached += 1;
                } else {
                    stats.compiled += 1;
                }
                plan.complete(&item.path);
            }
            Err(e) => {
                errors.push(e);
                plan.mark_failed(&item.path);
            }
        }
    }

    if errors.is_empty() {
        Ok(stats)
    } else {
        Err(errors)
    }
}

/// Execute compilation in parallel using multiple threads.
///
/// **Deprecated**: Use [`execute_parallel`] instead, which respects dependency
/// ordering and provides failure cascade. This function ignores dependencies
/// and simply round-robins work items across threads.
#[deprecated(note = "use execute_parallel() which respects dependency ordering")]
#[expect(
    clippy::disallowed_types,
    reason = "Arc required for thread-safe sharing across worker threads"
)]
pub fn compile_parallel<F, R>(
    plan: &CompilationPlan,
    jobs: usize,
    compile_fn: F,
) -> Result<Vec<R>, Vec<CompileError>>
where
    F: Fn(&WorkItem) -> Result<R, CompileError> + Send + Sync + 'static,
    R: Send + std::fmt::Debug + 'static,
{
    let jobs = if jobs == 0 {
        thread::available_parallelism()
            .map(std::num::NonZero::get)
            .unwrap_or(1)
    } else {
        jobs
    };

    // For small plans, just run sequentially
    if plan.len() <= jobs || jobs == 1 {
        let mut results = Vec::new();
        let mut errors = Vec::new();

        for item in plan.items() {
            match compile_fn(item) {
                Ok(r) => results.push(r),
                Err(e) => errors.push(e),
            }
        }

        if errors.is_empty() {
            Ok(results)
        } else {
            Err(errors)
        }
    } else {
        // Use a thread pool for larger plans
        let items = Arc::new(plan.items().to_vec());
        let results = Arc::new(Mutex::new(Vec::new()));
        let errors = Arc::new(Mutex::new(Vec::new()));
        let next_idx = Arc::new(AtomicUsize::new(0));
        let compile_fn = Arc::new(compile_fn);

        let mut handles = Vec::new();

        for _ in 0..jobs {
            let items = Arc::clone(&items);
            let results = Arc::clone(&results);
            let errors = Arc::clone(&errors);
            let next_idx = Arc::clone(&next_idx);
            let compile_fn = Arc::clone(&compile_fn);

            let handle = thread::spawn(move || loop {
                let idx = next_idx.fetch_add(1, Ordering::SeqCst);
                if idx >= items.len() {
                    break;
                }

                match compile_fn(&items[idx]) {
                    Ok(r) => {
                        results.lock().unwrap().push(r);
                    }
                    Err(e) => {
                        errors.lock().unwrap().push(e);
                    }
                }
            });

            handles.push(handle);
        }

        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }

        let results = Arc::try_unwrap(results).unwrap().into_inner().unwrap();
        let errors = Arc::try_unwrap(errors).unwrap().into_inner().unwrap();

        if errors.is_empty() {
            Ok(results)
        } else {
            Err(errors)
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::disallowed_types,
    clippy::redundant_closure_for_method_calls,
    clippy::items_after_statements,
    reason = "test code — Arc needed for cross-thread sharing, closures for readability, inline imports for locality"
)]
mod tests {
    use super::*;

    fn h(n: u64) -> ContentHash {
        ContentHash::new(n)
    }

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    #[test]
    fn test_work_item() {
        let item = WorkItem::new(p("main.ori"), h(123))
            .with_dependencies(vec![p("lib.ori")])
            .with_priority(1);

        assert_eq!(item.path, p("main.ori"));
        assert_eq!(item.hash.value(), 123);
        assert_eq!(item.dependencies.len(), 1);
        assert_eq!(item.priority, 1);
    }

    #[test]
    fn test_compilation_plan_empty() {
        let plan = CompilationPlan::new();
        assert!(plan.is_empty());
        assert!(plan.is_complete());
    }

    #[test]
    fn test_compilation_plan_single() {
        let mut plan = CompilationPlan::new();
        plan.add_item(WorkItem::new(p("main.ori"), h(1)));

        assert_eq!(plan.len(), 1);
        assert!(!plan.is_complete());

        let item = plan.take_next().unwrap();
        assert_eq!(item.path, p("main.ori"));

        plan.complete(&p("main.ori"));
        assert!(plan.is_complete());
    }

    #[test]
    fn test_compilation_plan_with_deps() {
        let mut plan = CompilationPlan::new();

        // Add items with dependencies
        plan.add_item(WorkItem::new(p("main.ori"), h(1)).with_dependencies(vec![p("lib.ori")]));
        plan.add_item(WorkItem::new(p("lib.ori"), h(2)));

        // lib.ori should be ready first (no deps)
        let item = plan.take_next().unwrap();
        assert_eq!(item.path, p("lib.ori"));
        plan.complete(&p("lib.ori"));

        // Now main.ori should be ready
        let item = plan.take_next().unwrap();
        assert_eq!(item.path, p("main.ori"));
        plan.complete(&p("main.ori"));

        assert!(plan.is_complete());
    }

    #[test]
    fn test_parallel_config_auto() {
        let config = ParallelConfig::auto();
        assert!(config.effective_jobs() >= 1);
    }

    #[test]
    fn test_parallel_config_explicit() {
        let config = ParallelConfig::new(4);
        assert_eq!(config.effective_jobs(), 4);
    }

    #[test]
    fn test_parallel_compiler_execute() {
        let mut plan = CompilationPlan::new();
        plan.add_item(WorkItem::new(p("a.ori"), h(1)));
        plan.add_item(WorkItem::new(p("b.ori"), h(2)));

        let compiler = ParallelCompiler::new(ParallelConfig::new(1));

        let stats = compiler
            .execute(plan, |item| {
                Ok(CompileResult {
                    path: item.path.clone(),
                    output: PathBuf::from(format!("{}.o", item.path.display())),
                    cached: false,
                    time_ms: 10,
                })
            })
            .unwrap();

        assert_eq!(stats.total, 2);
        assert_eq!(stats.compiled, 2);
        assert_eq!(stats.cached, 0);
    }

    #[test]
    fn test_parallel_compiler_with_error() {
        let mut plan = CompilationPlan::new();
        plan.add_item(WorkItem::new(p("bad.ori"), h(1)));

        let compiler = ParallelCompiler::new(ParallelConfig::new(1));

        let result = compiler.execute(plan, |item| {
            Err(CompileError {
                path: item.path.clone(),
                message: "syntax error".to_string(),
            })
        });

        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("syntax error"));
    }

    #[test]
    #[allow(
        deprecated,
        reason = "tests compile_parallel which is deprecated in favor of execute_parallel"
    )]
    fn test_compile_parallel_single() {
        let mut plan = CompilationPlan::new();
        plan.add_item(WorkItem::new(p("test.ori"), h(1)));

        let results: Vec<String> = compile_parallel(&plan, 1, |item| {
            Ok(format!("compiled: {}", item.path.display()))
        })
        .unwrap();

        assert_eq!(results.len(), 1);
        assert!(results[0].contains("test.ori"));
    }

    #[test]
    #[allow(
        deprecated,
        reason = "tests compile_parallel which is deprecated in favor of execute_parallel"
    )]
    fn test_compile_parallel_multiple() {
        let mut plan = CompilationPlan::new();
        for i in 0..10 {
            plan.add_item(WorkItem::new(p(&format!("file{i}.ori")), h(i)));
        }

        let results: Vec<usize> =
            compile_parallel(&plan, 4, |item| Ok(item.hash.value() as usize)).unwrap();

        assert_eq!(results.len(), 10);
    }

    #[test]
    fn test_compile_error_display() {
        let err = CompileError {
            path: p("test.ori"),
            message: "undefined variable".to_string(),
        };

        let msg = err.to_string();
        assert!(msg.contains("test.ori"));
        assert!(msg.contains("undefined variable"));
    }

    #[test]
    fn test_from_graph_three_file_dependency_order() {
        use crate::aot::incremental::deps::DependencyGraph;

        // Build a 3-file dependency graph:
        //   main.ori → lib.ori → utils.ori
        let mut graph = DependencyGraph::new();
        graph.add_file(p("utils.ori"), h(1), vec![]);
        graph.add_file(p("lib.ori"), h(2), vec![p("utils.ori")]);
        graph.add_file(p("main.ori"), h(3), vec![p("lib.ori")]);

        let files = vec![p("main.ori"), p("lib.ori"), p("utils.ori")];
        let plan = CompilationPlan::from_graph(&graph, &files);

        assert_eq!(plan.len(), 3);
        assert!(!plan.is_complete());

        // Execute the plan through ParallelCompiler to verify topological order
        let compiler = ParallelCompiler::new(ParallelConfig::new(1));
        let mut compilation_order = Vec::new();

        let stats = compiler
            .execute(plan, |item| {
                compilation_order.push(item.path.clone());
                Ok(CompileResult {
                    path: item.path.clone(),
                    output: PathBuf::from(format!("{}.o", item.path.display())),
                    cached: false,
                    time_ms: 1,
                })
            })
            .unwrap_or_else(|_| panic!("compilation should succeed"));

        assert_eq!(stats.total, 3);
        assert_eq!(stats.compiled, 3);

        // Verify topological order: utils before lib, lib before main
        let utils_pos = compilation_order
            .iter()
            .position(|p| p == &PathBuf::from("utils.ori"))
            .unwrap_or_else(|| panic!("utils.ori should be in compilation order"));
        let lib_pos = compilation_order
            .iter()
            .position(|p| p == &PathBuf::from("lib.ori"))
            .unwrap_or_else(|| panic!("lib.ori should be in compilation order"));
        let main_pos = compilation_order
            .iter()
            .position(|p| p == &PathBuf::from("main.ori"))
            .unwrap_or_else(|| panic!("main.ori should be in compilation order"));

        assert!(
            utils_pos < lib_pos,
            "utils.ori ({utils_pos}) should compile before lib.ori ({lib_pos})"
        );
        assert!(
            lib_pos < main_pos,
            "lib.ori ({lib_pos}) should compile before main.ori ({main_pos})"
        );
    }

    // ── execute_parallel tests ─────────────────────────────────

    #[test]
    fn test_execute_parallel_dependency_order() {
        let mut plan = CompilationPlan::new();
        plan.add_item(WorkItem::new(p("main.ori"), h(1)).with_dependencies(vec![p("lib.ori")]));
        plan.add_item(WorkItem::new(p("lib.ori"), h(2)));

        use std::sync::{Arc, Mutex};
        let order = Arc::new(Mutex::new(Vec::new()));
        let order_clone = Arc::clone(&order);

        let stats = execute_parallel(plan, 1, move |item| {
            order_clone
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push(item.path.clone());
            Ok(CompileResult {
                path: item.path.clone(),
                output: PathBuf::from(format!("{}.o", item.path.display())),
                cached: false,
                time_ms: 1,
            })
        })
        .unwrap_or_else(|_| panic!("should succeed"));

        assert_eq!(stats.total, 2);
        let order = order.lock().unwrap_or_else(|e| e.into_inner());
        assert_eq!(order[0], p("lib.ori"), "lib should compile before main");
        assert_eq!(order[1], p("main.ori"));
    }

    #[test]
    fn test_execute_parallel_failure_cascade() {
        let mut plan = CompilationPlan::new();

        // main depends on lib, lib depends on utils
        plan.add_item(WorkItem::new(p("main.ori"), h(1)).with_dependencies(vec![p("lib.ori")]));
        plan.add_item(WorkItem::new(p("lib.ori"), h(2)).with_dependencies(vec![p("utils.ori")]));
        plan.add_item(WorkItem::new(p("utils.ori"), h(3)));

        // utils.ori fails → lib.ori and main.ori should be skipped
        let result = execute_parallel(plan, 1, |item| {
            if item.path == p("utils.ori") {
                Err(CompileError {
                    path: item.path.clone(),
                    message: "utils failed".to_string(),
                })
            } else {
                Ok(CompileResult {
                    path: item.path.clone(),
                    output: p("out.o"),
                    cached: false,
                    time_ms: 1,
                })
            }
        });

        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].path, p("utils.ori"));
        // lib.ori and main.ori should NOT have been attempted (cascade failure)
    }

    #[test]
    fn test_execute_parallel_single_thread_fallback() {
        let mut plan = CompilationPlan::new();
        plan.add_item(WorkItem::new(p("a.ori"), h(1)));

        let stats = execute_parallel(plan, 1, |item| {
            Ok(CompileResult {
                path: item.path.clone(),
                output: p("a.o"),
                cached: false,
                time_ms: 5,
            })
        })
        .unwrap_or_else(|_| panic!("should succeed"));

        assert_eq!(stats.total, 1);
        assert_eq!(stats.compiled, 1);
    }

    #[test]
    fn test_execute_parallel_empty_plan() {
        let plan = CompilationPlan::new();

        let stats = execute_parallel(plan, 4, |_item| {
            Ok(CompileResult {
                path: p("never.ori"),
                output: p("never.o"),
                cached: false,
                time_ms: 0,
            })
        })
        .unwrap_or_else(|_| panic!("empty plan should succeed"));

        assert_eq!(stats.total, 0);
    }

    #[test]
    fn test_execute_parallel_multi_thread_same_as_sequential() {
        // Build the same plan twice, run with 1 thread and 4 threads
        fn make_plan() -> CompilationPlan {
            let mut plan = CompilationPlan::new();
            plan.add_item(WorkItem::new(PathBuf::from("a.ori"), ContentHash::new(1)));
            plan.add_item(WorkItem::new(PathBuf::from("b.ori"), ContentHash::new(2)));
            plan.add_item(WorkItem::new(PathBuf::from("c.ori"), ContentHash::new(3)));
            plan
        }

        let stats_1 = execute_parallel(make_plan(), 1, |item| {
            Ok(CompileResult {
                path: item.path.clone(),
                output: PathBuf::from(format!("{}.o", item.path.display())),
                cached: false,
                time_ms: 1,
            })
        })
        .unwrap_or_else(|_| panic!("should succeed"));

        let stats_4 = execute_parallel(make_plan(), 4, |item| {
            Ok(CompileResult {
                path: item.path.clone(),
                output: PathBuf::from(format!("{}.o", item.path.display())),
                cached: false,
                time_ms: 1,
            })
        })
        .unwrap_or_else(|_| panic!("should succeed"));

        assert_eq!(stats_1.total, stats_4.total);
        assert_eq!(stats_1.compiled, stats_4.compiled);
    }

    // ── mark_failed / transitive_dependents tests ────────────

    #[test]
    fn test_mark_failed_basic() {
        let mut plan = CompilationPlan::new();
        plan.add_item(WorkItem::new(p("a.ori"), h(1)));
        plan.add_item(WorkItem::new(p("b.ori"), h(2)).with_dependencies(vec![p("a.ori")]));

        plan.mark_failed(&p("a.ori"));

        assert!(plan.is_failed(&p("a.ori")));
        assert!(
            plan.is_failed(&p("b.ori")),
            "dependent should be cascade-failed"
        );
        assert_eq!(plan.failed_count(), 2);
        assert!(
            plan.is_complete(),
            "all items failed, plan should be complete"
        );
    }

    #[test]
    fn test_transitive_dependents() {
        let mut plan = CompilationPlan::new();
        plan.add_item(WorkItem::new(p("a.ori"), h(1)));
        plan.add_item(WorkItem::new(p("b.ori"), h(2)).with_dependencies(vec![p("a.ori")]));
        plan.add_item(WorkItem::new(p("c.ori"), h(3)).with_dependencies(vec![p("b.ori")]));

        let deps = plan.transitive_dependents(&p("a.ori"));

        assert_eq!(deps.len(), 2);
        assert!(deps.contains(&p("b.ori")));
        assert!(deps.contains(&p("c.ori")));
    }

    #[test]
    fn test_progress_tracking() {
        let compiler = ParallelCompiler::new(ParallelConfig::new(1));
        assert_eq!(compiler.progress(), 0);

        let mut plan = CompilationPlan::new();
        plan.add_item(WorkItem::new(p("a.ori"), h(1)));
        plan.add_item(WorkItem::new(p("b.ori"), h(2)));

        let _ = compiler.execute(plan, |item| {
            Ok(CompileResult {
                path: item.path.clone(),
                output: p("out.o"),
                cached: false,
                time_ms: 1,
            })
        });

        assert_eq!(compiler.progress(), 2);

        compiler.reset_progress();
        assert_eq!(compiler.progress(), 0);
    }
}
