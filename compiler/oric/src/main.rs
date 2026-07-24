//! Ori Compiler CLI
//!
//! Salsa-first incremental compiler.

mod test_command;

use oric::commands::{
    accumulate_build_options, add_target, build_file, check_file, demangle_symbol,
    emit_aims_state_file, emit_scip_file, explain_error, explain_idx, lex_file,
    list_installed_targets, list_targets, parse_file, remove_target, run_file, run_file_compiled,
    run_format, watch_file, TargetFilter, TargetSubcommand, TestEnforcement,
};

#[cfg(not(feature = "llvm"))]
compile_error!(
    "oric requires the `llvm` feature (enabled by default). \
     Build with: cargo build -p oric"
);

/// Stack size for the main work thread (32 MiB).
///
/// The OS default stack is 8 MiB on macOS, which is insufficient for LLVM's
/// aarch64 backend during instruction selection (deep internal C++ recursion
/// through FFI that `ensure_sufficient_stack` cannot wrap). Spawning on a
/// larger thread follows the `rustc` pattern (`rustc_driver::main`).
const STACK_SIZE: usize = 32 * 1024 * 1024;

fn main() {
    let builder = std::thread::Builder::new()
        .name("ori-main".into())
        .stack_size(STACK_SIZE);

    let handle = builder.spawn(real_main).unwrap_or_else(|e| {
        eprintln!("error: failed to spawn main thread: {e}");
        std::process::exit(1);
    });

    if let Err(payload) = handle.join() {
        std::panic::resume_unwind(payload);
    }
}

fn real_main() {
    oric::tracing_setup::init();

    let args: Vec<String> = std::env::args().collect();
    let Some(command) = args.get(1) else {
        print_usage();
        return;
    };

    dispatch_command(command, &args);
}

fn dispatch_command(command: &str, args: &[String]) {
    match command {
        "build" => run_build_command(args),
        "run" => run_run_command(args),
        "test" => test_command::run(args),
        "check" => run_check_command(args),
        "emit-scip" => run_emit_scip_command(args),
        "emit-aims-state" => run_emit_aims_state_command(args),
        "fmt" => run_format(&args[2..]),
        "parse" => run_parse_command(args),
        "lex" => run_lex_command(args),
        "target" => run_target_command(args),
        "targets" => run_targets_command(args),
        "demangle" => run_demangle_command(args),
        "help" | "--help" | "-h" => print_usage(),
        "version" | "--version" | "-v" => print_version(),
        "watch" => run_watch_command(args),
        "--explain" | "explain" => run_explain_command(args),
        _ => run_path_or_unknown(command),
    }
}

fn run_check_command(args: &[String]) {
    let (path, enforcement) = parse_enforced_file_command(args, "check");
    check_file(path, enforcement);
}

fn run_watch_command(args: &[String]) {
    let (path, enforcement) = parse_enforced_file_command(args, "watch");
    watch_file(path, enforcement);
}

fn parse_enforced_file_command<'a>(
    args: &'a [String],
    command: &str,
) -> (&'a str, TestEnforcement) {
    if args.len() < 3 {
        print_enforced_file_usage(command);
        std::process::exit(1);
    }

    let mut file_path = None;
    let mut enforcement = TestEnforcement::Off;
    for arg in args.iter().skip(2) {
        if let Some(value) = arg.strip_prefix("--test-enforcement=") {
            let Some(parsed) = TestEnforcement::parse_flag(value) else {
                eprintln!("error: invalid test enforcement level '{value}'");
                eprintln!("Valid values: off, warn, error");
                std::process::exit(1);
            };
            enforcement = parsed;
        } else if !arg.starts_with('-') && file_path.is_none() {
            file_path = Some(arg.as_str());
        }
    }

    let Some(path) = file_path else {
        eprintln!("error: missing file path");
        print_enforced_file_usage(command);
        std::process::exit(1);
    };
    (path, enforcement)
}

fn print_enforced_file_usage(command: &str) {
    eprintln!("Usage: ori {command} <file.ori> [--test-enforcement=off|warn|error]");
}

fn run_emit_scip_command(args: &[String]) {
    let (path, output) = parse_emit_command(args, "emit-scip", "index.scip");
    emit_scip_file(path, &output);
}

fn run_emit_aims_state_command(args: &[String]) {
    let (path, output) = parse_emit_command(args, "emit-aims-state", "aims-state.jsonl");
    emit_aims_state_file(path, &output);
}

fn parse_emit_command<'a>(
    args: &'a [String],
    command: &str,
    default_output: &str,
) -> (&'a str, String) {
    if args.len() < 3 {
        print_emit_usage(command);
        std::process::exit(1);
    }

    let mut file_path = None;
    let mut output = default_output.to_string();
    let mut iter = args.iter().skip(2);
    while let Some(arg) = iter.next() {
        if arg == "-o" || arg == "--output" {
            let Some(value) = iter.next() else {
                eprintln!("error: missing value for '{arg}'");
                std::process::exit(1);
            };
            output.clone_from(value);
        } else if !arg.starts_with('-') && file_path.is_none() {
            file_path = Some(arg.as_str());
        }
    }

    let Some(path) = file_path else {
        eprintln!("error: missing file path");
        print_emit_usage(command);
        std::process::exit(1);
    };
    (path, output)
}

fn print_emit_usage(command: &str) {
    eprintln!("Usage: ori {command} <file.ori> [-o <output>]");
}

fn run_parse_command(args: &[String]) {
    let Some(path) = args.get(2) else {
        eprintln!("Usage: ori parse <file.ori>");
        std::process::exit(1);
    };
    parse_file(path);
}

fn run_lex_command(args: &[String]) {
    let Some(path) = args.get(2) else {
        eprintln!("Usage: ori lex <file.ori>");
        std::process::exit(1);
    };
    lex_file(path);
}

fn run_target_command(args: &[String]) {
    let Some(raw_subcommand) = args.get(2) else {
        print_target_usage();
        std::process::exit(1);
    };
    let Some(subcommand) = TargetSubcommand::parse(raw_subcommand) else {
        eprintln!("error: unknown subcommand '{raw_subcommand}'");
        eprintln!("Valid subcommands: list, add, remove");
        std::process::exit(1);
    };

    match subcommand {
        TargetSubcommand::List => list_installed_targets(),
        TargetSubcommand::Add => run_target_add(args),
        TargetSubcommand::Remove => run_target_remove(args),
    }
}

fn print_target_usage() {
    eprintln!("Usage: ori target <subcommand> [target]");
    eprintln!();
    eprintln!("Subcommands:");
    eprintln!("  list             List installed targets");
    eprintln!("  add <target>     Install a target's sysroot");
    eprintln!("  remove <target>  Remove a target's sysroot");
    eprintln!();
    eprintln!("Examples:");
    eprintln!("  ori target list");
    eprintln!("  ori target add wasm32-unknown-wasip1");
    eprintln!("  ori target remove wasm32-unknown-wasip1");
}

fn run_target_add(args: &[String]) {
    let Some(target) = args.get(3) else {
        eprintln!("error: missing target name");
        eprintln!("Usage: ori target add <target>");
        eprintln!();
        eprintln!("Run `ori targets` to see available targets.");
        std::process::exit(1);
    };
    add_target(target);
}

fn run_target_remove(args: &[String]) {
    let Some(target) = args.get(3) else {
        eprintln!("error: missing target name");
        eprintln!("Usage: ori target remove <target>");
        eprintln!();
        eprintln!("Run `ori target list` to see installed targets.");
        std::process::exit(1);
    };
    remove_target(target);
}

fn run_targets_command(args: &[String]) {
    let filter = if args.iter().any(|arg| arg == "--installed") {
        TargetFilter::InstalledOnly
    } else {
        TargetFilter::All
    };
    list_targets(filter);
}

fn run_demangle_command(args: &[String]) {
    let Some(symbol) = args.get(2) else {
        eprintln!("Usage: ori demangle <symbol>");
        eprintln!("Example: ori demangle _ori_MyModule_foo");
        std::process::exit(1);
    };
    demangle_symbol(symbol);
}

fn print_version() {
    println!("Ori Compiler {}", oric::version::report_version());
}

fn run_explain_command(args: &[String]) {
    if args.get(2).map(String::as_str) == Some("idx") {
        explain_idx(&args[3..]);
        return;
    }

    let Some(error_code) = args.get(2) else {
        eprintln!("Usage: ori --explain <ERROR_CODE>");
        eprintln!("Example: ori --explain E2001");
        eprintln!("         ori explain idx <index> <file.ori>");
        std::process::exit(1);
    };
    explain_error(error_code);
}

fn run_path_or_unknown(command: &str) {
    if std::path::Path::new(command)
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("ori"))
    {
        run_file(command, false);
        return;
    }

    eprintln!("Unknown command: {command}");
    eprintln!();
    print_usage();
    std::process::exit(1);
}

fn run_build_command(args: &[String]) {
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
        eprintln!("  --wasm-opt           Run wasm-opt (requires Binaryen)");
        eprintln!("  -v, --verbose       Verbose output");
        std::process::exit(1);
    }
    let options = accumulate_build_options(args);
    build_file(&args[2], &options);
}

fn run_run_command(args: &[String]) {
    if args.len() < 3 {
        eprintln!("Usage: ori run <file.ori> [--compile] [--profile]");
        eprintln!();
        eprintln!("Options:");
        eprintln!("  --compile    AOT compile before running (faster repeated runs)");
        eprintln!("  --profile    Print evaluation performance counters");
        std::process::exit(1);
    }
    let compile_mode = args
        .iter()
        .skip(2)
        .any(|arg| arg == "--compile" || arg == "-c");
    let profile = args.iter().skip(2).any(|arg| arg == "--profile");
    let Some(path) = args.iter().skip(2).find(|arg| !arg.starts_with('-')) else {
        eprintln!("error: missing file path");
        eprintln!("Usage: ori run <file.ori> [--compile] [--profile]");
        std::process::exit(1);
    };
    if compile_mode {
        run_file_compiled(path);
    } else {
        run_file(path, profile);
    }
}

fn print_usage() {
    println!("Ori Compiler {}", oric::version::report_version());
    println!();
    println!("Usage: ori <command> [options]");
    println!();
    println!("Commands:");
    println!("  run <file.ori>       Run/evaluate an Ori program");
    println!("  build <file.ori>     Compile to native executable (AOT)");
    println!("  test [paths...]      Run tests (default: current directory)");
    println!("  check <file.ori>     Type check a file (no execution)");
    println!("  emit-scip <file.ori> Emit a minimal SCIP index (index.scip)");
    println!("  emit-aims-state <file.ori> Emit per-function AIMS state JSONL (aims-state.jsonl)");
    println!("  watch <file.ori>     Watch and re-check on changes");
    println!("  fmt [paths...]       Format Ori source files");
    println!("  target <subcommand>  Manage cross-compilation targets");
    println!("  targets              List supported compilation targets");
    println!("  demangle <symbol>    Demangle an Ori symbol name");
    println!("  parse <file.ori>     Parse and display AST info");
    println!("  lex <file.ori>       Tokenize and display tokens");
    println!("  --explain <code>     Explain an error code (e.g., E2001)");
    println!("  explain idx <n> <f>  Trace the provenance DAG for one type-pool index in a file");
    println!("  help                 Show this help message");
    println!("  version              Show version information");
    println!();
    println!("Run options:");
    println!("  --compile, -c       AOT compile before running (faster repeated runs)");
    println!("  --profile           Print evaluation performance counters");
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
    println!("  --wasm-opt          Run the wasm-opt post-processor (requires Binaryen)");
    println!("  --lto=<mode>        Link-time optimization: off, thin, full");
    println!();
    println!("Check/Watch options:");
    println!("  --test-enforcement=<level>  Test enforcement: off (default), warn, error");
    println!();
    test_command::print_options();
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
    println!("  ori test --filter map");
    println!("  ori check lib.ori");
    println!("  ori check lib.ori --test-enforcement=error");
    println!("  ori targets                     # List supported targets");
    println!("  ori target list                 # List installed targets");
    println!("  ori target add wasm32-unknown-wasip1  # Install WASI Preview1 target");
    println!("  ori demangle _ori_main          # Decode mangled symbol");
    println!("  ori fmt                         # Format all files");
    println!("  ori fmt --check                 # Check formatting (for CI)");
    println!("  ori --explain E2001             # Explain type mismatch");
}
