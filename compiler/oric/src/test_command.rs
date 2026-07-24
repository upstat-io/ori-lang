//! Argument parsing and execution for `ori test`.

use oric::commands::run_tests;
use oric::test::{Backend, OutputFormat, TestRunnerConfig};

#[derive(Clone, Copy)]
enum PendingValue {
    Filter,
    Format,
}

impl PendingValue {
    fn flag(self) -> &'static str {
        match self {
            Self::Filter => "--filter",
            Self::Format => "--format",
        }
    }

    fn expected(self) -> &'static str {
        match self {
            Self::Filter => "a non-empty pattern",
            Self::Format => "json or text",
        }
    }
}

struct ParsedTestCommand {
    paths: Vec<String>,
    config: TestRunnerConfig,
    help: bool,
}

pub(crate) fn run(args: &[String]) {
    let parsed = match parse_args(&args[2..]) {
        Ok(parsed) => parsed,
        Err(error) => exit_with_error(&error),
    };
    if parsed.help {
        print_usage();
        return;
    }
    let exit_code = run_tests(&parsed.paths, &parsed.config);
    if exit_code != 0 {
        std::process::exit(exit_code);
    }
}

fn parse_args(args: &[String]) -> Result<ParsedTestCommand, String> {
    let mut paths = Vec::new();
    let mut config = TestRunnerConfig::default();
    let mut pending = None;
    let mut help = false;

    for arg in args {
        if let Some(option) = pending.take() {
            parse_pending_value(option, arg, &mut config)?;
            continue;
        }
        match arg.as_str() {
            "--help" | "-h" => help = true,
            "--verbose" | "-v" => config.verbose = true,
            "--no-parallel" => config.parallel = false,
            "--coverage" => config.coverage = true,
            "--backend=llvm" => config.backend = Backend::LLVM,
            "--backend=interpreter" => config.backend = Backend::Interpreter,
            "--incremental" => config.incremental = true,
            "--format=json" => config.format = OutputFormat::Json,
            "--format=text" => config.format = OutputFormat::Text,
            "--filter" => pending = Some(PendingValue::Filter),
            "--format" => pending = Some(PendingValue::Format),
            "--__worker" => config.worker_protocol = true,
            _ => parse_inline_value(arg, &mut config, &mut paths)?,
        }
    }

    if let Some(option) = pending {
        return Err(missing_value_error(option));
    }
    if paths.is_empty() {
        paths.push(".".to_string());
    }
    Ok(ParsedTestCommand {
        paths,
        config,
        help,
    })
}

fn parse_pending_value(
    option: PendingValue,
    value: &str,
    config: &mut TestRunnerConfig,
) -> Result<(), String> {
    if value.is_empty() || value.starts_with('-') {
        return Err(missing_value_error(option));
    }
    match option {
        PendingValue::Filter => config.filter = Some(value.to_string()),
        PendingValue::Format => config.format = parse_format(value)?,
    }
    Ok(())
}

fn parse_inline_value(
    arg: &str,
    config: &mut TestRunnerConfig,
    paths: &mut Vec<String>,
) -> Result<(), String> {
    if let Some(filter) = arg.strip_prefix("--filter=") {
        if filter.is_empty() {
            return Err(missing_value_error(PendingValue::Filter));
        }
        config.filter = Some(filter.to_string());
    } else if let Some(format) = arg.strip_prefix("--format=") {
        config.format = parse_format(format)?;
    } else if let Some(names) = arg.strip_prefix("--__skip-unchanged=") {
        config.skip_unchanged = names
            .split(',')
            .filter(|name| !name.is_empty())
            .map(str::to_string)
            .collect();
    } else if arg.starts_with('-') {
        return Err(format!("unknown flag '{arg}' for `ori test`"));
    } else {
        paths.push(arg.to_string());
    }
    Ok(())
}

fn parse_format(value: &str) -> Result<OutputFormat, String> {
    match value {
        "json" => Ok(OutputFormat::Json),
        "text" => Ok(OutputFormat::Text),
        other => Err(format!(
            "invalid --format value '{other}' (expected json or text)"
        )),
    }
}

fn missing_value_error(option: PendingValue) -> String {
    format!(
        "{} requires {} (use `{} <value>` or `{}=<value>`)",
        option.flag(),
        option.expected(),
        option.flag(),
        option.flag()
    )
}

fn exit_with_error(error: &str) -> ! {
    eprintln!("error: {error}");
    eprintln!("Run `ori test --help` for usage.");
    std::process::exit(2);
}

fn print_usage() {
    println!("Usage: ori test [paths...] [options]");
    println!();
    println!("Run Ori spec tests in the given paths (default: current directory).");
    println!();
    print_options();
}

pub(crate) fn print_options() {
    println!("Test options:");
    println!("  --filter <pattern>  Only run tests matching pattern (also --filter=<pattern>)");
    println!("  --verbose, -v       Show detailed output");
    println!("  --no-parallel       Run tests sequentially");
    println!("  --coverage          Report which functions have tests");
    println!("  --backend=<name>    Use backend: interpreter (default), llvm");
    println!("  --incremental       Skip tests for unchanged functions");
    println!("  --format <name>     Summary format: text (default), json (also --format=<name>)");
    println!("  --help, -h          Show this help");
}
