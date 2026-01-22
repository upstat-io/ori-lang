//! Sigil V2 Compiler CLI

use std::env;

mod cli;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        cli::print_usage();
        std::process::exit(1);
    }

    match args[1].as_str() {
        "run" => {
            if args.len() < 3 {
                eprintln!("Usage: sigil-v2 run <file.si>");
                std::process::exit(1);
            }
            cli::run::run_file_and_print(&args[2]);
        }
        "test" => {
            if args.len() < 3 {
                eprintln!("Usage: sigil-v2 test <file.si>");
                std::process::exit(1);
            }
            cli::test::test_file_and_print(&args[2]);
        }
        "check" => {
            if args.len() < 3 {
                eprintln!("Usage: sigil-v2 check <file.si>");
                std::process::exit(1);
            }
            cli::check::check_file_and_print(&args[2]);
        }
        "-h" | "--help" | "help" => {
            cli::print_usage();
        }
        arg if arg.ends_with(".si") => {
            // Shorthand: sigil-v2 file.si = sigil-v2 run file.si
            cli::run::run_file_and_print(arg);
        }
        cmd => {
            eprintln!("Unknown command: {}", cmd);
            cli::print_usage();
            std::process::exit(1);
        }
    }
}
