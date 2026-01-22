//! Parallel code generation infrastructure.
//!
//! This module provides function-level parallelism for code generation,
//! allowing independent functions to be compiled concurrently.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::collections::HashMap;
use rayon::prelude::*;
use parking_lot::Mutex;

/// A task for generating code for a single function.
#[derive(Clone, Debug)]
pub struct CodegenTask {
    /// Function name.
    pub function_name: String,
    /// Module name containing the function.
    pub module_name: String,
    /// Function index within the module.
    pub function_index: usize,
    /// Estimated code size.
    pub estimated_size: usize,
}

impl CodegenTask {
    /// Create a new codegen task.
    pub fn new(function_name: String, module_name: String, function_index: usize) -> Self {
        CodegenTask {
            function_name,
            module_name,
            function_index,
            estimated_size: 100,
        }
    }

    /// Set the estimated code size.
    pub fn with_estimated_size(mut self, size: usize) -> Self {
        self.estimated_size = size;
        self
    }
}

/// Result of generating code for a function.
#[derive(Clone, Debug)]
pub struct CodegenResult {
    /// Function name.
    pub function_name: String,
    /// Module name.
    pub module_name: String,
    /// Generated code (placeholder - would be actual assembly/IR).
    pub code: Vec<u8>,
    /// Whether generation succeeded.
    pub success: bool,
    /// Error message if failed.
    pub error: Option<String>,
    /// Generation time in microseconds.
    pub time_us: u64,
}

impl CodegenResult {
    /// Create a successful result.
    pub fn success(function_name: String, module_name: String, code: Vec<u8>) -> Self {
        CodegenResult {
            function_name,
            module_name,
            code,
            success: true,
            error: None,
            time_us: 0,
        }
    }

    /// Create a failed result.
    pub fn failure(function_name: String, module_name: String, error: String) -> Self {
        CodegenResult {
            function_name,
            module_name,
            code: Vec::new(),
            success: false,
            error: Some(error),
            time_us: 0,
        }
    }

    /// Set the generation time.
    pub fn with_time(mut self, time_us: u64) -> Self {
        self.time_us = time_us;
        self
    }
}

/// Parallel code generator.
pub struct ParallelCodegen {
    /// Number of worker threads.
    num_workers: usize,
    /// Output directory.
    output_dir: PathBuf,
    /// Generated code cache.
    cache: Arc<Mutex<HashMap<String, Vec<u8>>>>,
}

impl ParallelCodegen {
    /// Create a new parallel code generator.
    pub fn new(num_workers: usize) -> Self {
        let num_workers = if num_workers == 0 {
            rayon::current_num_threads()
        } else {
            num_workers
        };

        ParallelCodegen {
            num_workers,
            output_dir: PathBuf::from("."),
            cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Set the output directory.
    pub fn with_output_dir(mut self, dir: impl AsRef<Path>) -> Self {
        self.output_dir = dir.as_ref().to_path_buf();
        self
    }

    /// Get the number of workers.
    pub fn num_workers(&self) -> usize {
        self.num_workers
    }

    /// Generate code for multiple functions in parallel.
    pub fn generate_parallel(&self, tasks: Vec<CodegenTask>) -> Vec<CodegenResult> {
        tasks
            .into_par_iter()
            .map(|task| self.generate_function(task))
            .collect()
    }

    /// Generate code for a single function.
    pub fn generate_function(&self, task: CodegenTask) -> CodegenResult {
        let start = std::time::Instant::now();

        // Check cache first
        let cache_key = format!("{}::{}", task.module_name, task.function_name);
        {
            let cache = self.cache.lock();
            if let Some(code) = cache.get(&cache_key) {
                return CodegenResult::success(
                    task.function_name,
                    task.module_name,
                    code.clone(),
                ).with_time(0);
            }
        }

        // Generate code (placeholder implementation)
        let code = self.generate_code_for_function(&task);

        let time_us = start.elapsed().as_micros() as u64;

        // Cache the result
        {
            let mut cache = self.cache.lock();
            cache.insert(cache_key, code.clone());
        }

        CodegenResult::success(task.function_name, task.module_name, code)
            .with_time(time_us)
    }

    /// Generate code for a function (placeholder).
    fn generate_code_for_function(&self, task: &CodegenTask) -> Vec<u8> {
        // Placeholder: generate dummy code based on estimated size
        vec![0u8; task.estimated_size]
    }

    /// Generate code for functions grouped by module.
    pub fn generate_by_module(&self, modules: HashMap<String, Vec<CodegenTask>>) -> HashMap<String, Vec<CodegenResult>> {
        modules
            .into_par_iter()
            .map(|(module_name, tasks)| {
                let results: Vec<CodegenResult> = tasks
                    .into_iter()
                    .map(|task| self.generate_function(task))
                    .collect();
                (module_name, results)
            })
            .collect()
    }

    /// Write generated code to output files.
    pub fn write_output(&self, results: &[CodegenResult]) -> std::io::Result<()> {
        use std::fs;
        use std::io::Write;

        // Group results by module
        let mut by_module: HashMap<String, Vec<&CodegenResult>> = HashMap::new();
        for result in results {
            by_module
                .entry(result.module_name.clone())
                .or_default()
                .push(result);
        }

        // Write each module's code to a file
        for (module_name, module_results) in by_module {
            let output_path = self.output_dir.join(format!("{}.o", module_name));

            let mut file = fs::File::create(&output_path)?;
            for result in module_results {
                if result.success {
                    file.write_all(&result.code)?;
                }
            }
        }

        Ok(())
    }

    /// Clear the code cache.
    pub fn clear_cache(&self) {
        self.cache.lock().clear();
    }

    /// Get the cache size.
    pub fn cache_size(&self) -> usize {
        self.cache.lock().len()
    }
}

impl Default for ParallelCodegen {
    fn default() -> Self {
        Self::new(0)
    }
}

/// Statistics for parallel code generation.
#[derive(Clone, Debug, Default)]
pub struct CodegenStats {
    /// Total functions generated.
    pub functions_generated: usize,
    /// Functions generated in parallel.
    pub parallel_functions: usize,
    /// Total bytes generated.
    pub total_bytes: usize,
    /// Cache hits.
    pub cache_hits: usize,
    /// Total generation time in microseconds.
    pub total_time_us: u64,
}

impl CodegenStats {
    /// Calculate from results.
    pub fn from_results(results: &[CodegenResult]) -> Self {
        let mut stats = CodegenStats::default();
        stats.functions_generated = results.len();
        stats.parallel_functions = results.len();

        for result in results {
            if result.success {
                stats.total_bytes += result.code.len();
            }
            stats.total_time_us += result.time_us;
        }

        stats
    }

    /// Calculate throughput in functions per second.
    pub fn throughput(&self) -> f64 {
        if self.total_time_us == 0 {
            0.0
        } else {
            (self.functions_generated as f64 * 1_000_000.0) / self.total_time_us as f64
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_codegen_task_creation() {
        let task = CodegenTask::new("foo".to_string(), "main".to_string(), 0);
        assert_eq!(task.function_name, "foo");
        assert_eq!(task.module_name, "main");
        assert_eq!(task.estimated_size, 100);
    }

    #[test]
    fn test_codegen_result_success() {
        let result = CodegenResult::success(
            "foo".to_string(),
            "main".to_string(),
            vec![1, 2, 3],
        );
        assert!(result.success);
        assert!(result.error.is_none());
        assert_eq!(result.code.len(), 3);
    }

    #[test]
    fn test_codegen_result_failure() {
        let result = CodegenResult::failure(
            "foo".to_string(),
            "main".to_string(),
            "compilation error".to_string(),
        );
        assert!(!result.success);
        assert!(result.error.is_some());
        assert!(result.code.is_empty());
    }

    #[test]
    fn test_parallel_codegen_single() {
        let codegen = ParallelCodegen::new(4);
        let task = CodegenTask::new("foo".to_string(), "main".to_string(), 0);

        let result = codegen.generate_function(task);
        assert!(result.success);
    }

    #[test]
    fn test_parallel_codegen_multiple() {
        let codegen = ParallelCodegen::new(4);
        let tasks: Vec<CodegenTask> = (0..10)
            .map(|i| CodegenTask::new(format!("func_{}", i), "main".to_string(), i))
            .collect();

        let results = codegen.generate_parallel(tasks);
        assert_eq!(results.len(), 10);
        assert!(results.iter().all(|r| r.success));
    }

    #[test]
    fn test_codegen_cache() {
        let codegen = ParallelCodegen::new(4);
        let task = CodegenTask::new("foo".to_string(), "main".to_string(), 0);

        // First generation
        let result1 = codegen.generate_function(task.clone());
        assert_eq!(codegen.cache_size(), 1);

        // Second generation should hit cache
        let result2 = codegen.generate_function(task);
        assert_eq!(codegen.cache_size(), 1);

        // Results should be the same
        assert_eq!(result1.code, result2.code);
    }

    #[test]
    fn test_codegen_stats() {
        let results = vec![
            CodegenResult::success("a".to_string(), "m".to_string(), vec![1, 2, 3])
                .with_time(100),
            CodegenResult::success("b".to_string(), "m".to_string(), vec![4, 5])
                .with_time(50),
        ];

        let stats = CodegenStats::from_results(&results);
        assert_eq!(stats.functions_generated, 2);
        assert_eq!(stats.total_bytes, 5);
        assert_eq!(stats.total_time_us, 150);
    }
}
