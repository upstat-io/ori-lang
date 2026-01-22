//! Work-stealing pool for parallel type checking.
//!
//! This module provides a work-stealing thread pool optimized for
//! type checking tasks with varying complexity.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use crossbeam::deque::{Injector, Stealer, Worker};
use crossbeam::utils::Backoff;

/// A unit of work to be processed.
pub trait WorkItem: Send + 'static {
    /// The result type produced by this work item.
    type Result: Send + 'static;

    /// Process this work item and produce a result.
    fn process(self) -> Self::Result;

    /// Estimate the cost of this work item (for scheduling).
    fn estimated_cost(&self) -> usize {
        1
    }
}

/// Result of processing a work item.
#[derive(Debug)]
pub struct WorkResult<T> {
    /// The result value.
    pub value: T,
    /// Worker thread that processed this item.
    pub worker_id: usize,
    /// Processing time in microseconds.
    pub time_us: u64,
}

/// Work-stealing thread pool.
pub struct WorkPool {
    /// Global work queue.
    injector: Arc<Injector<BoxedWork>>,
    /// Stealers for each worker.
    stealers: Vec<Stealer<BoxedWork>>,
    /// Number of workers.
    num_workers: usize,
    /// Whether the pool is shutting down.
    shutdown: Arc<AtomicBool>,
    /// Number of active workers.
    active_workers: Arc<AtomicUsize>,
}

/// Type-erased work item.
type BoxedWork = Box<dyn FnOnce() + Send + 'static>;

impl WorkPool {
    /// Create a new work pool with the specified number of workers.
    pub fn new(num_workers: usize) -> Self {
        let num_workers = if num_workers == 0 {
            rayon::current_num_threads()
        } else {
            num_workers
        };

        let injector = Arc::new(Injector::new());
        let stealers = Vec::new();

        WorkPool {
            injector,
            stealers,
            num_workers,
            shutdown: Arc::new(AtomicBool::new(false)),
            active_workers: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Get the number of workers.
    pub fn num_workers(&self) -> usize {
        self.num_workers
    }

    /// Submit work to the pool.
    pub fn submit<W: WorkItem>(&self, work: W) -> () {
        let boxed: BoxedWork = Box::new(move || {
            let _ = work.process();
        });
        self.injector.push(boxed);
    }

    /// Submit a batch of work items.
    pub fn submit_batch<W: WorkItem, I: IntoIterator<Item = W>>(&self, items: I) {
        for item in items {
            self.submit(item);
        }
    }

    /// Process all pending work using the current thread pool.
    ///
    /// This is a simpler interface that uses Rayon's thread pool
    /// instead of managing workers manually.
    pub fn process_all_rayon<W, R, I>(&self, items: I) -> Vec<R>
    where
        W: WorkItem<Result = R> + Send,
        R: Send,
        I: IntoIterator<Item = W>,
        I::IntoIter: Send,
    {
        use rayon::prelude::*;

        let items: Vec<W> = items.into_iter().collect();
        items.into_par_iter().map(|w| w.process()).collect()
    }

    /// Check if the pool is empty.
    pub fn is_empty(&self) -> bool {
        self.injector.is_empty()
    }

    /// Get the number of pending work items.
    pub fn pending_count(&self) -> usize {
        self.injector.len()
    }
}

impl Default for WorkPool {
    fn default() -> Self {
        Self::new(0)
    }
}

/// A simple work item for type checking a module.
#[derive(Debug)]
pub struct TypeCheckWork {
    /// Module name.
    pub module_name: String,
    /// Module index.
    pub module_index: usize,
    /// Estimated complexity.
    pub complexity: usize,
}

impl TypeCheckWork {
    /// Create a new type check work item.
    pub fn new(module_name: String, module_index: usize) -> Self {
        TypeCheckWork {
            module_name,
            module_index,
            complexity: 1,
        }
    }

    /// Create with estimated complexity.
    pub fn with_complexity(mut self, complexity: usize) -> Self {
        self.complexity = complexity;
        self
    }
}

impl WorkItem for TypeCheckWork {
    type Result = TypeCheckResult;

    fn process(self) -> Self::Result {
        // Placeholder implementation - actual type checking would happen here
        TypeCheckResult {
            module_name: self.module_name,
            module_index: self.module_index,
            success: true,
            error_count: 0,
        }
    }

    fn estimated_cost(&self) -> usize {
        self.complexity
    }
}

/// Result of type checking a module.
#[derive(Debug)]
pub struct TypeCheckResult {
    /// Module name.
    pub module_name: String,
    /// Module index.
    pub module_index: usize,
    /// Whether type checking succeeded.
    pub success: bool,
    /// Number of type errors.
    pub error_count: usize,
}

/// Level-based parallel type checker.
pub struct LevelTypeChecker {
    pool: WorkPool,
}

impl LevelTypeChecker {
    /// Create a new level-based type checker.
    pub fn new(num_workers: usize) -> Self {
        LevelTypeChecker {
            pool: WorkPool::new(num_workers),
        }
    }

    /// Type check modules level by level.
    ///
    /// Each level is processed in parallel, but levels are processed
    /// sequentially to respect dependencies.
    pub fn check_levels(&self, levels: &[Vec<TypeCheckWork>]) -> Vec<TypeCheckResult> {
        let mut all_results = Vec::new();

        for level in levels {
            // Process all modules at this level in parallel
            let results = self.pool.process_all_rayon(level.iter().cloned());
            all_results.extend(results);
        }

        all_results
    }
}

impl Clone for TypeCheckWork {
    fn clone(&self) -> Self {
        TypeCheckWork {
            module_name: self.module_name.clone(),
            module_index: self.module_index,
            complexity: self.complexity,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_work_pool_creation() {
        let pool = WorkPool::new(4);
        assert_eq!(pool.num_workers(), 4);
        assert!(pool.is_empty());
    }

    #[test]
    fn test_work_pool_default() {
        let pool = WorkPool::default();
        assert!(pool.num_workers() > 0);
    }

    #[test]
    fn test_type_check_work() {
        let work = TypeCheckWork::new("test".to_string(), 0);
        assert_eq!(work.module_name, "test");
        assert_eq!(work.module_index, 0);
        assert_eq!(work.estimated_cost(), 1);

        let work = work.with_complexity(10);
        assert_eq!(work.estimated_cost(), 10);
    }

    #[test]
    fn test_process_all_rayon() {
        let pool = WorkPool::new(4);

        let items: Vec<TypeCheckWork> = (0..10)
            .map(|i| TypeCheckWork::new(format!("module_{}", i), i))
            .collect();

        let results = pool.process_all_rayon(items);
        assert_eq!(results.len(), 10);

        for (i, result) in results.iter().enumerate() {
            assert!(result.success);
            assert_eq!(result.module_index, i);
        }
    }

    #[test]
    fn test_level_type_checker() {
        let checker = LevelTypeChecker::new(4);

        let levels = vec![
            // Level 0: no dependencies
            vec![
                TypeCheckWork::new("a".to_string(), 0),
                TypeCheckWork::new("b".to_string(), 1),
            ],
            // Level 1: depends on level 0
            vec![
                TypeCheckWork::new("c".to_string(), 2),
            ],
        ];

        let results = checker.check_levels(&levels);
        assert_eq!(results.len(), 3);
    }
}
