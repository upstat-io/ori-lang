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
                eprintln!("Usage: sigil emit <file.si> [-o output.c] [--verbose-arc]");
                std::process::exit(1);
            }
            let output = cli::get_output_name(&args, 3);
            let verbose_arc = args.iter().any(|a| a == "--verbose-arc");
            if verbose_arc {
                cli::build::emit_file_verbose_arc(&args[2], &output);
            } else {
                cli::build::emit_file(&args[2], &output);
            }
        }
        "test" | "--test" => {
            if args.len() < 3 {
                // No file specified - discover and run all tests
                cli::test::test_all();
            } else {
                cli::test::test_file(&args[2]);
            }
        }
        "check" | "--check" => {
            if args.len() < 3 {
                eprintln!("Usage: sigil check <file.si>");
                std::process::exit(1);
            }
            cli::test::check_coverage(&args[2]);
        }
        "fmt" | "format" => {
            let mut in_place = false;
            let mut check_only = false;
            let mut path: Option<&str> = None;

            for arg in args.iter().skip(2) {
                match arg.as_str() {
                    "-i" | "--in-place" => in_place = true,
                    "--check" => check_only = true,
                    _ if !arg.starts_with('-') => path = Some(arg),
                    _ => {
                        eprintln!("Unknown option: {}", arg);
                        std::process::exit(1);
                    }
                }
            }

            if check_only {
                let p = path.unwrap_or(".");
                if !cli::format::check_directory(p) {
                    std::process::exit(1);
                }
            } else if let Some(p) = path {
                if std::path::Path::new(p).is_dir() {
                    cli::format::format_directory(p, in_place);
                } else {
                    cli::format::format_file(p, in_place);
                }
            } else if in_place {
                cli::format::format_directory(".", true);
            } else {
                cli::format::format_stdin();
            }
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
