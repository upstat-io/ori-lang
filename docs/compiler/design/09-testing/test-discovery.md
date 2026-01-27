---
title: "Test Discovery"
description: "Ori Compiler Design — Test Discovery"
order: 901
section: "Testing"
---

# Test Discovery

Test discovery finds all test functions in a module and determines coverage.

## Location

```
compiler/oric/src/test/discovery.rs (~310 lines)
```

## Discovery Process

```rust
pub fn discover_tests(module: &Module) -> TestDiscovery {
    let mut tests = Vec::new();
    let mut targets: HashMap<Name, Vec<Name>> = HashMap::new();

    for item in &module.items {
        if let Item::Test(test) = item {
            tests.push(TestInfo {
                name: test.name,
                targets: test.targets.clone(),
                attributes: test.attributes.clone(),
                body: test.body,
            });

            // Track which functions are tested
            for target in &test.targets {
                targets.entry(*target)
                    .or_default()
                    .push(test.name);
            }
        }
    }

    TestDiscovery { tests, targets }
}
```

## TestInfo Structure

```rust
pub struct TestInfo {
    /// Test function name
    pub name: Name,

    /// Functions this test targets
    pub targets: Vec<Name>,

    /// Test attributes
    pub attributes: TestAttributes,

    /// Test body
    pub body: ExprId,
}

pub struct TestAttributes {
    pub skip: Option<String>,
    pub compile_fail: Option<String>,
    pub should_fail: Option<String>,
}
```

## Coverage Checking

```rust
pub fn check_coverage(module: &Module, discovery: &TestDiscovery) -> CoverageReport {
    let mut report = CoverageReport::new();

    for func in &module.functions {
        // Skip main and private functions
        if func.name.as_str() == "main" {
            continue;
        }

        if discovery.targets.contains_key(&func.name) {
            report.covered.push(func.name);
        } else {
            report.uncovered.push(func.name);
        }
    }

    report
}

pub struct CoverageReport {
    pub covered: Vec<Name>,
    pub uncovered: Vec<Name>,
}

impl CoverageReport {
    pub fn percentage(&self) -> f64 {
        let total = self.covered.len() + self.uncovered.len();
        if total == 0 {
            100.0
        } else {
            (self.covered.len() as f64 / total as f64) * 100.0
        }
    }
}
```

## Mandatory Testing

Compilation fails if functions lack tests:

```rust
pub fn check_mandatory_tests(module: &Module, discovery: &TestDiscovery) -> Vec<Problem> {
    let mut problems = Vec::new();

    for func in &module.functions {
        // Exemptions
        if func.name.as_str() == "main" { continue; }
        if func.is_private() { continue; }  // Private functions can use :: import for testing

        if !discovery.targets.contains_key(&func.name) {
            problems.push(Problem::UntrestedFunction {
                name: func.name,
                span: func.span,
            });
        }
    }

    problems
}
```

Error message:
```
error: function `@process` has no test
 --> src/libsi:10:1
   |
10 | @process (data: Data) -> Result<Output, Error> = ...
   | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
   |
   = help: add a test like:
     @test_process tests @process () -> void = run(...)
```

## Filtering Tests

```rust
pub fn filter_tests(
    discovery: &TestDiscovery,
    filter: &TestFilter,
) -> Vec<&TestInfo> {
    discovery.tests.iter()
        .filter(|test| {
            // Skip skipped tests
            if test.attributes.skip.is_some() {
                return false;
            }

            // Name filter
            if let Some(pattern) = &filter.name {
                if !test.name.as_str().contains(pattern) {
                    return false;
                }
            }

            // Target filter
            if let Some(target) = &filter.target {
                if !test.targets.iter().any(|t| t == target) {
                    return false;
                }
            }

            true
        })
        .collect()
}

pub struct TestFilter {
    pub name: Option<String>,
    pub target: Option<Name>,
}
```

## Test Ordering

Tests are ordered for deterministic output:

```rust
pub fn order_tests(tests: &mut [&TestInfo]) {
    tests.sort_by(|a, b| {
        // Sort by:
        // 1. Targeted tests before free-floating
        // 2. Alphabetically by name
        let a_targeted = !a.targets.is_empty();
        let b_targeted = !b.targets.is_empty();

        match (a_targeted, b_targeted) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.cmp(&b.name),
        }
    });
}
```

## Compile-Fail Tests

Special handling for tests that should fail to compile:

```rust
pub fn handle_compile_fail_tests(
    module: &Module,
    discovery: &TestDiscovery,
) -> Vec<CompileFailResult> {
    discovery.tests.iter()
        .filter(|t| t.attributes.compile_fail.is_some())
        .map(|test| {
            // Try to compile just this test's body
            let result = try_compile_isolated(module, test.body);

            match result {
                Ok(_) => CompileFailResult::UnexpectedSuccess(test.name),
                Err(errors) => {
                    let expected = test.attributes.compile_fail.as_ref().unwrap();
                    if errors.iter().any(|e| e.message.contains(expected)) {
                        CompileFailResult::ExpectedFailure(test.name)
                    } else {
                        CompileFailResult::WrongError {
                            test: test.name,
                            expected: expected.clone(),
                            actual: errors,
                        }
                    }
                }
            }
        })
        .collect()
}
```
