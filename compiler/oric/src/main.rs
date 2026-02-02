//! Ori Compiler CLI
//!
//! Salsa-first incremental compiler.

use oric::commands::{
    add_target, build_file, check_file, demangle_symbol, explain_error, lex_file,
    list_installed_targets, list_targets, parse_build_options, parse_file, remove_target, run_file,
    run_file_compiled, run_format, run_tests, BuildOptions, TargetSubcommand,
};
use oric::test::TestRunnerConfig;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        print_usage();
        return;
    }

    let command = &args[1];

    match command.as_str() {
        "build" => {
            if args.len() < 3 {
                eprintln!("Usage: ori build <file.ori> [options]");
                eprintln!();
                eprintln!("Options:");
                eprintln!("  --release           Build with optimizations (O2, no debug)");
                eprintln!("  --target=<triple>   Target triple (default: native)");
                eprintln!("  --opt=<level>       Optimization: 0, 1, 2, 3, s, z");
                eprintln!("  --debug=<level>     Debug info: 0, 1, 2");
                eprintln!("  -o <path>           Output file");
                eprintln!("  --emit=<type>       Emit: obj, llvm-ir, llvm-bc, asm");
                eprintln!("  --lib               Build static library");
                eprintln!("  --dylib             Build shared library");
                eprintln!("  --wasm              Build for WebAssembly");
                eprintln!("  -v, --verbose       Verbose output");
                std::process::exit(1);
            }

            // Parse options, handling -o specially (needs lookahead)
            let mut options = BuildOptions::default();
            let mut i = 3;
            while i < args.len() {
                if args[i] == "-o" && i + 1 < args.len() {
                    options.output = Some(std::path::PathBuf::from(&args[i + 1]));
                    i += 2;
                } else {
                    let parsed = parse_build_options(&args[i..=i]);
                    options.merge(&parsed);
                    i += 1;
                }
            }

            build_file(&args[2], &options);
        }
        "run" => {
            if args.len() < 3 {
                eprintln!("Usage: ori run <file.ori> [--compile]");
                eprintln!();
                eprintln!("Options:");
                eprintln!("  --compile    AOT compile before running (faster repeated runs)");
                std::process::exit(1);
            }

            // Parse run options
            let mut compile_mode = false;
            let mut file_path = None;

            for arg in args.iter().skip(2) {
                if arg == "--compile" || arg == "-c" {
                    compile_mode = true;
                } else if !arg.starts_with('-') && file_path.is_none() {
                    file_path = Some(arg.as_str());
                }
            }

            let Some(path) = file_path else {
                eprintln!("error: missing file path");
                eprintln!("Usage: ori run <file.ori> [--compile]");
                std::process::exit(1);
            };

            if compile_mode {
                run_file_compiled(path);
            } else {
                run_file(path);
            }
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
        "target" => {
            if args.len() < 3 {
                eprintln!("Usage: ori target <subcommand> [target]");
                eprintln!();
                eprintln!("Subcommands:");
                eprintln!("  list             List installed targets");
                eprintln!("  add <target>     Install a target's sysroot");
                eprintln!("  remove <target>  Remove a target's sysroot");
                eprintln!();
                eprintln!("Examples:");
                eprintln!("  ori target list");
                eprintln!("  ori target add wasm32-wasi");
                eprintln!("  ori target remove wasm32-wasi");
                std::process::exit(1);
            }

            let Some(subcommand) = TargetSubcommand::parse(&args[2]) else {
                eprintln!("error: unknown subcommand '{}'", args[2]);
                eprintln!("Valid subcommands: list, add, remove");
                std::process::exit(1);
            };

            match subcommand {
                TargetSubcommand::List => {
                    list_installed_targets();
                }
                TargetSubcommand::Add => {
                    if args.len() < 4 {
                        eprintln!("error: missing target name");
                        eprintln!("Usage: ori target add <target>");
                        eprintln!();
                        eprintln!("Run `ori targets` to see available targets.");
                        std::process::exit(1);
                    }
                    add_target(&args[3]);
                }
                TargetSubcommand::Remove => {
                    if args.len() < 4 {
                        eprintln!("error: missing target name");
                        eprintln!("Usage: ori target remove <target>");
                        eprintln!();
                        eprintln!("Run `ori target list` to see installed targets.");
                        std::process::exit(1);
                    }
                    remove_target(&args[3]);
                }
            }
        }
        "targets" => {
            let installed_only = args.iter().any(|a| a == "--installed");
            list_targets(installed_only);
        }
        "demangle" => {
            if args.len() < 3 {
                eprintln!("Usage: ori demangle <symbol>");
                eprintln!("Example: ori demangle _ori_MyModule_foo");
                std::process::exit(1);
            }
            demangle_symbol(&args[2]);
        }
        "help" | "--help" | "-h" => {
            print_usage();
        }
        "version" | "--version" | "-v" => {
            println!("Ori Compiler {}", env!("CARGO_PKG_VERSION"));
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
    println!("  build <file.ori>     Compile to native executable (AOT)");
    println!("  test [path]          Run tests (default: current directory)");
    println!("  check <file.ori>     Type check a file (no execution)");
    println!("  fmt [paths...]       Format Ori source files");
    println!("  target <subcommand>  Manage cross-compilation targets");
    println!("  targets              List supported compilation targets");
    println!("  demangle <symbol>    Demangle an Ori symbol name");
    println!("  parse <file.ori>     Parse and display AST info");
    println!("  lex <file.ori>       Tokenize and display tokens");
    println!("  --explain <code>     Explain an error code (e.g., E2001)");
    println!("  help                 Show this help message");
    println!("  version              Show version information");
    println!();
    println!("Run options:");
    println!("  --compile, -c       AOT compile before running (faster repeated runs)");
    println!();
    println!("Build options:");
    println!("  --release           Build with optimizations (O2, no debug)");
    println!("  --target=<triple>   Target triple (default: native)");
    println!("  --opt=<level>       Optimization: 0, 1, 2, 3, s, z");
    println!("  --debug=<level>     Debug info: 0, 1, 2");
    println!("  -o <path>           Output file path");
    println!("  --emit=<type>       Emit: obj, llvm-ir, llvm-bc, asm");
    println!("  --lib               Build static library");
    println!("  --dylib             Build shared library");
    println!("  --wasm              Build for WebAssembly");
    println!("  --lto=<mode>        Link-time optimization: off, thin, full");
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
    println!("Target subcommands:");
    println!("  list                List installed cross-compilation targets");
    println!("  add <target>        Install a target's sysroot");
    println!("  remove <target>     Remove a target's sysroot");
    println!();
    println!("Examples:");
    println!("  ori run main.ori");
    println!("  ori run main.ori --compile      # AOT compile for faster runs");
    println!("  ori build main.ori              # Compile to ./build/debug/main");
    println!("  ori build main.ori --release    # Optimized build");
    println!("  ori build main.ori -o myapp     # Custom output name");
    println!("  ori build main.ori --wasm       # WebAssembly output");
    println!("  ori test                        # Run all spec tests");
    println!("  ori test tests/spec/patterns/");
    println!("  ori test --filter=map");
    println!("  ori check lib.ori");
    println!("  ori targets                     # List supported targets");
    println!("  ori target list                 # List installed targets");
    println!("  ori target add wasm32-wasi      # Install WASI target");
    println!("  ori demangle _ori_main          # Decode mangled symbol");
    println!("  ori fmt                         # Format all files");
    println!("  ori fmt --check                 # Check formatting (for CI)");
    println!("  ori --explain E2001             # Explain type mismatch");
}
