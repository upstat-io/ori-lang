//! Ownership-transfer matrix for a polymorphic constant lambda returning one
//! of its borrowed arguments.
//!
//! AIMS RL-2 classifies `Return` as an ownership transfer: the returned
//! argument's existing owner credit moves to the caller, while every discarded
//! owned argument and captured closure field still receives exactly one release.
//! Each cell compiles through the production AOT entry point under the
//! class-ledger-only probe and runs with `ORI_CHECK_LEAKS=1`.

use crate::util::compile_and_run_with_build_env;

const CLASS_LEDGER_PROBE: &[(&str, &str)] = &[
    ("ORI_DISABLE_PREDICATE_STACK_RC", "1"),
    ("ORI_VERIFY_ARC", "1"),
    ("ORI_VERIFY_EACH", "1"),
];

fn assert_class_ledger_aot_success(source: &str, label: &str) {
    let (exit, stdout, stderr) = compile_and_run_with_build_env(source, CLASS_LEDGER_PROBE);
    assert_eq!(
        exit, 0,
        "[{label}] expected clean exit\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        !stderr.contains("FATAL")
            && !stderr.contains("already-freed")
            && !stderr.to_lowercase().contains("leak"),
        "[{label}] reported a leak or double-free\nstderr:\n{stderr}"
    );
}

#[test]
fn returned_list_argument_transfers_one_owner_credit() {
    assert_class_ledger_aot_success(
        include_str!("fixtures/const_lambda_return_lifecycle/returned_list.ori"),
        "returned_list_argument_transfers_one_owner_credit",
    );
}

#[test]
fn returned_heap_string_argument_transfers_one_owner_credit() {
    assert_class_ledger_aot_success(
        include_str!("fixtures/const_lambda_return_lifecycle/returned_heap_str.ori"),
        "returned_heap_string_argument_transfers_one_owner_credit",
    );
}

#[test]
fn returned_composite_argument_transfers_nested_owner_credits() {
    assert_class_ledger_aot_success(
        include_str!("fixtures/const_lambda_return_lifecycle/returned_composite.ori"),
        "returned_composite_argument_transfers_nested_owner_credits",
    );
}

#[test]
fn non_returned_closure_capture_releases_independently() {
    assert_class_ledger_aot_success(
        include_str!("fixtures/const_lambda_return_lifecycle/non_returned_capture.ori"),
        "non_returned_closure_capture_releases_independently",
    );
}
