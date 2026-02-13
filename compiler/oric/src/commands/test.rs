//! The `test` command: discover and run Ori spec tests, report results.

use oric::ir::StringInterner;
use oric::test::{CoverageReport, TestRunner, TestRunnerConfig, TestSummary};
use oric::TestOutcome;
use std::path::Path;

/// Run tests at the given path with the provided configuration.
/// Returns the exit code (0 for success, non-zero for failure).
pub fn run_tests(path: &str, config: &TestRunnerConfig) -> i32 {
    let path = Path::new(path);

    if !path.exists() {
        eprintln!("Path not found: {}", path.display());
        return 1;
    }

    let runner = TestRunner::with_config(config.clone());

    // Generate coverage report if requested
    if config.coverage {
        let report = runner.coverage_report(path);
        print_coverage_report(&report, runner.interner());
        return i32::from(!report.is_complete());
    }

    let summary = runner.run(path);

    // Print results
    print_test_summary(&summary, runner.interner(), config.verbose);

    summary.exit_code()
}

/// Print a coverage report showing which functions have tests.
fn print_coverage_report(report: &CoverageReport, interner: &StringInterner) {
    println!("Coverage Report");
    println!("===============");
    println!();

    if report.total == 0 {
        println!("No functions found.");
        return;
    }

    // Print covered functions
    let covered: Vec<_> = report.functions.iter().filter(|f| f.has_tests()).collect();
    if !covered.is_empty() {
        println!("Covered ({}):", covered.len());
        for func in covered {
            let tests: Vec<_> = func
                .test_names
                .iter()
                .map(|n| interner.lookup(*n))
                .collect();
            let func_name = func.name_str(interner);
            println!("  @{} <- {}", func_name, tests.join(", "));
        }
        println!();
    }

    // Print uncovered functions
    let uncovered: Vec<_> = report.functions.iter().filter(|f| !f.has_tests()).collect();
    if !uncovered.is_empty() {
        println!("Uncovered ({}):", uncovered.len());
        for func in uncovered {
            let func_name = func.name_str(interner);
            println!("  @{func_name}");
        }
        println!();
    }

    // Print summary
    println!(
        "Summary: {}/{} functions covered ({:.1}%)",
        report.covered,
        report.total,
        report.percentage()
    );

    if report.is_complete() {
        println!("\nOK");
    } else {
        println!("\nMISSING COVERAGE");
    }
}

/// Print a summary of test results, with optional verbose output.
fn print_test_summary(summary: &TestSummary, interner: &StringInterner, verbose: bool) {
    // Print file-by-file results
    for file in &summary.files {
        if file.total() == 0 && file.errors.is_empty() {
            continue;
        }

        // Print file errors (parse/type/LLVM errors) and blocked tests
        if !file.errors.is_empty() {
            // Skip LLVM compile errors in non-verbose mode
            if file.llvm_compile_error && !verbose {
                continue;
            }

            println!("\n{}", file.path.display());
            if file.llvm_compile_error {
                // Show the compilation error, then list blocked tests
                for error in &file.errors {
                    println!("  ERROR: {error}");
                }
                let blocked = file
                    .results
                    .iter()
                    .filter(|r| r.outcome.is_llvm_compile_fail())
                    .count();
                println!("  ({blocked} tests blocked)");
            } else {
                // Show which tests were blocked (real failures)
                for result in &file.results {
                    if result.outcome.is_failed() {
                        let name = result.name_str(interner);
                        println!("  FAIL: {name} - blocked by type errors");
                    }
                }
                // Then show the actual errors
                for error in &file.errors {
                    println!("    ERROR: {error}");
                }
            }
            continue;
        }

        if verbose || file.has_failures() {
            println!("\n{}", file.path.display());
        }

        for result in &file.results {
            let name = result.name_str(interner);
            let status = match &result.outcome {
                TestOutcome::Passed => {
                    if verbose {
                        format!("  PASS: {name} ({:.2?})", result.duration)
                    } else {
                        continue;
                    }
                }
                TestOutcome::Failed(msg) => {
                    format!("  FAIL: {name} - {msg}")
                }
                TestOutcome::Skipped(reason) => {
                    if verbose {
                        format!("  SKIP: {name} - {reason}")
                    } else {
                        continue;
                    }
                }
                TestOutcome::SkippedUnchanged => {
                    if verbose {
                        format!("  SKIP: {name} (unchanged)")
                    } else {
                        continue;
                    }
                }
                TestOutcome::LlvmCompileFail(reason) => {
                    if verbose {
                        format!("  LLVM COMPILE FAIL: {name} - {reason}")
                    } else {
                        continue;
                    }
                }
            };
            println!("{status}");
        }
    }

    // Print summary
    println!();
    println!("Test Summary:");

    let mut parts = vec![
        format!("{} passed", summary.passed),
        format!("{} failed", summary.failed),
        format!("{} skipped", summary.skipped),
    ];
    if summary.skipped_unchanged > 0 {
        parts.push(format!("{} skipped (unchanged)", summary.skipped_unchanged));
    }
    if summary.llvm_compile_fail > 0 {
        parts.push(format!("{} llvm compile fail", summary.llvm_compile_fail));
    }
    if summary.error_files > 0 {
        parts.push(format!("{} files with errors", summary.error_files));
    }
    println!("  {}", parts.join(", "));

    if summary.llvm_compile_fail_files > 0 {
        println!(
            "  ({} {} could not compile via LLVM)",
            summary.llvm_compile_fail_files,
            if summary.llvm_compile_fail_files == 1 {
                "file"
            } else {
                "files"
            }
        );

        // Show breakdown of unique error reasons
        let mut error_counts: rustc_hash::FxHashMap<&str, usize> = rustc_hash::FxHashMap::default();
        for file in &summary.files {
            if file.llvm_compile_error {
                for error in &file.errors {
                    *error_counts.entry(error.as_str()).or_default() += 1;
                }
            }
        }
        if !error_counts.is_empty() {
            let mut sorted: Vec<_> = error_counts.into_iter().collect();
            sorted.sort_by(|a, b| b.1.cmp(&a.1));
            println!("  Reasons:");
            for (error, count) in &sorted {
                let plural = if *count == 1 { "file" } else { "files" };
                // Truncate long error messages for readability
                let display = if error.len() > 80 {
                    format!("{}...", &error[..77])
                } else {
                    (*error).to_string()
                };
                println!("    {count} {plural}: {display}");
            }
        }
    }

    println!("  Completed in {:.2?}", summary.duration);

    if summary.has_failures() {
        println!();
        println!("FAILED");
    } else if summary.total() == 0
        && summary.llvm_compile_fail == 0
        && summary.llvm_compile_fail_files == 0
    {
        println!();
        println!("NO TESTS FOUND");
    } else {
        println!();
        println!("OK");
    }
}
