//! Parallel Compilation Support
//!
//! Provides parallel compilation of independent modules for faster builds.

use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
#[expect(
    clippy::disallowed_types,
    reason = "Arc required for thread-safe sharing"
)]
use std::sync::{Arc, Mutex};
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

        if item.dependencies.is_empty() {
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
    pub fn complete(&mut self, path: &Path) {
        self.completed.insert(path.to_path_buf());

        // Check if any pending items are now ready
        let mut newly_ready = Vec::new();

        for &idx in &self.pending {
            let item = &self.items[idx];
            let deps_satisfied = item.dependencies.iter().all(|d| self.completed.contains(d));

            if deps_satisfied {
                newly_ready.push(idx);
            }
        }

        for idx in newly_ready {
            self.pending.remove(&idx);
            self.ready.push_back(idx);
        }
    }

    /// Check if the plan is complete.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.ready.is_empty() && self.pending.is_empty()
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
#[derive(Debug)]
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
#[derive(Debug, Default)]
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

/// Execute compilation in parallel using multiple threads.
///
/// This is a more sophisticated parallel executor using a work-stealing approach.
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
