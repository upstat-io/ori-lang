//! Sigil Compiler CLI
//!
//! Salsa-first incremental compiler.

use sigilc::{CompilerDb, SourceFile, Db};
use sigilc::query::{tokens, parsed, typed, evaluated};
use sigilc::test::{TestRunner, TestRunnerConfig, TestSummary};
use std::path::{Path, PathBuf};

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        print_usage();
        return;
    }

    let command = &args[1];

    match command.as_str() {
        "run" => {
            if args.len() < 3 {
                eprintln!("Usage: sigil run <file.si>");
                std::process::exit(1);
            }
            run_file(&args[2]);
        }
        "test" => {
            // Parse args: path is optional, flags can come before or after
            let mut path: Option<String> = None;
            let mut config = TestRunnerConfig::default();

            for arg in args.iter().skip(2) {
                if arg.starts_with("--filter=") {
                    config.filter = Some(arg[9..].to_string());
                } else if arg == "--verbose" || arg == "-v" {
                    config.verbose = true;
                } else if arg == "--no-parallel" {
                    config.parallel = false;
                } else if arg == "--coverage" {
                    config.coverage = true;
                } else if !arg.starts_with('-') && path.is_none() {
                    path = Some(arg.clone());
                }
            }

            // Use provided path or current directory
            let path = path.unwrap_or_else(|| ".".to_string());
            run_tests(&path, config);
        }
        "check" => {
            if args.len() < 3 {
                eprintln!("Usage: sigil check <file.si>");
                std::process::exit(1);
            }
            check_file(&args[2]);
        }
        "parse" => {
            if args.len() < 3 {
                eprintln!("Usage: sigil parse <file.si>");
                std::process::exit(1);
            }
            parse_file(&args[2]);
        }
        "lex" => {
            if args.len() < 3 {
                eprintln!("Usage: sigil lex <file.si>");
                std::process::exit(1);
            }
            lex_file(&args[2]);
        }
        "help" | "--help" | "-h" => {
            print_usage();
        }
        "version" | "--version" | "-v" => {
            println!("Sigil Compiler 0.1.0-alpha.1");
            println!("Salsa-first incremental compilation");
        }
        _ => {
            // If it looks like a file path, try to run it
            if command.ends_with(".si") {
                run_file(command);
            } else {
                eprintln!("Unknown command: {}", command);
                eprintln!();
                print_usage();
                std::process::exit(1);
            }
        }
    }
}

fn print_usage() {
    println!("Sigil Compiler (Salsa-first incremental compilation)");
    println!();
    println!("Usage: sigil <command> [options]");
    println!();
    println!("Commands:");
    println!("  run <file.si>     Run/evaluate a Sigil program");
    println!("  test [path]       Run tests (default: current directory)");
    println!("  check <file.si>   Type check a file (no execution)");
    println!("  parse <file.si>   Parse and display AST info");
    println!("  lex <file.si>     Tokenize and display tokens");
    println!("  help              Show this help message");
    println!("  version           Show version information");
    println!();
    println!("Test options:");
    println!("  --filter=<pattern>  Only run tests matching pattern");
    println!("  --verbose, -v       Show detailed output");
    println!("  --no-parallel       Run tests sequentially");
    println!();
    println!("Examples:");
    println!("  sigil run main.si");
    println!("  sigil test                    # Run all spec tests");
    println!("  sigil test tests/spec/patterns/");
    println!("  sigil test --filter=map");
    println!("  sigil check lib.si");
    println!("  sigil main.si       (shorthand for 'run')");
}

fn read_file(path: &str) -> String {
    match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("Error reading '{}': {}", path, e);
            std::process::exit(1);
        }
    }
}

fn run_file(path: &str) {
    let content = read_file(path);
    let db = CompilerDb::new();
    let file = SourceFile::new(&db, PathBuf::from(path), content);

    // First check for parse errors
    let parse_result = parsed(&db, file);
    if parse_result.has_errors() {
        eprintln!("Parse errors in '{}':", path);
        for error in &parse_result.errors {
            eprintln!("  {}: {}", error.span, error.message);
        }
        std::process::exit(1);
    }

    // Then check for type errors
    let type_result = typed(&db, file);
    if type_result.has_errors() {
        eprintln!("Type errors in '{}':", path);
        for error in &type_result.errors {
            eprintln!("  {:?}", error);
        }
        std::process::exit(1);
    }

    // Finally, evaluate
    let eval_result = evaluated(&db, file);
    if eval_result.is_failure() {
        eprintln!("Runtime error: {}", eval_result.error.unwrap_or_default());
        std::process::exit(1);
    }

    // Print the result if it's not void
    if let Some(result) = eval_result.result {
        use sigilc::EvalOutput;
        match result {
            EvalOutput::Void => {}
            _ => println!("{}", result.display()),
        }
    }
}

fn check_file(path: &str) {
    let content = read_file(path);
    let db = CompilerDb::new();
    let file = SourceFile::new(&db, PathBuf::from(path), content);

    // Check for parse errors
    let parse_result = parsed(&db, file);
    if parse_result.has_errors() {
        eprintln!("Parse errors in '{}':", path);
        for error in &parse_result.errors {
            eprintln!("  {}: {}", error.span, error.message);
        }
        std::process::exit(1);
    }

    // Check for type errors
    let type_result = typed(&db, file);
    if type_result.has_errors() {
        eprintln!("Type errors in '{}':", path);
        for error in &type_result.errors {
            eprintln!("  {:?}", error);
        }
        std::process::exit(1);
    }

    // Check test coverage: every function (except @main) must have at least one test
    let interner = db.interner();
    let main_name = interner.intern("main");

    // Collect all tested function names
    let mut tested_functions: std::collections::HashSet<sigilc::Name> = std::collections::HashSet::new();
    for test in &parse_result.module.tests {
        for target in &test.targets {
            tested_functions.insert(*target);
        }
    }

    // Find functions without tests
    let mut untested: Vec<String> = Vec::new();
    for func in &parse_result.module.functions {
        if func.name != main_name && !tested_functions.contains(&func.name) {
            untested.push(interner.lookup(func.name).to_string());
        }
    }

    if !untested.is_empty() {
        eprintln!("Coverage error in '{}':", path);
        eprintln!("  The following functions have no tests:");
        for name in &untested {
            eprintln!("    @{}", name);
        }
        eprintln!();
        eprintln!("  Every function (except @main) requires at least one test.");
        eprintln!("  Add tests using: @test_name tests @{} () -> void = ...", untested[0]);
        std::process::exit(1);
    }

    let func_count = parse_result.module.functions.len();
    let test_count = parse_result.module.tests.len();
    println!("OK: {} ({} functions, {} tests, 100% coverage)", path, func_count, test_count);
}

fn parse_file(path: &str) {
    let content = read_file(path);
    let db = CompilerDb::new();
    let file = SourceFile::new(&db, PathBuf::from(path), content);

    let parse_result = parsed(&db, file);

    println!("Parse result for '{}':", path);
    println!("  Functions: {}", parse_result.module.functions.len());
    println!("  Expressions: {}", parse_result.arena.expr_count());
    println!("  Errors: {}", parse_result.errors.len());

    if !parse_result.module.functions.is_empty() {
        println!();
        println!("Functions:");
        for func in &parse_result.module.functions {
            let name = db.interner().lookup(func.name);
            let params = parse_result.arena.get_params(func.params);
            let param_names: Vec<_> = params.iter()
                .map(|p| db.interner().lookup(p.name))
                .collect();
            println!("  @{} ({})", name, param_names.join(", "));
        }
    }

    if !parse_result.errors.is_empty() {
        println!();
        println!("Errors:");
        for error in &parse_result.errors {
            println!("  {}: {}", error.span, error.message);
        }
    }
}

fn lex_file(path: &str) {
    let content = read_file(path);
    let db = CompilerDb::new();
    let file = SourceFile::new(&db, PathBuf::from(path), content);

    let toks = tokens(&db, file);

    println!("Tokens for '{}' ({} tokens):", path, toks.len());
    for tok in toks.iter() {
        println!("  {:?} @ {}", tok.kind, tok.span);
    }
}

fn run_tests(path: &str, config: TestRunnerConfig) {
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
        std::process::exit(if report.is_complete() { 0 } else { 1 });
    }

    let summary = runner.run(path);

    // Print results
    print_test_summary(&summary, config.verbose);

    // Exit with appropriate code
    std::process::exit(summary.exit_code());
}

fn print_coverage_report(report: &sigilc::test::CoverageReport) {
    println!("Coverage Report");
    println!("===============");
    println!();

    if report.total == 0 {
        println!("No functions found.");
        return;
    }

    // Print covered functions
    let covered: Vec<_> = report.functions.iter().filter(|f| f.has_tests).collect();
    if !covered.is_empty() {
        println!("Covered ({}):", covered.len());
        for func in covered {
            let tests = func.test_names.join(", ");
            println!("  @{} <- {}", func.name, tests);
        }
        println!();
    }

    // Print uncovered functions
    let uncovered: Vec<_> = report.functions.iter().filter(|f| !f.has_tests).collect();
    if !uncovered.is_empty() {
        println!("Uncovered ({}):", uncovered.len());
        for func in uncovered {
            println!("  @{}", func.name);
        }
        println!();
    }

    // Print summary
    println!("Summary: {}/{} functions covered ({:.1}%)",
        report.covered, report.total, report.percentage());

    if report.is_complete() {
        println!("\nOK");
    } else {
        println!("\nMISSING COVERAGE");
    }
}

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
                println!("  ERROR: {}", error);
            }
            continue;
        }

        if verbose || file.has_failures() {
            println!("\n{}", file.path.display());
        }

        for result in &file.results {
            let status = match &result.outcome {
                sigilc::TestOutcome::Passed => {
                    if verbose {
                        format!("  PASS: {} ({:.2?})", result.name, result.duration)
                    } else {
                        continue;
                    }
                }
                sigilc::TestOutcome::Failed(msg) => {
                    format!("  FAIL: {} - {}", result.name, msg)
                }
                sigilc::TestOutcome::Skipped(reason) => {
                    if verbose {
                        format!("  SKIP: {} - {}", result.name, reason)
                    } else {
                        continue;
                    }
                }
            };
            println!("{}", status);
        }
    }

    // Print summary
    println!();
    println!("Test Summary:");
    println!(
        "  {} passed, {} failed, {} skipped ({} total)",
        summary.passed, summary.failed, summary.skipped, summary.total()
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
