use sigilc::{lexer, ast, parser, types, eval, codegen};

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;
use ast::{Item, Module};
use rayon::prelude::*;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_usage();
        std::process::exit(1);
    }

    match args[1].as_str() {
        "run" | "--run" => {
            if args.len() < 3 {
                eprintln!("Usage: sigil run <file.si>");
                std::process::exit(1);
            }
            run_file(&args[2]);
        }
        "build" | "--build" => {
            if args.len() < 3 {
                eprintln!("Usage: sigil build <file.si> [-o output]");
                std::process::exit(1);
            }
            let output = get_output_name(&args, 3);
            build_file(&args[2], &output);
        }
        "emit" | "--emit" => {
            if args.len() < 3 {
                eprintln!("Usage: sigil emit <file.si> [-o output.c]");
                std::process::exit(1);
            }
            let output = get_output_name(&args, 3);
            emit_file(&args[2], &output);
        }
        "test" | "--test" => {
            if args.len() < 3 {
                // No file specified - discover and run all tests
                test_all();
            } else {
                test_file(&args[2]);
            }
        }
        "check" | "--check" => {
            if args.len() < 3 {
                eprintln!("Usage: sigil check <file.si>");
                std::process::exit(1);
            }
            check_coverage(&args[2]);
        }
        "--repl" | "repl" => {
            println!("Sigil REPL v0.1.0");
            println!("Type :help for help, :quit to exit\n");
            repl();
        }
        "--help" | "-h" | "help" => {
            print_usage();
        }
        // Default: treat as file path for backwards compatibility
        path if path.ends_with(".si") => {
            run_file(path);
        }
        _ => {
            eprintln!("Unknown command: {}", args[1]);
            print_usage();
            std::process::exit(1);
        }
    }
}

fn print_usage() {
    eprintln!("Sigil v0.1.0");
    eprintln!();
    eprintln!("Usage:");
    eprintln!("  sigil run <file.si>              Interpret and run");
    eprintln!("  sigil build <file.si> [-o out]   Compile to native binary");
    eprintln!("  sigil emit <file.si> [-o out.c]  Emit C code");
    eprintln!("  sigil test                       Run all tests (parallel)");
    eprintln!("  sigil test <file.si>             Run tests for specific file");
    eprintln!("  sigil check <file.si>            Check test coverage");
    eprintln!("  sigil repl                       Start REPL");
    eprintln!("  sigil <file.si>                  Run file (shorthand)");
}

fn get_output_name(args: &[String], start: usize) -> String {
    for i in start..args.len() {
        if args[i] == "-o" && i + 1 < args.len() {
            return args[i + 1].clone();
        }
    }
    // Default: input file name without extension
    args.get(2)
        .map(|p| p.trim_end_matches(".si").to_string())
        .unwrap_or_else(|| "output".to_string())
}

fn run_file(path: &str) {
    let source = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error reading file '{}': {}", path, e);
            std::process::exit(1);
        }
    };

    if let Err(e) = run(&source, path) {
        eprintln!("{}", e);
        std::process::exit(1);
    }
}

fn emit_file(path: &str, output: &str) {
    let source = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error reading file '{}': {}", path, e);
            std::process::exit(1);
        }
    };

    match compile_to_c(&source, path) {
        Ok(c_code) => {
            let out_path = if output.ends_with(".c") {
                output.to_string()
            } else {
                format!("{}.c", output)
            };
            if let Err(e) = fs::write(&out_path, &c_code) {
                eprintln!("Error writing file '{}': {}", out_path, e);
                std::process::exit(1);
            }
            println!("Emitted: {}", out_path);
        }
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    }
}

fn build_file(path: &str, output: &str) {
    let source = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error reading file '{}': {}", path, e);
            std::process::exit(1);
        }
    };

    // Generate C code
    let c_code = match compile_to_c(&source, path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    };

    // Write temporary C file
    let c_path = format!("{}.c", output);
    if let Err(e) = fs::write(&c_path, &c_code) {
        eprintln!("Error writing file '{}': {}", c_path, e);
        std::process::exit(1);
    }

    // Compile with gcc/cc
    let compiler = if cfg!(target_os = "windows") { "gcc" } else { "cc" };
    let status = Command::new(compiler)
        .args([&c_path, "-o", output, "-O2"])
        .status();

    match status {
        Ok(s) if s.success() => {
            // Clean up temporary C file
            let _ = fs::remove_file(&c_path);
            println!("Built: {}", output);
        }
        Ok(s) => {
            eprintln!("Compiler exited with: {}", s);
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("Failed to run compiler '{}': {}", compiler, e);
            eprintln!("C file left at: {}", c_path);
            std::process::exit(1);
        }
    }
}

fn compile_to_c(source: &str, filename: &str) -> Result<String, String> {
    // Step 1: Tokenize
    let tokens = lexer::tokenize(source, filename)?;

    // Step 2: Parse
    let ast = parser::parse(tokens, filename)?;

    // Step 3: Type check
    let typed_ast = types::check(ast)?;

    // Step 4: Generate C
    codegen::generate(&typed_ast)
}

fn run(source: &str, filename: &str) -> Result<(), String> {
    // Step 1: Tokenize
    let tokens = lexer::tokenize(source, filename)?;

    // Step 2: Parse
    let ast = parser::parse(tokens, filename)?;

    // Step 3: Type check
    let typed_ast = types::check(ast)?;

    // Step 4: Evaluate
    eval::run(typed_ast)?;

    Ok(())
}

fn repl() {
    use std::io::{self, Write};

    let mut env = eval::Environment::new();

    loop {
        print!("> ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_err() {
            break;
        }

        let input = input.trim();

        match input {
            ":quit" | ":q" => break,
            ":help" | ":h" => {
                println!("Commands:");
                println!("  :quit, :q   Exit REPL");
                println!("  :help, :h   Show this help");
                println!("  :type <expr> Show type of expression");
            }
            _ if input.starts_with(":type ") => {
                let expr = &input[6..];
                match eval::type_of(expr, &env) {
                    Ok(t) => println!("{}", t),
                    Err(e) => eprintln!("Error: {}", e),
                }
            }
            _ if input.is_empty() => continue,
            _ => {
                match eval::eval_line(input, &mut env) {
                    Ok(result) => {
                        if !result.is_empty() {
                            println!("{}", result);
                        }
                    }
                    Err(e) => eprintln!("Error: {}", e),
                }
            }
        }
    }
}

fn get_test_file_path(source_path: &str) -> String {
    let path = Path::new(source_path);
    let parent = path.parent().unwrap_or(Path::new("."));
    let stem = path.file_stem().unwrap().to_str().unwrap();

    // Try _test/name.test.si first
    let test_dir = parent.join("_test");
    let test_file = test_dir.join(format!("{}.test.si", stem));
    if test_file.exists() {
        return test_file.to_str().unwrap().to_string();
    }

    // Fall back to name.test.si in same directory
    let test_file = parent.join(format!("{}.test.si", stem));
    test_file.to_str().unwrap().to_string()
}

fn parse_file(path: &str) -> Result<Module, String> {
    let source = fs::read_to_string(path)
        .map_err(|e| format!("Error reading '{}': {}", path, e))?;
    let tokens = lexer::tokenize(&source, path)?;
    parser::parse(tokens, path)
}

fn get_functions(module: &Module) -> Vec<String> {
    module.items.iter().filter_map(|item| {
        if let Item::Function(f) = item {
            Some(f.name.clone())
        } else {
            None
        }
    }).collect()
}

fn get_tested_functions(module: &Module) -> Vec<String> {
    module.items.iter().filter_map(|item| {
        if let Item::Test(t) = item {
            Some(t.target.clone())
        } else {
            None
        }
    }).collect()
}

fn check_coverage(source_path: &str) {
    // Parse source file
    let source_module = match parse_file(source_path) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    };

    let functions = get_functions(&source_module);
    let test_path = get_test_file_path(source_path);

    // Parse test file
    let tested = if Path::new(&test_path).exists() {
        match parse_file(&test_path) {
            Ok(m) => get_tested_functions(&m),
            Err(e) => {
                eprintln!("Error parsing test file: {}", e);
                std::process::exit(1);
            }
        }
    } else {
        Vec::new()
    };

    // Check coverage
    let mut missing = Vec::new();
    for func in &functions {
        if func == "main" {
            continue; // main is exempt
        }
        if !tested.contains(func) {
            missing.push(func.clone());
        }
    }

    if missing.is_empty() {
        println!("✓ All functions have tests");
        println!("  {} functions, {} tests", functions.len(), tested.len());
    } else {
        eprintln!("✗ Missing tests for {} function(s):", missing.len());
        for func in &missing {
            eprintln!("  - @{}", func);
        }
        eprintln!();
        eprintln!("Create tests in: {}", test_path);
        eprintln!("Example:");
        eprintln!("  @test_{} tests @{} () -> void = run(", missing[0], missing[0]);
        eprintln!("      assert({}(...) == expected)", missing[0]);
        eprintln!("  )");
        std::process::exit(1);
    }
}

fn resolve_import_path(from_file: &str, import_path: &[String]) -> String {
    let from = Path::new(from_file);
    let dir = from.parent().unwrap_or(Path::new("."));

    // Convert dot-separated path to file path
    // e.g., "math" -> "../math.si" (relative to _test folder)
    // e.g., "utils.helpers" -> "../utils/helpers.si"
    let mut path = dir.to_path_buf();

    // Go up one level if we're in _test folder
    if dir.ends_with("_test") {
        path = path.parent().unwrap_or(Path::new(".")).to_path_buf();
    }

    for (i, segment) in import_path.iter().enumerate() {
        if i == import_path.len() - 1 {
            path = path.join(format!("{}.si", segment));
        } else {
            path = path.join(segment);
        }
    }

    path.to_str().unwrap().to_string()
}

fn load_imports(test_module: &Module, test_path: &str) -> Result<Vec<Item>, String> {
    let mut imported_items = Vec::new();

    for item in &test_module.items {
        if let Item::Use(use_def) = item {
            let source_path = resolve_import_path(test_path, &use_def.path);

            if !Path::new(&source_path).exists() {
                return Err(format!(
                    "Cannot find module '{}' (looked for {})",
                    use_def.path.join("."),
                    source_path
                ));
            }

            let source_module = parse_file(&source_path)?;

            // Import specified items or all if wildcard
            for use_item in &use_def.items {
                if use_item.name == "*" {
                    // Import all functions and configs
                    for item in &source_module.items {
                        match item {
                            Item::Function(_) | Item::Config(_) => {
                                imported_items.push(item.clone());
                            }
                            _ => {}
                        }
                    }
                } else {
                    // Import specific item
                    let found = source_module.items.iter().find(|item| {
                        match item {
                            Item::Function(f) => f.name == use_item.name,
                            Item::Config(c) => c.name == use_item.name,
                            _ => false,
                        }
                    });

                    if let Some(item) = found {
                        imported_items.push(item.clone());
                    } else {
                        return Err(format!(
                            "Cannot find '{}' in module '{}'",
                            use_item.name,
                            use_def.path.join(".")
                        ));
                    }
                }
            }
        }
    }

    Ok(imported_items)
}

fn test_file(test_path: &str) {
    // Parse test file
    let test_module = match parse_file(test_path) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Error parsing test file: {}", e);
            std::process::exit(1);
        }
    };

    // Check for imports
    let has_imports = test_module.items.iter().any(|i| matches!(i, Item::Use(_)));
    if !has_imports {
        eprintln!("Error: Test file must import the module being tested");
        eprintln!("Add: use module_name {{ function1, function2 }}");
        std::process::exit(1);
    }

    // Load imported modules
    let imported_items = match load_imports(&test_module, test_path) {
        Ok(items) => items,
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    };

    // Combine imported items with test items
    let mut combined_items = imported_items;
    combined_items.extend(test_module.items.clone());

    let combined = Module {
        name: test_module.name.clone(),
        items: combined_items,
    };

    // Type check
    let typed = match types::check(combined) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Type error: {}", e);
            std::process::exit(1);
        }
    };

    // Run tests
    let tests: Vec<_> = typed.items.iter().filter_map(|item| {
        if let Item::Test(t) = item {
            Some(t)
        } else {
            None
        }
    }).collect();

    if tests.is_empty() {
        eprintln!("No tests found in {}", test_path);
        std::process::exit(1);
    }

    println!("Running {} test(s)...\n", tests.len());

    let mut passed = 0;
    let mut failed = 0;

    for test in &tests {
        print!("  @{} tests @{} ... ", test.name, test.target);
        match eval::run_test(&typed, test) {
            Ok(()) => {
                println!("✓");
                passed += 1;
            }
            Err(e) => {
                println!("✗");
                eprintln!("    {}", e);
                failed += 1;
            }
        }
    }

    println!();
    if failed == 0 {
        println!("All {} test(s) passed!", passed);
    } else {
        eprintln!("{} passed, {} failed", passed, failed);
        std::process::exit(1);
    }
}

// ============================================================================
// Parallel Test Runner
// ============================================================================

/// Discover all test files in the current directory and subdirectories
fn discover_test_files() -> Vec<PathBuf> {
    let mut test_files = Vec::new();
    discover_test_files_recursive(Path::new("."), &mut test_files);
    test_files.sort();
    test_files
}

fn discover_test_files_recursive(dir: &Path, test_files: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();

        // Skip hidden directories and common non-source directories
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.starts_with('.') || name == "target" || name == "node_modules" {
                continue;
            }
        }

        if path.is_dir() {
            discover_test_files_recursive(&path, test_files);
        } else if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.ends_with(".test.si") {
                test_files.push(path);
            }
        }
    }
}

/// Result of running a single test file
struct TestFileResult {
    path: String,
    passed: usize,
    failed: usize,
    errors: Vec<String>,
}

/// Run a single test file and return results
fn run_test_file(test_path: &Path) -> TestFileResult {
    let path_str = test_path.to_string_lossy().to_string();

    // Parse test file
    let test_module = match parse_file(&path_str) {
        Ok(m) => m,
        Err(e) => {
            return TestFileResult {
                path: path_str,
                passed: 0,
                failed: 1,
                errors: vec![format!("Parse error: {}", e)],
            };
        }
    };

    // Check for imports
    let has_imports = test_module.items.iter().any(|i| matches!(i, Item::Use(_)));
    if !has_imports {
        return TestFileResult {
            path: path_str,
            passed: 0,
            failed: 1,
            errors: vec!["Test file must import the module being tested".to_string()],
        };
    }

    // Load imported modules
    let imported_items = match load_imports(&test_module, &path_str) {
        Ok(items) => items,
        Err(e) => {
            return TestFileResult {
                path: path_str,
                passed: 0,
                failed: 1,
                errors: vec![e],
            };
        }
    };

    // Combine imported items with test items
    let mut combined_items = imported_items;
    combined_items.extend(test_module.items.clone());

    let combined = Module {
        name: test_module.name.clone(),
        items: combined_items,
    };

    // Type check
    let typed = match types::check(combined) {
        Ok(t) => t,
        Err(e) => {
            return TestFileResult {
                path: path_str,
                passed: 0,
                failed: 1,
                errors: vec![format!("Type error: {}", e)],
            };
        }
    };

    // Run tests
    let tests: Vec<_> = typed.items.iter().filter_map(|item| {
        if let Item::Test(t) = item {
            Some(t)
        } else {
            None
        }
    }).collect();

    if tests.is_empty() {
        return TestFileResult {
            path: path_str,
            passed: 0,
            failed: 0,
            errors: vec!["No tests found".to_string()],
        };
    }

    let mut passed = 0;
    let mut failed = 0;
    let mut errors = Vec::new();

    for test in &tests {
        match eval::run_test(&typed, test) {
            Ok(()) => {
                passed += 1;
            }
            Err(e) => {
                errors.push(format!("@{}: {}", test.name, e));
                failed += 1;
            }
        }
    }

    TestFileResult {
        path: path_str,
        passed,
        failed,
        errors,
    }
}

/// Run all tests in parallel
fn test_all() {
    let start_time = Instant::now();

    // Discover test files
    let test_files = discover_test_files();

    if test_files.is_empty() {
        eprintln!("No test files found (*.test.si)");
        std::process::exit(1);
    }

    println!("Discovering tests...");
    println!("Found {} test file(s)\n", test_files.len());

    // Counters for progress
    let total_passed = AtomicUsize::new(0);
    let total_failed = AtomicUsize::new(0);
    let files_done = AtomicUsize::new(0);
    let total_files = test_files.len();

    // Run tests in parallel
    let results: Vec<TestFileResult> = test_files
        .par_iter()
        .map(|path| {
            let result = run_test_file(path);

            // Update counters
            total_passed.fetch_add(result.passed, Ordering::Relaxed);
            total_failed.fetch_add(result.failed, Ordering::Relaxed);
            let done = files_done.fetch_add(1, Ordering::Relaxed) + 1;

            // Print progress
            let status = if result.failed == 0 && result.errors.is_empty() {
                "✓"
            } else {
                "✗"
            };
            println!("[{}/{}] {} {} ({} passed)",
                done, total_files, status, result.path, result.passed);

            result
        })
        .collect();

    let elapsed = start_time.elapsed();
    let passed = total_passed.load(Ordering::Relaxed);
    let failed = total_failed.load(Ordering::Relaxed);

    // Print summary
    println!("\n{}", "=".repeat(60));

    // Print failures
    let failures: Vec<_> = results.iter()
        .filter(|r| !r.errors.is_empty())
        .collect();

    if !failures.is_empty() {
        println!("\nFailures:\n");
        for result in &failures {
            println!("  {}", result.path);
            for error in &result.errors {
                println!("    {}", error);
            }
        }
        println!();
    }

    // Print final summary
    println!("Test Results: {} passed, {} failed ({:.2}s)",
        passed, failed, elapsed.as_secs_f64());

    if failed > 0 {
        std::process::exit(1);
    }
}
