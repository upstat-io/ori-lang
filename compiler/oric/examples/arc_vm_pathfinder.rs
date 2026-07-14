//! Thin prototype harness for the production-shaped bytecode VM seams.

use std::env;
use std::fs;
use std::io::{self, Write};
use std::process;
use std::time::Instant;

fn main() {
    let (path, typed_primitives) = source_arguments();
    let source = fs::read_to_string(&path).unwrap_or_else(|error| {
        report_status("failed", 0, 0, 0, 1);
        eprintln!("error: cannot read {path}: {error}");
        process::exit(2);
    });

    let frontend_start = Instant::now();
    let executable = oric::test_support::compile_to_executable(
        &path,
        &source,
        ori_repr::NarrowingPolicy::Aggressive,
    )
    .unwrap_or_else(|error| {
        report_status("failed", 0, 0, 0, 1);
        eprintln!("error: {error}");
        process::exit(1);
    });
    let frontend_elapsed = frontend_start.elapsed();
    let bytecode_start = Instant::now();
    let bytecode =
        ori_vm::compile_with_options(&executable, ori_vm::CompileOptions { typed_primitives })
            .unwrap_or_else(|error| {
                report_status("unsupported", 0, 0, 1, 0);
                eprintln!("error: {error}");
                process::exit(3);
            });
    if env::var_os("ORI_VM_DUMP_BYTECODE").is_some() {
        eprintln!("{bytecode:#?}");
    }
    let bytecode = ori_vm::verify(bytecode).unwrap_or_else(|error| {
        report_status("failed", 0, 0, 0, 1);
        eprintln!("bytecode verifier error: {error}");
        process::exit(1);
    });
    let bytecode_elapsed = bytecode_start.elapsed();

    let vm_start = Instant::now();
    let outcome =
        ori_vm::execute(&bytecode, ori_vm::ExecutionConfig::default()).unwrap_or_else(|error| {
            report_status("failed", 1, 0, 0, 1);
            eprintln!("error: {error}");
            process::exit(1);
        });
    let vm_elapsed = vm_start.elapsed();

    io::stdout()
        .write_all(&outcome.output)
        .unwrap_or_else(|error| {
            eprintln!("error: cannot write VM output: {error}");
            process::exit(1);
        });
    let bytecode_metrics = bytecode.metrics();
    eprintln!(
        "status=full_vm entered_vm=1 fallback=0 unsupported=0 failed=0 typed_primitives={} functions={} bytecode_ops={} steps={} frames_peak={} heap_peak={} frontend_ms={:.3} bytecode_ms={:.3} vm_ms={:.3}",
        u8::from(typed_primitives),
        bytecode_metrics.function_count,
        bytecode_metrics.instruction_count,
        outcome.metrics.steps,
        outcome.metrics.peak_frames,
        outcome.metrics.peak_heap_objects,
        frontend_elapsed.as_secs_f64() * 1000.0,
        bytecode_elapsed.as_secs_f64() * 1000.0,
        vm_elapsed.as_secs_f64() * 1000.0,
    );
}

fn source_arguments() -> (String, bool) {
    let mut arguments = env::args().skip(1).peekable();
    let typed_primitives = if arguments.peek().is_some_and(|value| value == "--untyped") {
        arguments.next();
        false
    } else {
        true
    };
    let Some(path) = arguments.next() else {
        eprintln!("usage: arc_vm_pathfinder [--untyped] <source.ori>");
        process::exit(2);
    };
    if arguments.next().is_some() {
        eprintln!("error: unexpected extra argument");
        process::exit(2);
    }
    (path, typed_primitives)
}

fn report_status(status: &str, entered_vm: u8, fallback: u8, unsupported: u8, failed: u8) {
    eprintln!(
        "status={status} entered_vm={entered_vm} fallback={fallback} unsupported={unsupported} failed={failed}"
    );
}
