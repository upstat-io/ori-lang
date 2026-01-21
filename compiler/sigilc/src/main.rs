// Sigil compiler CLI entry point

mod cli;

use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        cli::print_usage();
        std::process::exit(1);
    }

    match args[1].as_str() {
        "run" | "--run" => {
            if args.len() < 3 {
                eprintln!("Usage: sigil run <file.si>");
                std::process::exit(1);
            }
            cli::run::run_file(&args[2]);
        }
        "build" | "--build" => {
            if args.len() < 3 {
                eprintln!("Usage: sigil build <file.si> [-o output]");
                std::process::exit(1);
            }
            let output = cli::get_output_name(&args, 3);
            cli::build::build_file(&args[2], &output);
        }
        "emit" | "--emit" => {
            if args.len() < 3 {
                eprintln!("Usage: sigil emit <file.si> [-o output.c]");
                std::process::exit(1);
            }
            let output = cli::get_output_name(&args, 3);
            cli::build::emit_file(&args[2], &output);
        }
        "test" | "--test" => {
            if args.len() < 3 {
                // No file specified - discover and run all tests
                cli::test_runner::test_all();
            } else {
                cli::test_runner::test_file(&args[2]);
            }
        }
        "check" | "--check" => {
            if args.len() < 3 {
                eprintln!("Usage: sigil check <file.si>");
                std::process::exit(1);
            }
            cli::test_runner::check_coverage(&args[2]);
        }
        "--repl" | "repl" => {
            println!("Sigil REPL v0.1.0");
            println!("Type :help for help, :quit to exit\n");
            cli::repl::repl();
        }
        "--help" | "-h" | "help" => {
            cli::print_usage();
        }
        // Default: treat as file path for backwards compatibility
        path if path.ends_with(".si") => {
            cli::run::run_file(path);
        }
        _ => {
            eprintln!("Unknown command: {}", args[1]);
            cli::print_usage();
            std::process::exit(1);
        }
    }
}
