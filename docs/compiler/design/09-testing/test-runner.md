# Test Runner

The test runner executes discovered tests in parallel and reports results.

## Location

```
compiler/sigilc/src/test/runner.rs (~494 lines)
```

## Runner Structure

```rust
pub struct TestRunner {
    /// Pattern registry for evaluation
    pattern_registry: SharedPatternRegistry,

    /// Type registry for evaluation
    type_registry: SharedTypeRegistry,

    /// Thread pool for parallel execution
    thread_pool: ThreadPool,

    /// Configuration
    config: TestConfig,
}

pub struct TestConfig {
    /// Number of parallel threads
    pub parallelism: usize,

    /// Timeout per test
    pub timeout: Duration,

    /// Output format
    pub output: TestOutputFormat,

    /// Fail fast on first failure
    pub fail_fast: bool,
}
```

## Running Tests

```rust
impl TestRunner {
    pub fn run(&self, tests: &[TestInfo], module: &Module) -> TestResults {
        let (tx, rx) = channel();

        // Submit tests to thread pool
        for test in tests {
            let tx = tx.clone();
            let test = test.clone();
            let module = module.clone();
            let pattern_registry = self.pattern_registry.clone();
            let type_registry = self.type_registry.clone();
            let timeout = self.config.timeout;

            self.thread_pool.execute(move || {
                let result = run_single_test(
                    &test,
                    &module,
                    &pattern_registry,
                    &type_registry,
                    timeout,
                );
                tx.send((test.name, result)).ok();
            });
        }

        drop(tx);

        // Collect results
        let mut results = TestResults::new();
        for (name, result) in rx {
            results.add(name, result);

            if self.config.fail_fast && result.is_failure() {
                break;
            }
        }

        results
    }
}
```

## Single Test Execution

```rust
fn run_single_test(
    test: &TestInfo,
    module: &Module,
    pattern_registry: &SharedPatternRegistry,
    type_registry: &SharedTypeRegistry,
    timeout: Duration,
) -> TestResult {
    let start = Instant::now();

    // Handle special test types
    if let Some(reason) = &test.attributes.skip {
        return TestResult::Skipped(reason.clone());
    }

    // Run with timeout
    let outcome = std::panic::catch_unwind(|| {
        let mut evaluator = Evaluator::new(
            module.clone(),
            pattern_registry.clone(),
            type_registry.clone(),
        );

        // Set up timeout
        let deadline = Instant::now() + timeout;

        match evaluator.eval_expr_with_deadline(test.body, deadline) {
            Ok(Value::Void) => TestOutcome::Passed,
            Ok(other) => TestOutcome::WrongReturn(other),
            Err(EvalError::Timeout) => TestOutcome::Timeout,
            Err(e) => TestOutcome::Failed(e),
        }
    });

    let duration = start.elapsed();

    match outcome {
        Ok(TestOutcome::Passed) => {
            // Check if we expected failure
            if let Some(expected) = &test.attributes.should_fail {
                TestResult::UnexpectedPass(expected.clone(), duration)
            } else {
                TestResult::Passed(duration)
            }
        }

        Ok(TestOutcome::Failed(e)) => {
            // Check if we expected this failure
            if let Some(expected) = &test.attributes.should_fail {
                if e.message.contains(expected) {
                    TestResult::Passed(duration)
                } else {
                    TestResult::WrongFailure {
                        expected: expected.clone(),
                        actual: e,
                        duration,
                    }
                }
            } else {
                TestResult::Failed(e, duration)
            }
        }

        Ok(TestOutcome::Timeout) => TestResult::Timeout(duration),
        Ok(TestOutcome::WrongReturn(v)) => TestResult::WrongReturn(v, duration),
        Err(panic) => TestResult::Panicked(format!("{:?}", panic), duration),
    }
}
```

## Test Results

```rust
pub struct TestResults {
    pub passed: Vec<(Name, Duration)>,
    pub failed: Vec<(Name, EvalError, Duration)>,
    pub skipped: Vec<(Name, String)>,
    pub timed_out: Vec<(Name, Duration)>,
}

impl TestResults {
    pub fn summary(&self) -> String {
        format!(
            "{} passed, {} failed, {} skipped",
            self.passed.len(),
            self.failed.len(),
            self.skipped.len(),
        )
    }

    pub fn success(&self) -> bool {
        self.failed.is_empty() && self.timed_out.is_empty()
    }

    pub fn total_duration(&self) -> Duration {
        let all_durations = self.passed.iter().map(|(_, d)| *d)
            .chain(self.failed.iter().map(|(_, _, d)| *d))
            .chain(self.timed_out.iter().map(|(_, d)| *d));

        all_durations.sum()
    }
}
```

## Output Formatting

### Terminal Output

```rust
fn format_terminal(results: &TestResults) -> String {
    let mut output = String::new();

    for (name, duration) in &results.passed {
        output.push_str(&format!(
            "test {} ... \x1b[32mok\x1b[0m ({:?})\n",
            name, duration
        ));
    }

    for (name, error, duration) in &results.failed {
        output.push_str(&format!(
            "test {} ... \x1b[31mFAILED\x1b[0m ({:?})\n",
            name, duration
        ));
        output.push_str(&format!("  {}\n", error.message));
        if let Some(span) = error.span {
            output.push_str(&format!("    at {}\n", span));
        }
    }

    for (name, reason) in &results.skipped {
        output.push_str(&format!(
            "test {} ... \x1b[33mskipped\x1b[0m ({})\n",
            name, reason
        ));
    }

    output.push_str(&format!("\n{}\n", results.summary()));
    output
}
```

### JSON Output

```rust
fn format_json(results: &TestResults) -> String {
    serde_json::to_string_pretty(&json!({
        "passed": results.passed.iter().map(|(n, d)| json!({
            "name": n.to_string(),
            "duration_ms": d.as_millis(),
        }))collect::<Vec<_>>(),
        "failed": results.failed.iter().map(|(n, e, d)| json!({
            "name": n.to_string(),
            "error": e.message,
            "duration_ms": d.as_millis(),
        }))collect::<Vec<_>>(),
        "skipped": results.skipped.iter().map(|(n, r)| json!({
            "name": n.to_string(),
            "reason": r,
        }))collect::<Vec<_>>(),
        "summary": {
            "passed": results.passed.len(),
            "failed": results.failed.len(),
            "skipped": results.skipped.len(),
            "total_duration_ms": results.total_duration().as_millis(),
        }
    })).unwrap()
}
```

## Parallel Execution

Tests run in parallel using a thread pool:

```rust
impl TestRunner {
    pub fn new(config: TestConfig) -> Self {
        let parallelism = config.parallelism
            .unwrap_or_else(|| num_cpus::get());

        Self {
            thread_pool: ThreadPool::new(parallelism),
            config,
            // ...
        }
    }
}
```

Benefits:
- Fast execution on multi-core machines
- Each test gets isolated environment
- Results collected as they complete

## CLI Usage

```bash
# Run all tests
sigil test

# Run tests matching pattern
sigil test math

# Run tests for specific function
sigil test --target add

# Run with parallelism
sigil test --jobs 8

# Output JSON
sigil test --format json
```
