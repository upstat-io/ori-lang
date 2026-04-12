//! `FileCheck`-style IR assertion tests.
//!
//! Compiles `.ori` files from `tests/codegen/`, captures LLVM IR, and
//! matches `// CHECK:` directives against the output. Uses the shared
//! `check` engine from `ori_test_harness`.

use crate::util::compile_and_capture_ir;
use ori_test_harness::check::{run_checks, CheckMode};
use ori_test_harness::directive;
use std::path::Path;

/// Run a single `FileCheck` test: compile the source, capture IR, match directives.
fn run_filecheck(relative_path: &str) {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let test_path = manifest.join(relative_path);
    let source = std::fs::read_to_string(&test_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", test_path.display()));

    let parse_result = directive::parse_directives(&source);
    let directives = parse_result.directives;

    let check_directives: Vec<_> = directives
        .iter()
        .filter(|d| {
            matches!(
                d.directive,
                directive::Directive::Check { .. }
                    | directive::Directive::CheckNot { .. }
                    | directive::Directive::CheckLabel { .. }
                    | directive::Directive::CheckNext { .. }
            )
        })
        .cloned()
        .collect();

    assert!(
        !check_directives.is_empty(),
        "no CHECK directives in {}",
        test_path.display()
    );

    let ir = compile_and_capture_ir(&source);
    let result = run_checks(&ir, &check_directives, CheckMode::Matches);

    if !result.passed {
        let failures: Vec<String> = result.failures.iter().map(|f| format!("  {f}")).collect();
        panic!(
            "FileCheck failed for {}:\n{}\n\n--- IR ---\n{}",
            test_path.display(),
            failures.join("\n"),
            &ir[..ir.len().min(2000)]
        );
    }
}

#[test]
fn filecheck_rc_simple_inc_dec() {
    run_filecheck("tests/codegen/rc_simple_inc_dec.ori");
}

#[test]
fn filecheck_rc_no_ops_for_int() {
    run_filecheck("tests/codegen/rc_no_ops_for_int.ori");
}

#[test]
fn filecheck_rc_string_copy_inc() {
    run_filecheck("tests/codegen/rc_string_copy_inc.ori");
}

#[test]
fn filecheck_cow_is_shared_check() {
    run_filecheck("tests/codegen/cow_is_shared_check.ori");
}

#[test]
fn filecheck_iterator_for_loop_drop() {
    run_filecheck("tests/codegen/iterator_for_loop_drop.ori");
}

#[test]
fn filecheck_abi_sret_struct_return() {
    run_filecheck("tests/codegen/abi_sret_struct_return.ori");
}

#[test]
fn filecheck_closure_capture_env() {
    run_filecheck("tests/codegen/closure_capture_env.ori");
}

#[test]
fn filecheck_closure_no_env_for_empty_capture() {
    run_filecheck("tests/codegen/closure_no_env_for_empty_capture.ori");
}

#[test]
fn filecheck_rc_list_of_strings_cleanup() {
    run_filecheck("tests/codegen/rc_list_of_strings_cleanup.ori");
}

#[test]
fn filecheck_rc_no_ops_for_bool() {
    run_filecheck("tests/codegen/rc_no_ops_for_bool.ori");
}

#[test]
fn filecheck_iterator_break_cleanup() {
    run_filecheck("tests/codegen/iterator_break_cleanup.ori");
}

#[test]
fn filecheck_abi_void_return() {
    run_filecheck("tests/codegen/abi_void_return.ori");
}
