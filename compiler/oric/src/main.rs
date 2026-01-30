//! Ori Compiler CLI
//!
//! Salsa-first incremental compiler.

use oric::test::TestRunnerConfig;

mod commands;
use commands::{check_file, explain_error, lex_file, parse_file, run_file, run_format, run_tests};

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
        "fmt" => {
            run_format(&args[2..]);
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
    println!("  run <file.ori>       Run/evaluate an Ori program");
    println!("  test [path]          Run tests (default: current directory)");
    println!("  check <file.ori>     Type check a file (no execution)");
    println!("  fmt [paths...]       Format Ori source files");
    println!("  parse <file.ori>     Parse and display AST info");
    println!("  lex <file.ori>       Tokenize and display tokens");
    println!("  --explain <code>     Explain an error code (e.g., E2001)");
    println!("  help                 Show this help message");
    println!("  version              Show version information");
    println!();
    println!("Test options:");
    println!("  --filter=<pattern>  Only run tests matching pattern");
    println!("  --verbose, -v       Show detailed output");
    println!("  --no-parallel       Run tests sequentially");
    println!("  --backend=<name>    Use backend: interpreter (default), llvm");
    println!();
    println!("Format options:");
    println!("  --check             Check if files are formatted (exit 1 if not)");
    println!("  --diff              Show diff output instead of modifying files");
    println!();
    println!("Examples:");
    println!("  ori run main.ori");
    println!("  ori test                    # Run all spec tests");
    println!("  ori test tests/spec/patterns/");
    println!("  ori test --filter=map");
    println!("  ori check lib.ori");
    println!("  ori fmt                     # Format all files in current directory");
    println!("  ori fmt src/                # Format files in src/ directory");
    println!("  ori fmt --check             # Check formatting (for CI)");
    println!("  ori --explain E2001         # Explain type mismatch");
    println!("  ori main.ori                # Shorthand for 'run'");
}
