//! Ori Compiler CLI
//!
//! Salsa-first incremental compiler.

use ori_diagnostic::{ErrorCode, ErrorDocs};
use oric::{CompilerDb, SourceFile, Db};
use oric::query::{tokens, parsed, typed, evaluated, codegen_c};
use oric::test::{TestRunner, TestRunnerConfig, TestSummary};
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
                eprintln!("Usage: ori run <file.ori>");
                std::process::exit(1);
            }
            run_file(&args[2]);
        }
        "test" => {
            // Parse args: path is optional, flags can come before or after
            let mut path: Option<String> = None;
            let mut config = TestRunnerConfig::default();

            for arg in args.iter().skip(2) {
                if let Some(filter) = arg.strip_prefix("--filter=") {
                    config.filter = Some(filter.to_string());
                } else if arg == "--verbose" || arg == "-v" {
                    config.verbose = true;
                } else if arg == "--no-parallel" {
                    config.parallel = false;
                } else if arg == "--coverage" {
                    config.coverage = true;
                } else if arg == "--backend=llvm" {
                    config.backend = oric::test::Backend::LLVM;
                } else if arg == "--backend=interpreter" {
                    config.backend = oric::test::Backend::Interpreter;
                } else if !arg.starts_with('-') && path.is_none() {
                    path = Some(arg.clone());
                }
            }

            // Use provided path or current directory
            let path = path.unwrap_or_else(|| ".".to_string());
            run_tests(&path, &config);
        }
        "check" => {
            if args.len() < 3 {
                eprintln!("Usage: ori check <file.ori>");
                std::process::exit(1);
            }
            check_file(&args[2]);
        }
        "compile" => {
            if args.len() < 3 {
                eprintln!("Usage: ori compile <file.ori> [-o output.c]");
                std::process::exit(1);
            }
            let input_file = &args[2];
            let output_file = if args.len() >= 5 && args[3] == "-o" {
                args[4].clone()
            } else {
                // Default: replace .ori with .c
                let path = Path::new(input_file);
                path.with_extension("c")
                    .to_string_lossy()
                    .to_string()
            };
            compile_file(input_file, &output_file);
        }
        "parse" => {
            if args.len() < 3 {
                eprintln!("Usage: ori parse <file.ori>");
                std::process::exit(1);
            }
            parse_file(&args[2]);
        }
        "lex" => {
            if args.len() < 3 {
                eprintln!("Usage: ori lex <file.ori>");
                std::process::exit(1);
            }
            lex_file(&args[2]);
        }
        "help" | "--help" | "-h" => {
            print_usage();
        }
        "version" | "--version" | "-v" => {
            println!("Ori Compiler 0.1.0-alpha.1");
            println!("Salsa-first incremental compilation");
        }
        "--explain" | "explain" => {
            if args.len() < 3 {
                eprintln!("Usage: ori --explain <ERROR_CODE>");
                eprintln!("Example: ori --explain E2001");
                std::process::exit(1);
            }
            explain_error(&args[2]);
        }
        _ => {
            // If it looks like a file path, try to run it
            if std::path::Path::new(command)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("si"))
            {
                run_file(command);
            } else {
                eprintln!("Unknown command: {command}");
                eprintln!();
                print_usage();
                std::process::exit(1);
            }
        }
    }
}

fn print_usage() {
    println!("Ori Compiler (Salsa-first incremental compilation)");
    println!();
    println!("Usage: ori <command> [options]");
    println!();
    println!("Commands:");
    println!("  run <file.ori>       Run/evaluate a Ori program");
    println!("  test [path]         Run tests (default: current directory)");
    println!("  check <file.ori>     Type check a file (no execution)");
    println!("  compile <file.ori>   Generate C code (-o output.c)");
    println!("  parse <file.ori>     Parse and display AST info");
    println!("  lex <file.ori>       Tokenize and display tokens");
    println!("  --explain <code>    Explain an error code (e.g., E2001)");
    println!("  help                Show this help message");
    println!("  version             Show version information");
    println!();
    println!("Test options:");
    println!("  --filter=<pattern>  Only run tests matching pattern");
    println!("  --verbose, -v       Show detailed output");
    println!("  --no-parallel       Run tests sequentially");
    println!("  --backend=<name>    Use backend: interpreter (default), llvm");
    println!();
    println!("Examples:");
    println!("  ori run main.ori");
    println!("  ori test                    # Run all spec tests");
    println!("  ori test tests/spec/patterns/");
    println!("  ori test --filter=map");
    println!("  ori check lib.ori");
    println!("  ori compile main.ori -o out.c");
    println!("  ori --explain E2001         # Explain type mismatch");
    println!("  ori main.ori       (shorthand for 'run')");
}

fn explain_error(code_str: &str) {
    let Some(code) = parse_error_code(code_str) else {
        eprintln!("Unknown error code: {code_str}");
        eprintln!();
        eprintln!("Error codes have the format EXXXX where X is a digit.");
        eprintln!("Examples: E0001, E1001, E2001");
        std::process::exit(1);
    };

    if let Some(doc) = ErrorDocs::get(code) {
        println!("{doc}");
    } else {
        eprintln!("No documentation available for {code_str}");
        eprintln!();
        eprintln!("This error code exists but doesn't have detailed documentation yet.");
        eprintln!("Please check the error message for guidance.");
        std::process::exit(1);
    }
}

fn parse_error_code(s: &str) -> Option<ErrorCode> {
    match s.to_uppercase().as_str() {
        // Lexer errors
        "E0001" => Some(ErrorCode::E0001),
        "E0002" => Some(ErrorCode::E0002),
        "E0003" => Some(ErrorCode::E0003),
        "E0004" => Some(ErrorCode::E0004),
        "E0005" => Some(ErrorCode::E0005),
        // Parser errors
        "E1001" => Some(ErrorCode::E1001),
        "E1002" => Some(ErrorCode::E1002),
        "E1003" => Some(ErrorCode::E1003),
        "E1004" => Some(ErrorCode::E1004),
        "E1005" => Some(ErrorCode::E1005),
        "E1006" => Some(ErrorCode::E1006),
        "E1007" => Some(ErrorCode::E1007),
        "E1008" => Some(ErrorCode::E1008),
        "E1009" => Some(ErrorCode::E1009),
        "E1010" => Some(ErrorCode::E1010),
        "E1011" => Some(ErrorCode::E1011),
        "E1012" => Some(ErrorCode::E1012),
        "E1013" => Some(ErrorCode::E1013),
        "E1014" => Some(ErrorCode::E1014),
        // Type errors
        "E2001" => Some(ErrorCode::E2001),
        "E2002" => Some(ErrorCode::E2002),
        "E2003" => Some(ErrorCode::E2003),
        "E2004" => Some(ErrorCode::E2004),
        "E2005" => Some(ErrorCode::E2005),
        "E2006" => Some(ErrorCode::E2006),
        "E2007" => Some(ErrorCode::E2007),
        "E2008" => Some(ErrorCode::E2008),
        "E2009" => Some(ErrorCode::E2009),
        "E2010" => Some(ErrorCode::E2010),
        "E2011" => Some(ErrorCode::E2011),
        "E2012" => Some(ErrorCode::E2012),
        "E2013" => Some(ErrorCode::E2013),
        "E2014" => Some(ErrorCode::E2014),
        // Pattern errors
        "E3001" => Some(ErrorCode::E3001),
        "E3002" => Some(ErrorCode::E3002),
        "E3003" => Some(ErrorCode::E3003),
        // Internal errors
        "E9001" => Some(ErrorCode::E9001),
        "E9002" => Some(ErrorCode::E9002),
        _ => None,
    }
}

fn read_file(path: &str) -> String {
    match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("Error reading '{path}': {e}");
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
        eprintln!("Parse errors in '{path}':");
        for error in &parse_result.errors {
            eprintln!("  {}: {}", error.span, error.message);
        }
        std::process::exit(1);
    }

    // Then check for type errors
    let type_result = typed(&db, file);
    if type_result.has_errors() {
        eprintln!("Type errors in '{path}':");
        for error in &type_result.errors {
            eprintln!("  {error:?}");
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
        use oric::EvalOutput;
        match result {
            EvalOutput::Void => {}
            _ => println!("{}", result.display()),
        }
    }
}

fn compile_file(input_path: &str, output_path: &str) {
    let content = read_file(input_path);
    let db = CompilerDb::new();
    let file = SourceFile::new(&db, PathBuf::from(input_path), content);

    // Generate C code
    let result = codegen_c(&db, file);

    if result.has_errors() {
        eprintln!("Compilation errors:");
        for error in &result.errors {
            eprintln!("  {}", error.message);
        }
        std::process::exit(1);
    }

    // Write output
    if let Err(e) = std::fs::write(output_path, &result.code) {
        eprintln!("Error writing '{output_path}': {e}");
        std::process::exit(1);
    }

    println!("Generated: {output_path}");
}

fn check_file(path: &str) {
    let content = read_file(path);
    let db = CompilerDb::new();
    let file = SourceFile::new(&db, PathBuf::from(path), content);

    // Check for parse errors
    let parse_result = parsed(&db, file);
    if parse_result.has_errors() {
        eprintln!("Parse errors in '{path}':");
        for error in &parse_result.errors {
            eprintln!("  {}: {}", error.span, error.message);
        }
        std::process::exit(1);
    }

    // Check for type errors
    let type_result = typed(&db, file);
    if type_result.has_errors() {
        eprintln!("Type errors in '{path}':");
        for error in &type_result.errors {
            eprintln!("  {error:?}");
        }
        std::process::exit(1);
    }

    // Check test coverage: every function (except @main) must have at least one test
    let interner = db.interner();
    let main_name = interner.intern("main");

    // Collect all tested function names
    let mut tested_functions: std::collections::HashSet<oric::Name> = std::collections::HashSet::new();
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
        eprintln!("Coverage error in '{path}':");
        eprintln!("  The following functions have no tests:");
        for name in &untested {
            eprintln!("    @{name}");
        }
        eprintln!();
        eprintln!("  Every function (except @main) requires at least one test.");
        eprintln!("  Add tests using: @test_name tests @{} () -> void = ...", untested[0]);
        std::process::exit(1);
    }

    let func_count = parse_result.module.functions.len();
    let test_count = parse_result.module.tests.len();
    println!("OK: {path} ({func_count} functions, {test_count} tests, 100% coverage)");
}

fn parse_file(path: &str) {
    let content = read_file(path);
    let db = CompilerDb::new();
    let file = SourceFile::new(&db, PathBuf::from(path), content);

    let parse_result = parsed(&db, file);

    println!("Parse result for '{path}':");
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

fn run_tests(path: &str, config: &TestRunnerConfig) {
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

fn print_coverage_report(report: &oric::test::CoverageReport) {
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
                println!("  ERROR: {error}");
            }
            continue;
        }

        if verbose || file.has_failures() {
            println!("\n{}", file.path.display());
        }

        for result in &file.results {
            let status = match &result.outcome {
                oric::TestOutcome::Passed => {
                    if verbose {
                        format!("  PASS: {} ({:.2?})", result.name, result.duration)
                    } else {
                        continue;
                    }
                }
                oric::TestOutcome::Failed(msg) => {
                    format!("  FAIL: {} - {}", result.name, msg)
                }
                oric::TestOutcome::Skipped(reason) => {
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
