//! The `test` command: discover and run Ori spec tests, report results.

use oric::test::{CoverageReport, TestRunner, TestRunnerConfig, TestSummary};
use oric::TestOutcome;
use std::path::Path;

/// Run tests at the given path with the provided configuration.
pub fn run_tests(path: &str, config: &TestRunnerConfig) {
    let path = Path::new(path);

    if !path.exists() {
        eprintln!("Path not found: {}", path.display());
        std::process::exit(1);
    }

    let runner = TestRunner::with_config(config.clone());

    // Generate coverage report if requested
    if config.coverage {
        let report = runner.coverage_report(path);
        print_coverage_report(&report);
        std::process::exit(i32::from(!report.is_complete()));
    }

    let summary = runner.run(path);

    // Print results
    print_test_summary(&summary, config.verbose);

    // Exit with appropriate code
    std::process::exit(summary.exit_code());
}

/// Print a coverage report showing which functions have tests.
fn print_coverage_report(report: &CoverageReport) {
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
            let tests = func.test_names.join(", ");
            println!("  @{} <- {}", func.name, tests);
        }
        println!();
    }

    // Print uncovered functions
    let uncovered: Vec<_> = report.functions.iter().filter(|f| !f.has_tests()).collect();
    if !uncovered.is_empty() {
        println!("Uncovered ({}):", uncovered.len());
        for func in uncovered {
            println!("  @{}", func.name);
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
fn print_test_summary(summary: &TestSummary, verbose: bool) {
    // Print file-by-file results
    for file in &summary.files {
        if file.total() == 0 && file.errors.is_empty() {
            continue;
        }

        // Print file errors (parse/type errors)
        if !file.errors.is_empty() {
            println!("\n{}", file.path.display());
            for error in &file.errors {
                println!("  ERROR: {error}");
            }
            continue;
        }

        if verbose || file.has_failures() {
            println!("\n{}", file.path.display());
        }

        for result in &file.results {
            let status = match &result.outcome {
                TestOutcome::Passed => {
                    if verbose {
                        format!("  PASS: {} ({:.2?})", result.name, result.duration)
                    } else {
                        continue;
                    }
                }
                TestOutcome::Failed(msg) => {
                    format!("  FAIL: {} - {}", result.name, msg)
                }
                TestOutcome::Skipped(reason) => {
                    if verbose {
                        format!("  SKIP: {} - {}", result.name, reason)
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
    println!(
        "  {} passed, {} failed, {} skipped ({} total)",
        summary.passed,
        summary.failed,
        summary.skipped,
        summary.total()
    );
    println!("  Completed in {:.2?}", summary.duration);

    if summary.has_failures() {
        println!();
        println!("FAILED");
    } else if summary.total() == 0 {
        println!();
        println!("NO TESTS FOUND");
    } else {
        println!();
        println!("OK");
    }
}
