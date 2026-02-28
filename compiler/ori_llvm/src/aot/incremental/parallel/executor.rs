//! Parallel and sequential compilation executors.
//!
//! Contains [`ParallelCompiler`] for coordinated compilation, and the
//! [`execute_parallel`] / [`execute_sequential`] free functions for
//! dependency-aware parallel execution.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
#[expect(
    clippy::disallowed_types,
    reason = "Arc required for thread-safe sharing"
)]
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::thread;

use super::{
    CompilationPlan, CompilationStats, CompileError, CompileResult, ParallelConfig, WorkItem,
};

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
